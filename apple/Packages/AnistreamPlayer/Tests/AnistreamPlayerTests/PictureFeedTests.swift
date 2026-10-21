// Picture in Picture's frames: what the feed copies from mpv's layer, and what that costs.

#if os(macOS)
import AnistreamPlayer
import CoreImage
import CoreVideo
import Darwin
import Foundation
import Testing

extension PlayerTests {
    @Suite struct PictureFeedTests {
        @Test func copiesTheFramesMpvPresents() async throws {
            let (client, layer) = try await startedClient(width: 960, height: 540)
            try await play(client, "testsrc.mkv")
            let feed = await PictureFeed()
            #expect(feed.start(layer))
            // testsrc.mkv is 24 fps: 36 frames in a second and a half, or later when the machine
            // is busy and mpv draws fewer, every one copied.
            let copied = await eventually { feed.counts.copied >= 36 ? feed.counts : nil }
            let counts = feed.counts
            let frame = try #require(feed.latestFrame)
            feed.stop()
            await client.shutdown()
            #expect(copied != nil, "\(counts)")
            #expect(counts.dropped <= 2, "\(counts)")
            #expect(CVPixelBufferGetWidth(frame) == 960 && CVPixelBufferGetHeight(frame) == 540)
            #expect(CVPixelBufferGetIOSurface(frame) != nil)
            #expect(variety(try image(frame)) > 0.01)
        }

        /// In the player on iPhone and iPad: the video's part of a drawable of another shape,
        /// small. The video is 16:9 in a portrait layer, with bars above and below it.
        @Test func copiesTheVideosPartSmall() async throws {
            let (client, layer) = try await startedClient(width: 540, height: 960)
            try await play(client, "testsrc.mkv")
            let feed = await PictureFeed()
            feed.contentAspect = 16 / 9
            feed.maximumSize = CGSize(width: 320, height: 320)
            #expect(feed.start(layer))
            let copied = await eventually { feed.counts.copied >= 12 ? true : nil }
            let frame = try #require(feed.latestFrame)
            feed.stop()
            await client.shutdown()
            #expect(copied == true, "\(feed.counts)")
            // 540 wide at 16:9 is 540×304, fitted inside 320×320 at even sizes.
            #expect(CVPixelBufferGetWidth(frame) == 320 && CVPixelBufferGetHeight(frame) == 180)
            // The video's rows top and bottom, not the bars: neither is black throughout.
            let image = try image(frame)
            #expect(brightness(of: image, row: 2) > 0.05 && brightness(of: image, row: image.height - 3) > 0.05)
        }

        /// The feed shows what mpv drew: the frame after the shaders, with the subtitles in it, as
        /// mpv's own screenshot of the window has it, and not the video alone. Playing, so the
        /// frames differ by a few frames of motion either way; only the subtitles tell them apart.
        @Test func showsWhatMpvDrewWithSubtitlesAndShaders() async throws {
            let (client, layer) = try await startedClient(width: 1280, height: 720)
            try await play(client, "subs.mkv")
            try await client.setShaders([Media.shaders.appending(path: "KrigBilateral.glsl").path])
            let feed = await PictureFeed()
            feed.start(layer)
            // A second of frames in, so the subtitles are up.
            _ = await eventually { feed.counts.copied >= 24 ? true : nil }
            let frame = try image(try #require(feed.latestFrame))
            let window = try await client.screenshot(.window)
            let video = try await client.screenshot(.video)
            feed.stop()
            await client.shutdown()
            let toWindow = meanDifference(frame, window)
            let toVideo = meanDifference(frame, video)
            #expect(toWindow < toVideo, "the feed is \(toWindow) from the window and \(toVideo) from the video alone")
            #expect(toWindow < 12, "the feed differs from mpv's screenshot by \(toWindow) levels")
        }

        /// Paused, mpv draws no frames for the feed to copy: its screenshot is shown instead.
        @Test func showsAStillWhilePaused() async throws {
            let (client, layer) = try await startedClient(width: 640, height: 360)
            try await play(client, "testsrc.mkv")
            try await client.set("pause", .flag(true))
            let feed = await PictureFeed()
            feed.start(layer)
            // What was drawn before the pause comes through; then nothing more.
            try await Task.sleep(for: .milliseconds(800))
            let before = feed.counts.copied
            try await Task.sleep(for: .milliseconds(500))
            #expect(feed.counts.copied == before)
            feed.show(try await client.screenshot(.window))
            let frame = try #require(feed.latestFrame)
            #expect(feed.counts.copied == before + 1)
            #expect(CVPixelBufferGetWidth(frame) == 640 && CVPixelBufferGetHeight(frame) == 360)
            #expect(variety(try image(frame)) > 0.01)
            feed.stop()
            await client.shutdown()
        }

        @Test func copiesNothingOnceStopped() async throws {
            let (client, layer) = try await startedClient(width: 640, height: 360)
            try await play(client, "testsrc.mkv")
            let feed = await PictureFeed()
            feed.start(layer)
            _ = await eventually { feed.counts.copied > 0 ? true : nil }
            feed.stop()
            try await Task.sleep(for: .milliseconds(200))
            let before = feed.counts.copied
            try await Task.sleep(for: .milliseconds(800))
            #expect(feed.counts.copied == before)
            #expect(before > 0)
            await client.shutdown()
        }

        /// What the feed costs, against the same playback without it. Printed for the record;
        /// the bounds are loose, as the machine running the suite is busy.
        @Test func costsLittleOnTheCPUAndTheGPU() async throws {
            let (client, layer) = try await startedClient(width: 1280, height: 720)
            try await play(client, "testsrc.mkv")
            try await Task.sleep(for: .seconds(1))
            let off = try await cpuSeconds(over: .seconds(4))
            let feed = await PictureFeed()
            feed.start(layer)
            let on = try await cpuSeconds(over: .seconds(4))
            let counts = feed.counts
            feed.stop()
            await client.shutdown()
            let perFrame = counts.copied > 0 ? counts.gpuSeconds / Double(counts.copied) * 1000 : 0
            print("picture feed: process CPU \(String(format: "%.3f", off)) s without, \(String(format: "%.3f", on)) s with, over 4 s; \(counts.copied) frames copied, \(counts.dropped) dropped, GPU \(String(format: "%.3f", perFrame)) ms a frame")
            // At least half of the 96 frames: mpv draws fewer when the machine is busy.
            #expect(counts.copied >= 48, "\(counts)")
            // 0.17–0.25 ms a frame on an M4 Pro. CI's runners are virtual Macs with an emulated GPU,
            // where the same copy measured 2.5 ms, so there the bound only catches a large regression.
            let bound = ProcessInfo.processInfo.environment["CI"] == nil ? 2.0 : 10.0
            #expect(perFrame < bound, "GPU \(perFrame) ms a frame")
        }
    }
}

/// The mean brightness of one row of `image`, 0 to 1.
private func brightness(of image: CGImage, row: Int) -> Double {
    var data = [UInt8](repeating: 0, count: image.width * image.height * 4)
    let context = CGContext(
        data: &data, width: image.width, height: image.height, bitsPerComponent: 8,
        bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
    context?.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    // Rows are stored top first.
    let start = row * image.width * 4
    var sum = 0
    for x in 0..<image.width {
        let pixel = start + x * 4
        sum += Int(data[pixel]) + Int(data[pixel + 1]) + Int(data[pixel + 2])
    }
    return Double(sum) / Double(image.width * 3 * 255)
}

/// The process's CPU time, user and system, over `duration`.
private func cpuSeconds(over duration: Duration) async throws -> Double {
    func now() -> Double {
        var usage = rusage()
        getrusage(RUSAGE_SELF, &usage)
        return Double(usage.ru_utime.tv_sec + usage.ru_stime.tv_sec)
            + Double(usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) / 1_000_000
    }
    let start = now()
    try await Task.sleep(for: duration)
    return now() - start
}

/// A pixel buffer as an 8-bit sRGB image.
private func image(_ buffer: CVPixelBuffer) throws -> CGImage {
    let context = CIContext()
    let ci = CIImage(cvPixelBuffer: buffer)
    return try #require(context.createCGImage(
        ci, from: ci.extent, format: .RGBA8, colorSpace: CGColorSpace(name: CGColorSpace.sRGB)))
}

/// The mean difference between two images of one size, in 8-bit levels, over a grid of points.
private func meanDifference(_ a: CGImage, _ b: CGImage) -> Double {
    func pixels(_ image: CGImage) -> [UInt8] {
        var data = [UInt8](repeating: 0, count: image.width * image.height * 4)
        let context = CGContext(
            data: &data, width: image.width, height: image.height, bitsPerComponent: 8,
            bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        context?.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return data
    }
    guard a.width == b.width, a.height == b.height else { return .infinity }
    let (pa, pb) = (pixels(a), pixels(b))
    var total = 0.0
    var count = 0
    for y in stride(from: 0, to: a.height, by: 4) {
        for x in stride(from: 0, to: a.width, by: 4) {
            let i = (y * a.width + x) * 4
            for c in 0..<3 { total += abs(Double(pa[i + c]) - Double(pb[i + c])) }
            count += 3
        }
    }
    return total / Double(count)
}
#endif
