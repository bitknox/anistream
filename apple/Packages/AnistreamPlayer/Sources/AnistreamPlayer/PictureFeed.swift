// mpv's frames as sample buffers, for Picture in Picture on the Mac.
//
// The system's Picture in Picture window shows an AVPlayerLayer or an AVSampleBufferDisplayLayer,
// and mpv draws into neither: it renders through MoltenVK into the player's CAMetalLayer. While
// the feed runs, every frame mpv presents is copied on the GPU into an IOSurface-backed pixel
// buffer and enqueued on `displayLayer`. The copy is of the finished frame, after the shaders,
// with the subtitles and the OSD drawn in. Nothing runs while the feed is off.
//
// The frames go out as 8-bit BGRA, which every display layer takes, whatever the drawable is:
// mpv's is 10-bit, so the copy converts as it samples (MPS's bilinear scale, at the same size).

#if os(macOS)
import AVFoundation
import CoreMedia
import CoreVideo
import Foundation
import Metal
import MetalPerformanceShaders
import QuartzCore

/// Copies the frames mpv presents to an `MpvLayer` into `displayLayer`.
///
/// - The copy is made when the frame has been presented, from the drawable's own texture: a
///   drawable is presented only once the work that drew it has finished, and the copy runs on a
///   queue of its own, which Metal does not order after MoltenVK's.
/// - The drawable is held until its copy is done, so the layer cannot hand its texture out again
///   meanwhile.
/// - A frame is dropped rather than queued when the copies fall behind (`maximumPending`).
/// - With the layer in no window, a drawable is presented only once later frames push it out,
///   about three frames behind, and a paused mpv draws no later frames: what it shows while
///   paused comes through `show(_:)`, from mpv's own screenshot of the window.
public final class PictureFeed: @unchecked Sendable {
    /// What the Picture in Picture window shows.
    nonisolated(unsafe) public let displayLayer: AVSampleBufferDisplayLayer
    /// The display layer's renderer, which takes frames from any thread.
    nonisolated(unsafe) private let renderer: AVSampleBufferVideoRenderer

    /// Frames copied, frames dropped, and the GPU time the copies took, since the feed started.
    public struct Counts: Sendable, Equatable {
        public var copied = 0
        public var dropped = 0
        public var gpuSeconds: Double = 0
    }

    /// Copies in flight past which a frame is dropped.
    public static let maximumPending = 2

    private let lock = NSLock()
    private var layer: MpvLayer?
    private var queue: MTLCommandQueue?
    private var scaler: MPSImageBilinearScale?
    private var pool: CVPixelBufferPool?
    private var poolKey: PoolKey?
    private var format: CMVideoFormatDescription?
    private var pending = 0
    private var tally = Counts()
    private var lastFrame: CVPixelBuffer?
    private let enqueueing = DispatchQueue(label: "tv.anistream.picture-feed")

    private struct PoolKey: Equatable {
        let width: Int
        let height: Int
        let pixelFormat: OSType
    }

    @MainActor
    public init() {
        displayLayer = AVSampleBufferDisplayLayer()
        displayLayer.videoGravity = .resizeAspect
        displayLayer.backgroundColor = CGColor(gray: 0, alpha: 1)
        renderer = displayLayer.sampleBufferRenderer
    }

    public var isRunning: Bool { lock.withLock { layer != nil } }

    public var counts: Counts { lock.withLock { tally } }

    /// The last frame copied, for a capture of what the window shows.
    public var latestFrame: CVPixelBuffer? { lock.withLock { lastFrame } }

    /// Start copying `layer`'s frames. False when there is no Metal device to copy with.
    @discardableResult
    public func start(_ layer: MpvLayer) -> Bool {
        guard let device = layer.device ?? MTLCreateSystemDefaultDevice(),
            let queue = device.makeCommandQueue()
        else { return false }
        queue.label = "tv.anistream.picture-feed"
        let scaler = MPSImageBilinearScale(device: device)
        lock.withLock {
            self.layer = layer
            self.queue = queue
            self.scaler = scaler
            tally = Counts()
        }
        let feed = WeakFeed(feed: self)
        layer.tapDrawables { drawable in
            drawable.addPresentedHandler { presented in
                guard let drawable = presented as? CAMetalDrawable else { return }
                feed.feed?.copy(drawable)
            }
        }
        return true
    }

    /// Stop copying. What the display layer shows stays until it is flushed.
    public func stop() {
        let layer = lock.withLock { () -> MpvLayer? in
            let layer = self.layer
            self.layer = nil
            return layer
        }
        layer?.tapDrawables(nil)
    }

    /// Empty the display layer, as after a jump in time.
    public func flush() {
        enqueueing.async { [self] in renderer.flush() }
    }

    private func copy(_ drawable: CAMetalDrawable) {
        let source = drawable.texture
        let key = PoolKey(width: source.width, height: source.height, pixelFormat: kCVPixelFormatType_32BGRA)
        let prepared: (MTLCommandQueue, MPSImageBilinearScale, CVPixelBufferPool)? = lock.withLock {
            guard layer != nil, let queue, let scaler else { return nil }
            guard pending < Self.maximumPending else {
                tally.dropped += 1
                return nil
            }
            if poolKey != key {
                pool = Self.makePool(key)
                poolKey = key
                format = nil
            }
            guard let pool else { return nil }
            pending += 1
            return (queue, scaler, pool)
        }
        guard let (queue, scaler, pool) = prepared else { return }
        var made: CVPixelBuffer?
        CVPixelBufferPoolCreatePixelBuffer(nil, pool, &made)
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm, width: key.width, height: key.height, mipmapped: false)
        descriptor.usage = [.shaderWrite, .shaderRead, .renderTarget]
        descriptor.storageMode = .shared
        guard let pixels = made, let surface = CVPixelBufferGetIOSurface(pixels)?.takeUnretainedValue(),
            let target = queue.device.makeTexture(descriptor: descriptor, iosurface: surface, plane: 0),
            let buffer = queue.makeCommandBuffer()
        else {
            lock.withLock {
                pending -= 1
                tally.dropped += 1
            }
            return
        }
        if let space = layerColorSpace() {
            CVBufferSetAttachment(pixels, kCVImageBufferCGColorSpaceKey, space, .shouldPropagate)
        }
        if source.pixelFormat == .bgra8Unorm, let blit = buffer.makeBlitCommandEncoder() {
            blit.copy(from: source, to: target)
            blit.endEncoding()
        } else {
            // A different format (mpv's 10-bit drawable): converted as it is sampled.
            scaler.encode(commandBuffer: buffer, sourceTexture: source, destinationTexture: target)
        }
        // The drawable and the target texture are held until the copy is done.
        let held = Held(drawable: drawable, texture: target, pixels: pixels)
        let feed = WeakFeed(feed: self)
        buffer.addCompletedHandler { done in
            feed.feed?.finished(held.pixels, gpuSeconds: max(0, done.gpuEndTime - done.gpuStartTime), ok: done.status == .completed)
            withExtendedLifetime(held) {}
        }
        buffer.commit()
    }

    /// Show `image` as the next frame: mpv's screenshot of the paused picture.
    public func show(_ image: CGImage) {
        let key = PoolKey(width: image.width, height: image.height, pixelFormat: kCVPixelFormatType_32BGRA)
        let pool: CVPixelBufferPool? = lock.withLock {
            guard layer != nil else { return nil }
            if poolKey != key {
                self.pool = Self.makePool(key)
                poolKey = key
                format = nil
            }
            pending += 1
            return self.pool
        }
        guard let pool else { return }
        var made: CVPixelBuffer?
        CVPixelBufferPoolCreatePixelBuffer(nil, pool, &made)
        guard let pixels = made else {
            finished(nil, gpuSeconds: 0, ok: false)
            return
        }
        CVPixelBufferLockBaseAddress(pixels, [])
        let context = CGContext(
            data: CVPixelBufferGetBaseAddress(pixels), width: key.width, height: key.height,
            bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(pixels),
            space: image.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        context?.draw(image, in: CGRect(x: 0, y: 0, width: key.width, height: key.height))
        CVPixelBufferUnlockBaseAddress(pixels, [])
        if let space = image.colorSpace {
            CVBufferSetAttachment(pixels, kCVImageBufferCGColorSpaceKey, space, .shouldPropagate)
        }
        finished(context == nil ? nil : pixels, gpuSeconds: 0, ok: context != nil)
    }

    private func finished(_ pixels: CVPixelBuffer?, gpuSeconds: Double, ok: Bool) {
        let format = lock.withLock { () -> CMVideoFormatDescription? in
            pending -= 1
            guard ok, let pixels, layer != nil else {
                tally.dropped += 1
                return nil
            }
            tally.copied += 1
            tally.gpuSeconds += gpuSeconds
            lastFrame = pixels
            if self.format == nil || !CMVideoFormatDescriptionMatchesImageBuffer(self.format!, imageBuffer: pixels) {
                var made: CMVideoFormatDescription?
                CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixels, formatDescriptionOut: &made)
                self.format = made
            }
            return self.format
        }
        guard let format, let pixels else { return }
        // Shown as it arrives: the frames come at mpv's pace, already in time.
        var timing = CMSampleTimingInfo(
            duration: .invalid, presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()),
            decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        CMSampleBufferCreateReadyWithImageBuffer(
            allocator: nil, imageBuffer: pixels, formatDescription: format, sampleTiming: &timing,
            sampleBufferOut: &sample)
        guard let sample else { return }
        if let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: true),
            CFArrayGetCount(attachments) > 0
        {
            let dictionary = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
            CFDictionarySetValue(
                dictionary, Unmanaged.passUnretained(kCMSampleAttachmentKey_DisplayImmediately).toOpaque(),
                Unmanaged.passUnretained(kCFBooleanTrue).toOpaque())
        }
        let sendable = SendableSample(sample: sample)
        enqueueing.async { [self] in
            if renderer.status == .failed { renderer.flush() }
            renderer.enqueue(sendable.sample)
        }
    }

    private func layerColorSpace() -> CGColorSpace? {
        lock.withLock { layer }?.colorspace
    }

    private static func makePool(_ key: PoolKey) -> CVPixelBufferPool? {
        let attributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: key.pixelFormat,
            kCVPixelBufferWidthKey as String: key.width,
            kCVPixelBufferHeightKey as String: key.height,
            kCVPixelBufferMetalCompatibilityKey as String: true,
            kCVPixelBufferIOSurfacePropertiesKey as String: [String: Any](),
        ]
        var pool: CVPixelBufferPool?
        CVPixelBufferPoolCreate(nil, [kCVPixelBufferPoolMinimumBufferCountKey as String: 4] as CFDictionary, attributes as CFDictionary, &pool)
        return pool
    }
}

private struct SendableSample: @unchecked Sendable {
    let sample: CMSampleBuffer
}

/// What a copy in flight keeps alive: the drawable it reads, the texture it writes, and the pixel
/// buffer behind that texture.
private struct Held: @unchecked Sendable {
    let drawable: CAMetalDrawable
    let texture: MTLTexture
    let pixels: CVPixelBuffer
}

private struct WeakFeed: @unchecked Sendable {
    weak var feed: PictureFeed?
}
#endif
