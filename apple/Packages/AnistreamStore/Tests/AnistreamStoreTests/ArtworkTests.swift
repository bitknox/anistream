// Artwork asked for once per URL, decoded small enough, and asked again while on screen.

@testable import AnistreamStore
import CoreGraphics
import Foundation
import ImageIO
import Synchronization
import Testing
import UniformTypeIdentifiers

/// A PNG of `width` by `height` pixels in one colour.
func png(width: Int, height: Int) -> Data {
    let context = CGContext(
        data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )!
    context.setFillColor(red: 0.89, green: 0.63, blue: 0.36, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let data = NSMutableData()
    let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    CGImageDestinationFinalize(destination)
    return data as Data
}

@MainActor
struct ArtworkTests {
    @Test func aURLIsAskedForOnceWhileItIsOnItsWayAndOnceItIsHere() async throws {
        var asked: [String] = []
        let artwork = ArtworkStore { url in asked.append(url); return true }
        let url = "https://img.example/cover/900001.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == [url])
        #expect(artwork.isPending(url))

        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == [url])
        #expect(!artwork.isPending(url))
    }

    @Test func artIsDownsampledToTheLargestSizeAsked() async throws {
        let artwork = ArtworkStore { _ in true }
        let url = "https://img.example/banner/900001.jpg"
        artwork.request(url, maxPixelSize: 300)
        artwork.request(url, maxPixelSize: 600)
        artwork.ready(url: url, bytes: png(width: 1900, height: 400))
        try await waitFor { artwork.image(url) != nil }
        let image = try #require(artwork.image(url))
        #expect(image.width == 600)
        #expect(image.height == 126)
    }

    @Test func aLargerAskDecodesAgainWhereTheFileHasMore() async throws {
        // Johan's blurry banner: a list row decoded a cover at 108 pixels, and the preview's
        // banner, the same URL for a title with no banner, stretched that decode.
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900005-xl.jpg"
        let bytes = png(width: 460, height: 690)
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url) != nil }
        #expect(artwork.image(url)?.height == 108)

        artwork.request(url, maxPixelSize: 1800)
        #expect(asked.value == 2, "asked again, for the larger decode")
        #expect(artwork.image(url)?.height == 108, "the small one shows meanwhile")
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url)?.height == 690 }
        // The file has no more than that, so a larger ask still asks nothing.
        artwork.request(url, maxPixelSize: 2400)
        #expect(asked.value == 2)
    }

    @Test func aSmallerAskUsesTheLargerDecode() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/banner/900001.jpg"
        artwork.request(url, maxPixelSize: 600)
        artwork.ready(url: url, bytes: png(width: 1600, height: 900))
        try await waitFor { artwork.image(url) != nil }
        artwork.request(url, maxPixelSize: 108)
        #expect(asked.value == 1)
        #expect(artwork.image(url)?.width == 600)
    }

    @Test func anAskThatGrowsWhileTheBytesDecodeIsAskedAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900002-xl.jpg"
        let bytes = png(width: 460, height: 690)
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: bytes)
        // Before those bytes have decoded at 108.
        artwork.request(url, maxPixelSize: 1800)
        #expect(asked.value == 1, "the first is still on its way")
        try await waitFor { asked.value == 2 }
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url)?.height == 690 }
    }

    @Test func aURLThatFailedWaitsBeforeItIsAskedForAgain() {
        let clock = SteppedClock()
        var asked = 0
        let artwork = ArtworkStore(now: { clock.now }) { _ in asked += 1; return true }
        let url = "https://img.example/cover/900005.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.failed(url: url)
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 1)
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 2, "the first wait is short")
        artwork.failed(url: url)
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 2, "the second is longer")
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 3)
    }

    @Test func anAskWithNoEngineYetIsMadeAgainLater() {
        let engine = Box(false)
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return engine.value }
        artwork.request("https://img.example/cover/900002.jpg", maxPixelSize: 108)
        engine.value = true
        artwork.request("https://img.example/cover/900002.jpg", maxPixelSize: 108)
        #expect(asked.value == 2)
    }

    @Test func anAskWithNoEngineYetIsSentOnceThereIsOne() {
        // A cover already on screen asks once, from its task: it never asks again by itself.
        let engine = Box(false)
        var sent: [String] = []
        let artwork = ArtworkStore { url in
            guard engine.value else { return false }
            sent.append(url)
            return true
        }
        let first = "https://img.example/cover/900002.jpg"
        let second = "https://img.example/cover/900003.jpg"
        artwork.request(first, maxPixelSize: 108)
        artwork.request(second, maxPixelSize: 108)
        artwork.request(first, maxPixelSize: 108)
        #expect(!artwork.hasPending)
        engine.value = true
        artwork.sendUnsent()
        #expect(sent == [first, second], "each once, in the order asked")
        #expect(artwork.isPending(first) && artwork.isPending(second))
        artwork.sendUnsent()
        #expect(sent.count == 2)
    }

    @Test func aURLOnScreenThatFailedIsAskedAgainOnTheBackoff() async throws {
        // A view stays on screen and asks once: its art used to stay a plate until it came back.
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            now: { ContinuousClock.now },
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 1 }
        for times in 2...5 {
            artwork.failed(url: url)
            try await waitFor { asked.value == times }
        }
        // Tries left no more: only showing it again asks again.
        artwork.failed(url: url)
        try await Task.sleep(for: .milliseconds(50))
        #expect(asked.value == 5)
        let waits = slept.all.map { Int(($0 + .milliseconds(500)).components.seconds) }
        #expect(waits == [2, 5, 15, 60])
    }

    @Test func aURLNoLongerOnScreenIsNotAskedAgainByItself() async throws {
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        try await waitFor { asked.value == 1 }
        shown.cancel()
        await shown.value
        artwork.failed(url: url)
        try await Task.sleep(for: .milliseconds(50))
        #expect(asked.value == 1)
        #expect(slept.all.isEmpty)
    }

    @Test func aRetryWaitingIsDroppedWhenTheScreenGoes() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore(sleep: { _ in try await Task.sleep(for: .milliseconds(100)) }) { _ in
            asked.value += 1
            return true
        }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        try await waitFor { asked.value == 1 }
        artwork.failed(url: url)
        shown.cancel()
        await shown.value
        try await Task.sleep(for: .milliseconds(200))
        #expect(asked.value == 1)
    }

    @Test func aScreenShowingAURLThatFailedAMomentAgoAsksOnceTheWaitIsOver() async throws {
        let clock = SteppedClock()
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            now: { clock.now },
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900006.jpg"
        // Failed for the widgets, with no screen showing it.
        artwork.request(url, maxPixelSize: 108)
        artwork.failed(url: url)
        clock.advance(.milliseconds(500))
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 2 }
        #expect(slept.all == [.milliseconds(1500)], "what was left of the wait")
    }

    @Test func artTheCacheLetGoWhileOnScreenIsAskedAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900001.jpg"
        let bytes = png(width: 460, height: 690)
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 1 }
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url) != nil }
        #expect(artwork.arrivals[url] == 1)

        artwork.evict(url)
        #expect(artwork.image(url) == nil)
        // The engine answers from its disk.
        try await waitFor { asked.value == 2 }
        #expect(artwork.isPending(url))
        _ = artwork.image(url)
        try await Task.sleep(for: .milliseconds(20))
        #expect(asked.value == 2, "once")
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url) != nil }
        #expect(artwork.arrivals[url] == 2, "which the view reading it sees")
    }

    @Test func artTheCacheLetGoOffScreenWaitsToBeShownAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900001.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }
        artwork.evict(url)
        #expect(artwork.image(url) == nil)
        try await Task.sleep(for: .milliseconds(20))
        #expect(asked.value == 1)
    }

    @Test func bytesThatAreNoImageDecodeToNothing() {
        #expect(ArtworkStore.decode(Data([0xFF, 0xD8, 0x00, 0x7F]), maxPixelSize: 100) == nil)
    }
}

/// The waits a store's retries asked for, from whichever thread they ask.
final class Slept: Sendable {
    private let waits = Mutex<[Duration]>([])
    func add(_ wait: Duration) { waits.withLock { $0.append(wait) } }
    var all: [Duration] { waits.withLock { $0 } }
}

/// A value the main actor changes from inside a closure.
@MainActor
final class Box<Value> {
    var value: Value
    init(_ value: Value) { self.value = value }
}

/// Wait, a little at a time, until `done` says so.
@MainActor
func waitFor(_ done: () -> Bool, within: Duration = .seconds(5)) async throws {
    let deadline = ContinuousClock.now + within
    while !done() {
        try #require(ContinuousClock.now < deadline, "waited \(within)")
        try await Task.sleep(for: .milliseconds(10))
    }
}
