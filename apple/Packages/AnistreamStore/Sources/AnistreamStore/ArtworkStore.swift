// Covers and banners: asked of the engine once per URL, decoded off the main thread, and kept
// decoded in memory for as long as the system lets them be.

import AnistreamKit
import CoreGraphics
import Foundation
import ImageIO
import Observation

/// A decoded cover or banner.
public final class Artwork: @unchecked Sendable {
    // A CGImage is immutable once made; the class carries it across actors and into NSCache.
    public let image: CGImage

    init(_ image: CGImage) {
        self.image = image
    }

    /// What it costs to keep: its pixels.
    var cost: Int { image.bytesPerRow * image.height }
}

/// The artwork the screens show.
///
/// A screen asks with `request(_:maxPixelSize:)` when art comes on screen and reads it with
/// `image(_:)`. Each URL is fetched once: a second ask while the first is on its way, or once the
/// art is here, sends nothing. The engine answers with the file's bytes (`ArtworkReady`), which
/// are downsampled with ImageIO to the largest size any screen asked for, off the main thread, and
/// kept in an `NSCache`. A URL that failed is asked for again only after `retryAfter`; its plate
/// stays meanwhile.
@Observable @MainActor
public final class ArtworkStore {
    /// How long a URL that failed waits before it is asked for again.
    public static let retryAfter: Duration = .seconds(60)

    /// Bumped for a URL each time its art lands, which is what a view reading it observes.
    public private(set) var arrivals: [String: Int] = [:]

    @ObservationIgnored private let cache: NSCache<NSString, Artwork>
    @ObservationIgnored private var inFlight: Set<String> = []
    @ObservationIgnored private var sizes: [String: Int] = [:]
    @ObservationIgnored private var failed: [String: ContinuousClock.Instant] = [:]
    @ObservationIgnored private let fetch: @MainActor (String) -> Bool
    @ObservationIgnored private let now: @Sendable () -> ContinuousClock.Instant

    /// - Parameters:
    ///   - fetch: asks the engine for a URL (`FetchArtwork`); false when there is no engine to
    ///     ask yet, so a later ask tries again.
    ///   - costLimit: how many bytes of decoded pixels to keep.
    public init(
        costLimit: Int = 256 << 20,
        now: @escaping @Sendable () -> ContinuousClock.Instant = { ContinuousClock.now },
        fetch: @escaping @MainActor (String) -> Bool
    ) {
        cache = NSCache()
        cache.totalCostLimit = costLimit
        self.fetch = fetch
        self.now = now
    }

    /// The art for `url`, when it is here.
    public func image(_ url: String) -> CGImage? {
        _ = arrivals[url]
        return cache.object(forKey: url as NSString)?.image
    }

    /// Ask for `url`'s art, decoded to at most `maxPixelSize` pixels on its long side.
    public func request(_ url: String, maxPixelSize: Int) {
        sizes[url] = max(sizes[url] ?? 0, maxPixelSize)
        guard cache.object(forKey: url as NSString) == nil, !inFlight.contains(url) else { return }
        if let at = failed[url], now() - at < Self.retryAfter { return }
        if fetch(url) {
            inFlight.insert(url)
        }
    }

    /// Whether `url` has been asked for and not answered.
    public func isPending(_ url: String) -> Bool {
        inFlight.contains(url)
    }

    func ready(url: String, bytes: Data) {
        let size = sizes[url] ?? 1024
        Task.detached(priority: .utility) { [weak self] in
            let image = Self.decode(bytes, maxPixelSize: size)
            await self?.decoded(url: url, image: image)
        }
    }

    func failed(url: String) {
        inFlight.remove(url)
        failed[url] = now()
    }

    private func decoded(url: String, image: CGImage?) {
        inFlight.remove(url)
        guard let image else {
            failed[url] = now()
            return
        }
        failed[url] = nil
        let artwork = Artwork(image)
        cache.setObject(artwork, forKey: url as NSString, cost: artwork.cost)
        arrivals[url, default: 0] += 1
    }

    /// `bytes` decoded and downsampled to at most `maxPixelSize` on the long side, with the
    /// file's orientation applied; `nil` when they are not an image.
    public nonisolated static func decode(_ bytes: Data, maxPixelSize: Int) -> CGImage? {
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
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }
}
