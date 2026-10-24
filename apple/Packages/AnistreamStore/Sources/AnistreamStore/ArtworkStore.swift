// Covers and banners: asked of the engine once per URL, decoded off the main thread, kept
// decoded in memory for as long as the system lets them be, and asked again while on screen.

import AnistreamKit
import CoreGraphics
import Foundation
import ImageIO
import Observation

/// A decoded cover or banner, and what it was decoded from.
public final class Artwork: @unchecked Sendable {
    // A CGImage is immutable once made; the class carries it across actors and into NSCache.
    public let image: CGImage
    /// The long side the file was decoded down to at most: the largest any screen had asked for.
    let decodedFor: Int
    /// The long side of the image in the file.
    let sourceLongSide: Int

    init(_ image: CGImage, decodedFor: Int, sourceLongSide: Int) {
        self.image = image
        self.decodedFor = decodedFor
        self.sourceLongSide = sourceLongSide
    }

    /// The long side as decoded.
    var longSide: Int { max(image.width, image.height) }

    /// Whether this decode is enough for a screen that needs `size` pixels: it was decoded for at
    /// least that many, or the file has no more to give.
    func covers(_ size: Int) -> Bool {
        decodedFor >= size || longSide >= sourceLongSide
    }

    /// What it costs to keep: its pixels.
    var cost: Int { image.bytesPerRow * image.height }
}

/// The artwork the screens show.
///
/// A screen shows art with `show(_:maxPixelSize:)`, from a task that runs while the art is on
/// screen, and reads it with `image(_:)`. Anything else asks with `request(_:maxPixelSize:)`. The
/// engine answers with the file's bytes (`ArtworkReady`), which are downsampled with ImageIO to
/// the largest size any screen has asked for, off the main thread, and kept in an `NSCache`, one
/// decode per URL, the largest.
///
/// A URL is asked of the engine once while it is on its way, and not again while its decode is
/// enough. A larger ask than the decode answers, where the file is larger too, asks again (the
/// engine has the file on disk by then), and the smaller decode shows until the larger lands: a
/// cover a list row decoded at 108 pixels is never what a banner stretches across 1,200 points.
///
/// Art stays wanted for as long as a screen shows it, and a screen never has to ask twice:
/// - asked for before there is an engine, it is asked once the engine is there
///   (`sendUnsent()`);
/// - failed, it is asked again after each of `backoff` while it is on screen, and after that only
///   when a screen shows it again, the last of them later; its plate stays meanwhile;
/// - let go by the cache while on screen, it is asked again, which the engine answers from disk.
@Observable @MainActor
public final class ArtworkStore {
    /// How long a URL that failed waits before it is asked for again, after its first failure,
    /// its second and so on: while a screen shows it, it is asked again after each, and after the
    /// last only when a screen shows it again, the last wait later.
    public static let backoff: [Duration] = [.seconds(2), .seconds(5), .seconds(15), .seconds(60)]

    /// Bumped for a URL each time its art lands, which is what a view reading it observes.
    public private(set) var arrivals: [String: Int] = [:]

    @ObservationIgnored private let cache: NSCache<NSString, Artwork>
    @ObservationIgnored private var inFlight: Set<String> = []
    /// Asked for while there was no engine to ask, in the order asked.
    @ObservationIgnored private var unsent: [String] = []
    @ObservationIgnored private var sizes: [String: Int] = [:]
    /// How many times in a row each URL failed, and when it last did.
    @ObservationIgnored private var failures: [String: (count: Int, at: ContinuousClock.Instant)] = [:]
    /// How many screens show each URL now.
    @ObservationIgnored private var shown: [String: Int] = [:]
    /// The URLs decoded at least once, which the engine has on disk.
    @ObservationIgnored private var decodedOnce: Set<String> = []
    /// The retries waiting, by URL.
    @ObservationIgnored private var retries: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private let fetch: @MainActor (String) -> Bool
    @ObservationIgnored private let now: @Sendable () -> ContinuousClock.Instant
    @ObservationIgnored private let sleep: @Sendable (Duration) async throws -> Void
    @ObservationIgnored private let backoff: [Duration]

    /// - Parameters:
    ///   - fetch: asks the engine for a URL (`FetchArtwork`); false when there is no engine to
    ///     ask yet, so the ask waits for `sendUnsent()`.
    ///   - costLimit: how many bytes of decoded pixels to keep.
    ///   - backoff: the waits after each failure in a row (`ArtworkStore.backoff`).
    ///   - sleep: how a retry waits out its part of the backoff.
    public init(
        costLimit: Int = 256 << 20,
        backoff: [Duration] = ArtworkStore.backoff,
        now: @escaping @Sendable () -> ContinuousClock.Instant = { ContinuousClock.now },
        sleep: @escaping @Sendable (Duration) async throws -> Void = { try await Task.sleep(for: $0) },
        fetch: @escaping @MainActor (String) -> Bool
    ) {
        cache = NSCache()
        cache.totalCostLimit = costLimit
        self.fetch = fetch
        self.now = now
        self.sleep = sleep
        self.backoff = backoff.isEmpty ? Self.backoff : backoff
    }

    /// The art for `url`, when it is here.
    public func image(_ url: String) -> CGImage? {
        _ = arrivals[url]
        if let artwork = cache.object(forKey: url as NSString) { return artwork.image }
        if shown[url] != nil, decodedOnce.contains(url), !inFlight.contains(url) {
            // The cache let it go while a screen still shows it. Asked again outside the view's
            // update, which reads this.
            Task { [weak self] in self?.askAgain(url) }
        }
        return nil
    }

    /// Show `url`'s art, decoded to at most `maxPixelSize` pixels on its long side, for as long as
    /// the calling task runs: what a view calls from `.task(id:)`, which ends as the view goes.
    /// Asked for now, and while the task runs, asked for again whenever it failed or the cache
    /// let it go.
    public func show(_ url: String, maxPixelSize: Int) async {
        shown[url, default: 0] += 1
        defer { hide(url) }
        request(url, maxPixelSize: maxPixelSize)
        scheduleRetry(url)
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(3600))
        }
    }

    /// Ask for `url`'s art, decoded to at most `maxPixelSize` pixels on its long side.
    public func request(_ url: String, maxPixelSize: Int) {
        sizes[url] = max(sizes[url] ?? 0, maxPixelSize)
        if let cached = cache.object(forKey: url as NSString), cached.covers(maxPixelSize) { return }
        ask(url)
    }

    /// Send what was asked for while there was no engine: once there is one.
    public func sendUnsent() {
        let asks = unsent
        unsent = []
        for url in asks where !covered(url) {
            send(url)
        }
    }

    /// Whether `url` has been asked for and not answered.
    public func isPending(_ url: String) -> Bool {
        inFlight.contains(url)
    }

    /// Whether any art has been asked for and not answered.
    public var hasPending: Bool {
        !inFlight.isEmpty
    }

    /// Whether the decode here is enough for the largest size asked for `url`.
    private func covered(_ url: String) -> Bool {
        guard let cached = cache.object(forKey: url as NSString) else { return false }
        return cached.covers(sizes[url] ?? 0)
    }

    /// Ask the engine for `url`, unless it is on its way, or failed and its wait is not over.
    private func ask(_ url: String) {
        if let failure = failures[url], now() - failure.at < wait(after: failure.count) { return }
        send(url)
    }

    /// Ask the engine for `url` now, unless it is on its way.
    private func send(_ url: String) {
        guard !inFlight.contains(url) else { return }
        if fetch(url) {
            inFlight.insert(url)
        } else if !unsent.contains(url) {
            unsent.append(url)
        }
    }

    /// What `url` waits after failing `count` times in a row.
    private func wait(after count: Int) -> Duration {
        backoff[min(max(count, 1), backoff.count) - 1]
    }

    /// The cache let `url` go while a screen shows it: the engine still has the file.
    private func askAgain(_ url: String) {
        guard shown[url] != nil, cache.object(forKey: url as NSString) == nil else { return }
        send(url)
    }

    /// When `url` is on screen and failed, with retries left, ask for it again once its wait is
    /// over.
    private func scheduleRetry(_ url: String) {
        guard shown[url] != nil, retries[url] == nil, let failure = failures[url],
              failure.count <= backoff.count
        else { return }
        let left = wait(after: failure.count) - (now() - failure.at)
        let sleep = self.sleep
        retries[url] = Task { [weak self] in
            do { try await sleep(max(left, .zero)) } catch { return }
            self?.retry(url)
        }
    }

    private func retry(_ url: String) {
        retries[url] = nil
        guard shown[url] != nil, !covered(url) else { return }
        send(url)
    }

    /// A screen showing `url` went.
    private func hide(_ url: String) {
        guard let count = shown[url] else { return }
        if count > 1 {
            shown[url] = count - 1
        } else {
            shown[url] = nil
            retries.removeValue(forKey: url)?.cancel()
        }
    }

    private func didFail(_ url: String) {
        failures[url] = (count: (failures[url]?.count ?? 0) + 1, at: now())
        scheduleRetry(url)
    }

    /// Let the cache drop `url`'s decode, as the system may at any time.
    func evict(_ url: String) {
        cache.removeObject(forKey: url as NSString)
    }

    func ready(url: String, bytes: Data) {
        let size = sizes[url] ?? 1024
        Task.detached(priority: .utility) { [weak self] in
            let decoded = Self.decodeWithSource(bytes, maxPixelSize: size)
            await self?.decoded(url: url, size: size, decoded: decoded)
        }
    }

    func failed(url: String) {
        inFlight.remove(url)
        didFail(url)
    }

    private func decoded(url: String, size: Int, decoded: (image: CGImage, sourceLongSide: Int)?) {
        inFlight.remove(url)
        let cached = cache.object(forKey: url as NSString)
        guard let decoded else {
            if cached == nil { didFail(url) }
            return
        }
        failures[url] = nil
        decodedOnce.insert(url)
        let artwork = Artwork(decoded.image, decodedFor: size, sourceLongSide: decoded.sourceLongSide)
        // The largest decode is the one kept: a smaller one answering late changes nothing.
        if cached.map({ $0.longSide < artwork.longSide }) ?? true {
            cache.setObject(artwork, forKey: url as NSString, cost: artwork.cost)
            arrivals[url, default: 0] += 1
        }
        // A screen asked for more while these bytes were decoding.
        if let wanted = sizes[url], !(cache.object(forKey: url as NSString)?.covers(wanted) ?? false) {
            ask(url)
        }
    }

    /// `bytes` decoded and downsampled to at most `maxPixelSize` on the long side, with the
    /// file's orientation applied; `nil` when they are not an image.
    public nonisolated static func decode(_ bytes: Data, maxPixelSize: Int) -> CGImage? {
        decodeWithSource(bytes, maxPixelSize: maxPixelSize)?.image
    }

    /// The same, with the long side of the image in the file.
    nonisolated static func decodeWithSource(
        _ bytes: Data,
        maxPixelSize: Int
    ) -> (image: CGImage, sourceLongSide: Int)? {
        let source = CGImageSourceCreateWithData(
            bytes as CFData, [kCGImageSourceShouldCache: false] as CFDictionary
        )
        guard let source else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: max(1, maxPixelSize),
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
        else { return nil }
        let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        let width = properties?[kCGImagePropertyPixelWidth] as? Int ?? image.width
        let height = properties?[kCGImagePropertyPixelHeight] as? Int ?? image.height
        return (image, max(width, height))
    }
}
