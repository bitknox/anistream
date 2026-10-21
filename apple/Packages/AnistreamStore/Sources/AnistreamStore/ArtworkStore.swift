// Covers and banners: asked of the engine once per URL, decoded off the main thread, and kept
// decoded in memory for as long as the system lets them be.

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
/// A screen asks with `request(_:maxPixelSize:)` when art comes on screen and reads it with
/// `image(_:)`. The engine answers with the file's bytes (`ArtworkReady`), which are downsampled
/// with ImageIO to the largest size any screen has asked for, off the main thread, and kept in an
/// `NSCache`, one decode per URL, the largest.
///
/// A URL is asked of the engine once while it is on its way, and not again while its decode is
/// enough. A larger ask than the decode answers, where the file is larger too, asks again (the
/// engine has the file on disk by then), and the smaller decode shows until the larger lands: a
/// cover a list row decoded at 108 pixels is never what a banner stretches across 1,200 points.
/// A URL that failed is asked for again only after `retryAfter`; its plate stays meanwhile.
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
        if let cached = cache.object(forKey: url as NSString), cached.covers(maxPixelSize) { return }
        ask(url)
    }

    /// Ask the engine for `url`, unless it is on its way or failed a moment ago.
    private func ask(_ url: String) {
        guard !inFlight.contains(url) else { return }
        if let at = failed[url], now() - at < Self.retryAfter { return }
        if fetch(url) {
            inFlight.insert(url)
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

    func ready(url: String, bytes: Data) {
        let size = sizes[url] ?? 1024
        Task.detached(priority: .utility) { [weak self] in
            let decoded = Self.decodeWithSource(bytes, maxPixelSize: size)
            await self?.decoded(url: url, size: size, decoded: decoded)
        }
    }

    func failed(url: String) {
        inFlight.remove(url)
        failed[url] = now()
    }

    private func decoded(url: String, size: Int, decoded: (image: CGImage, sourceLongSide: Int)?) {
        inFlight.remove(url)
        let cached = cache.object(forKey: url as NSString)
        guard let decoded else {
            if cached == nil { failed[url] = now() }
            return
        }
        failed[url] = nil
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
