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
            try await Task.sleep(for: .seconds(2))
            let counts = feed.counts
            let frame = try #require(feed.latestFrame)
            feed.stop()
            await client.shutdown()
            // testsrc.mkv is 24 fps: about 48 frames in two seconds, every one copied.
            #expect(counts.copied >= 36, "\(counts)")
            #expect(counts.dropped <= 2, "\(counts)")
            #expect(CVPixelBufferGetWidth(frame) == 960 && CVPixelBufferGetHeight(frame) == 540)
            #expect(CVPixelBufferGetIOSurface(frame) != nil)
            #expect(variety(try image(frame)) > 0.01)
        }

        /// The feed shows what mpv drew: the frame after the shaders, with the subtitles in it, as
        /// mpv's own screenshot of the window has it.
        @Test func showsWhatMpvDrewWithSubtitlesAndShaders() async throws {
            let (client, layer) = try await startedClient(width: 960, height: 540)
            try await play(client, "subs.mkv")
            try await client.setShaders([Media.shaders.appending(path: "KrigBilateral.glsl").path])
            try await Task.sleep(for: .seconds(1))
            try await client.set("pause", .flag(true))
            let feed = await PictureFeed()
            feed.start(layer)
            // A redraw of the paused frame, which the feed copies.
            try await client.seek(0.5, mode: .absolute)
            let copied = await eventually { feed.counts.copied > 0 ? true : nil }
            #expect(copied == true)
            try await Task.sleep(for: .milliseconds(500))
            let frame = try #require(feed.latestFrame)
            let screenshot = try await client.screenshot()
            feed.stop()
            await client.shutdown()
            let difference = meanDifference(try image(frame), screenshot)
            // The same picture: the only difference is the 10-bit drawable against the 8-bit
            // screenshot, a level or two.
            #expect(difference < 4, "the feed differs from mpv's screenshot by \(difference) levels")
            #expect(variety(screenshot) > 0.01)
        }

        @Test func copiesNothingOnceStopped() async throws {
            let (client, layer) = try await startedClient(width: 640, height: 360)
            try await play(client, "testsrc.mkv")
            let feed = await PictureFeed()
            feed.start(layer)
            try await Task.sleep(for: .milliseconds(500))
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
            #expect(counts.copied >= 70, "\(counts)")
            #expect(perFrame < 2)
        }
    }
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
