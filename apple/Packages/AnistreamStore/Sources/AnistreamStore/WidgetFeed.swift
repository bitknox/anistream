// The home-screen widgets' snapshot: asked of the engine when what they show changes, written
// with its art into the App Group container, and the widgets told to read it again.

import AnistreamKit
import AnistreamWidgetData
import CoreGraphics
import Foundation
#if canImport(WidgetKit)
import WidgetKit
#endif

/// Where the widgets' art comes from: the engine's artwork, decoded.
public protocol WidgetArtSource {
    /// The image at `url`, decoded to at most `maxPixelSize` on its long side; nil when it could
    /// not be had.
    @MainActor func images(for urls: [String], maxPixelSize: Int) async -> [String: CGImage]
}

/// Keeps the widgets' snapshot current.
///
/// In the foreground, the store calls `changed()` when Continue, the Calendar, a playback or a
/// sync moves, and the snapshot is asked for once things settle; `received(_:)` writes what the
/// engine answers. A background task writes the snapshot it asked for itself with
/// `write(_:art:)`, and so does anything else that holds a `WidgetData`.
///
/// Nothing is written where the bundle names no App Group: the Fixtures app, the tests and the
/// dev harness, so none of them ever writes over the app's.
@MainActor
public final class WidgetFeed {
    /// Where the snapshot goes.
    public let files: WidgetFiles?
    private let reload: @MainActor () -> Void
    /// How long changes are gathered before one snapshot is asked for.
    public static let settle: Duration = .seconds(2)
    /// At most this many art files are fetched for one snapshot.
    static let artLimit = 60
    private var asking: Task<Void, Never>?
    private var writing: Task<Void, Never>?

    public init(files: WidgetFiles? = WidgetFiles.shared(), reload: @escaping @MainActor () -> Void = WidgetFeed.reloadWidgets) {
        self.files = files
        self.reload = reload
    }

    /// Whether there is anywhere to write.
    public var isEnabled: Bool { files != nil }

    /// Tell every widget to read the snapshot again.
    public static func reloadWidgets() {
        #if canImport(WidgetKit)
        WidgetCenter.shared.reloadAllTimelines()
        #endif
    }

    /// Something the widgets show changed: `ask` for a snapshot once nothing has changed for
    /// `settle`.
    public func changed(ask: @escaping @MainActor () -> Void) {
        guard isEnabled else { return }
        asking?.cancel()
        asking = Task {
            try? await Task.sleep(for: Self.settle)
            guard !Task.isCancelled else { return }
            ask()
        }
    }

    /// The engine's snapshot, written once the one before it is.
    public func received(_ data: WidgetData, art: some WidgetArtSource) {
        guard isEnabled else { return }
        let previous = writing
        writing = Task {
            await previous?.value
            try? await self.write(data, art: art)
        }
    }

    /// What a background refresh writes its record with: this feed, the art from `art`. `nil`
    /// where there is nowhere to write.
    public func writer(art: some WidgetArtSource & Sendable) -> NoticeRefresh.WidgetWriter? {
        guard isEnabled else { return nil }
        return { [weak self] data in try await self?.write(data, art: art) }
    }

    /// Wait for the snapshot being written, if one is.
    public func finished() async {
        await writing?.value
    }

    /// Write `data` and its art into the App Group container, then reload the widgets. The art
    /// it has from an earlier snapshot is kept, and the art no snapshot names is deleted.
    public func write(_ data: WidgetData, art: some WidgetArtSource) async throws {
        guard let files else { return }
        var names: [String: String] = [:]
        let wanted = Self.art(in: data).filter { url, kind in
            let name = WidgetFiles.artName(for: url, kind: kind)
            if files.hasArt(name) {
                names[url] = name
                return false
            }
            return true
        }
        for kind in ArtKind.allCases {
            let urls = wanted.filter { $0.kind == kind }.map(\.url)
            guard !urls.isEmpty else { continue }
            let size = kind == .wide ? 1_600 : 400
            let images = await art.images(for: urls, maxPixelSize: size)
            for (url, image) in images {
                let name = WidgetFiles.artName(for: url, kind: kind)
                guard let jpeg = ArtThumbnail.jpeg(from: image, kind: kind) else { continue }
                try? files.writeArt(jpeg, named: name)
                names[url] = name
            }
        }
        try files.write(Self.snapshot(data, art: names))
        reload()
    }

    /// The art a snapshot shows, the most needed first, at most `artLimit`: Continue's banners
    /// and covers, then each broadcast's banner, or its cover without one, today's in your list
    /// before the rest of today's and the next days' in your list.
    static func art(in data: WidgetData) -> [(url: String, kind: ArtKind)] {
        var wanted: [(url: String, kind: ArtKind)] = []
        var seen = Set<String>()
        func add(_ url: String?, _ kind: ArtKind) {
            guard let url, !url.isEmpty, seen.insert(url).inserted else { return }
            wanted.append((url, kind))
        }
        for item in data.continueWatching {
            add(item.bannerUrl, .wide)
            add(item.coverUrl, .poster)
        }
        func row(_ airing: WidgetAiring) {
            if let banner = airing.bannerUrl { add(banner, .wide) } else { add(airing.coverUrl, .poster) }
        }
        data.airingToday.filter(\.onList).forEach(row)
        data.airingToday.filter { !$0.onList }.forEach(row)
        data.nextDays.filter(\.onList).forEach(row)
        return Array(wanted.prefix(artLimit))
    }

    /// The engine's record as the widgets read it, with the art files that are there.
    static func snapshot(_ data: WidgetData, art names: [String: String]) -> WidgetSnapshot {
        func art(banner: String?, cover: String?, color: String?) -> Art {
            Art(wide: banner.flatMap { names[$0] }, poster: cover.flatMap { names[$0] }, color: color)
        }
        func airing(_ row: WidgetAiring) -> SnapshotAiring {
            SnapshotAiring(
                id: Int(row.id), title: row.title, episode: Int(row.episode),
                airsAt: Date(timeIntervalSince1970: TimeInterval(row.airsAt)),
                onList: row.onList, watched: row.watched,
                translation: row.translation.map { $0 == .dub ? .dub : .sub },
                art: art(banner: row.bannerUrl, cover: row.coverUrl, color: row.coverColor))
        }
        return WidgetSnapshot(
            version: WidgetSnapshot.newestVersion,
            generatedAt: Date(timeIntervalSince1970: TimeInterval(data.generatedAt)),
            signedIn: data.signedIn,
            tracker: data.tracker,
            continueWatching: data.continueWatching.map { item in
                SnapshotContinue(
                    id: Int(item.id), title: item.title, episode: Int(item.episode),
                    watched: Int(item.watched), episodes: item.episodes.map(Int.init),
                    fraction: item.fraction, episodeTitle: item.episodeTitle, resume: item.resume,
                    positionSecs: item.positionSecs.map(Int.init), durationSecs: item.durationSecs.map(Int.init),
                    minutesLeft: item.minutesLeft.map(Int.init),
                    nextAiring: item.nextAiring.map { Date(timeIntervalSince1970: TimeInterval($0)) },
                    nextAiringEpisode: item.nextAiringEpisode.map(Int.init),
                    art: art(banner: item.bannerUrl, cover: item.coverUrl, color: item.coverColor))
            },
            airingToday: data.airingToday.map(airing),
            nextDays: data.nextDays.map(airing),
            nextAiring: data.nextAiring.map {
                SnapshotNextAiring(
                    id: Int($0.id), title: $0.title, episode: Int($0.episode),
                    airsAt: Date(timeIntervalSince1970: TimeInterval($0.airsAt)))
            })
    }
}

extension EngineQuestions: WidgetArtSource {
    /// Asks the engine for each of `urls` at once, as a background refresh on an engine of its
    /// own does, with no `ArtworkStore` there: ten seconds at most each.
    public func images(for urls: [String], maxPixelSize: Int) async -> [String: CGImage] {
        let fetched = await withTaskGroup(of: (String, Data?).self) { group in
            for url in urls { group.addTask { (url, await self.art(url)) } }
            var fetched: [String: Data] = [:]
            for await (url, bytes) in group { fetched[url] = bytes }
            return fetched
        }
        return fetched.compactMapValues { ArtworkStore.decodeWithSource($0, maxPixelSize: maxPixelSize)?.image }
    }
}

extension ArtworkStore: WidgetArtSource {
    /// Asks the engine for every one of `urls` at once, then waits for each, a quarter of a minute
    /// at most in all.
    public func images(for urls: [String], maxPixelSize: Int) async -> [String: CGImage] {
        for url in urls { request(url, maxPixelSize: maxPixelSize) }
        let deadline = ContinuousClock.now + .seconds(15)
        while urls.contains(where: isPending), ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }
        var images: [String: CGImage] = [:]
        for url in urls {
            if let image = image(url) { images[url] = image }
        }
        return images
    }
}
