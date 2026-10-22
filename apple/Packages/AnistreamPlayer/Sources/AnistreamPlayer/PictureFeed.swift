// mpv's frames as sample buffers, for Picture in Picture on the Mac, iPhone and iPad.
//
// The system's Picture in Picture window shows an AVPlayerLayer or an AVSampleBufferDisplayLayer,
// and mpv draws into neither: it renders through MoltenVK into the player's CAMetalLayer. While
// the feed runs, every frame mpv presents is copied on the GPU into an IOSurface-backed pixel
// buffer and enqueued on `displayLayer`. The copy is of the finished frame, after the shaders,
// with the subtitles and the OSD drawn in. Nothing runs while the feed is off.
//
// The frames go out as 8-bit BGRA, which every display layer takes, whatever the drawable is:
// mpv's is 10-bit, so the copy converts as it samples (MPS's bilinear scale), at the drawable's
// size or within `maximumSize`.

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
/// - The iOS Simulator's drawables have no presented handler. There each is copied as mpv takes
///   the next, by when mpv has submitted the frame; the copy's queue is not ordered after mpv's,
///   so a copy can catch a frame still being drawn. The Simulator is for trying the steps, not
///   for judging the picture.
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
    private var limit: CGSize?
    private var aspect: Double?
    /// The scaler's transform is set for each copy, one copy at a time.
    private let encodeLock = NSLock()
    private var tally = Counts()
    /// What makes the pixel buffer pool for frames of one size and format: a test's may fail.
    let makePool: @Sendable (PoolKey) -> CVPixelBufferPool?
    private var lastFrame: CVPixelBuffer?
    #if targetEnvironment(simulator)
    /// The drawable mpv took last, copied as it takes the next.
    private var previous: CAMetalDrawable?
    #endif
    private let enqueueing = DispatchQueue(label: "tv.anistream.picture-feed")

    struct PoolKey: Equatable {
        let width: Int
        let height: Int
        let pixelFormat: OSType
    }

    @MainActor
    public convenience init() {
        self.init(makePool: Self.makePool)
    }

    @MainActor
    init(makePool: @escaping @Sendable (PoolKey) -> CVPixelBufferPool?) {
        self.makePool = makePool
        displayLayer = AVSampleBufferDisplayLayer()
        displayLayer.videoGravity = .resizeAspect
        displayLayer.backgroundColor = CGColor(gray: 0, alpha: 1)
        renderer = displayLayer.sampleBufferRenderer
    }

    public var isRunning: Bool { lock.withLock { layer != nil } }

    public var counts: Counts { lock.withLock { tally } }

    /// Copies and stills in flight, for a test.
    var inFlight: Int { lock.withLock { pending } }

    /// The largest frame handed over, in pixels: a larger drawable is scaled down to fit inside
    /// it, keeping its shape. Nil, the default: the drawable's own size. On iPhone and iPad the
    /// feed runs while the player plays full screen, so the system can take the picture into
    /// its floating window by itself, and there a smaller copy does.
    public var maximumSize: CGSize? {
        get { lock.withLock { limit } }
        set { lock.withLock { limit = newValue } }
    }

    /// The picture's width over its height, when mpv draws it with bars into a drawable of
    /// another shape (the player full screen or in its window): only that centred part is
    /// copied, so the floating window opens at the video's shape. Nil, the default: the whole
    /// drawable, which is the video's shape once the picture floats.
    public var contentAspect: Double? {
        get { lock.withLock { aspect } }
        set { lock.withLock { aspect = newValue } }
    }

    /// The centred part of a `width` × `height` drawable at `aspect`, in pixels; all of it with
    /// no aspect.
    static func content(width: Int, height: Int, aspect: Double?) -> CGRect {
        let whole = CGRect(x: 0, y: 0, width: width, height: height)
        guard let aspect, aspect > 0, width > 0, height > 0 else { return whole }
        let shape = Double(width) / Double(height)
        if abs(shape - aspect) < 0.01 { return whole }
        if shape > aspect {
            let w = (Double(height) * aspect).rounded()
            return CGRect(x: ((Double(width) - w) / 2).rounded(), y: 0, width: w, height: Double(height))
        }
        let h = (Double(width) / aspect).rounded()
        return CGRect(x: 0, y: ((Double(height) - h) / 2).rounded(), width: Double(width), height: h)
    }

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
        #if targetEnvironment(simulator)
        layer.tapDrawables { drawable in feed.feed?.taken(drawable) }
        #else
        layer.tapDrawables { drawable in
            drawable.addPresentedHandler { presented in
                guard let drawable = presented as? CAMetalDrawable else { return }
                feed.feed?.copy(drawable)
            }
        }
        #endif
        return true
    }

    #if targetEnvironment(simulator)
    private func taken(_ drawable: CAMetalDrawable) {
        let last = lock.withLock { () -> CAMetalDrawable? in
            let last = previous
            previous = drawable
            return last
        }
        if let last { copy(last) }
    }
    #endif

    /// Stop copying. What the display layer shows stays until it is flushed.
    public func stop() {
        let layer = lock.withLock { () -> MpvLayer? in
            let layer = self.layer
            self.layer = nil
            #if targetEnvironment(simulator)
            previous = nil
            #endif
            return layer
        }
        layer?.tapDrawables(nil)
    }

    /// Empty the display layer, as after a jump in time.
    public func flush() {
        enqueueing.async { [self] in renderer.flush() }
    }

    /// `width` × `height` scaled down to fit inside `limit`, keeping its shape; as it is when it
    /// fits, or with no limit.
    static func fitted(width: Int, height: Int, inside limit: CGSize?) -> (width: Int, height: Int) {
        guard let limit, limit.width >= 2, limit.height >= 2,
            Double(width) > limit.width || Double(height) > limit.height
        else { return (width, height) }
        let scale = min(limit.width / Double(width), limit.height / Double(height))
        // Even sizes, which every video format takes.
        func even(_ value: Double) -> Int { max(2, Int((value / 2).rounded()) * 2) }
        return (even(Double(width) * scale), even(Double(height) * scale))
    }

    private func copy(_ drawable: CAMetalDrawable) {
        let source = drawable.texture
        let prepared: (MTLCommandQueue, MPSImageBilinearScale, CVPixelBufferPool, PoolKey, CGRect)? = lock.withLock {
            guard layer != nil, let queue, let scaler else { return nil }
            let part = Self.content(width: source.width, height: source.height, aspect: aspect)
            let size = Self.fitted(width: Int(part.width), height: Int(part.height), inside: limit)
            let key = PoolKey(width: size.width, height: size.height, pixelFormat: kCVPixelFormatType_32BGRA)
            guard pending < Self.maximumPending else {
                tally.dropped += 1
                return nil
            }
            if poolKey != key {
                pool = makePool(key)
                poolKey = key
                format = nil
            }
            guard let pool else { return nil }
            pending += 1
            return (queue, scaler, pool, key, part)
        }
        guard let (queue, scaler, pool, key, part) = prepared else { return }
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
        if source.pixelFormat == .bgra8Unorm, source.width == key.width, source.height == key.height,
            let blit = buffer.makeBlitCommandEncoder()
        {
            blit.copy(from: source, to: target)
            blit.endEncoding()
        } else {
            // A different format (mpv's 10-bit drawable), a part, or a smaller size: converted,
            // cut and scaled as it is sampled. The transform maps the part onto the whole target.
            let scaleX = Double(key.width) / part.width
            let scaleY = Double(key.height) / part.height
            var transform = MPSScaleTransform(
                scaleX: scaleX, scaleY: scaleY, translateX: -part.minX * scaleX, translateY: -part.minY * scaleY)
            encodeLock.withLock {
                withUnsafePointer(to: &transform) { pointer in
                    scaler.scaleTransform = pointer
                    scaler.encode(commandBuffer: buffer, sourceTexture: source, destinationTexture: target)
                    scaler.scaleTransform = nil
                }
            }
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

    /// Show `image` as the next frame: mpv's screenshot of the paused picture, cut to the
    /// video's part as the frames are.
    public func show(_ whole: CGImage) {
        let part = Self.content(width: whole.width, height: whole.height, aspect: contentAspect)
        guard let image = whole.cropping(to: part) else { return }
        let key = PoolKey(width: image.width, height: image.height, pixelFormat: kCVPixelFormatType_32BGRA)
        let pool: CVPixelBufferPool? = lock.withLock {
            guard layer != nil else { return nil }
            if poolKey != key {
                self.pool = makePool(key)
                poolKey = key
                format = nil
            }
            // Pending only once there is a pool to draw from: a frame that cannot be made is
            // dropped here, and would otherwise hold a copy's place for good.
            guard let pool = self.pool else {
                tally.dropped += 1
                return nil
            }
            pending += 1
            return pool
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

    static func makePool(_ key: PoolKey) -> CVPixelBufferPool? {
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
