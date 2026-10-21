// Spotlight: the titles on your Library lists and on Continue, found from the system's search,
// each opening its page. Kept as those lists change: a title that leaves them leaves the index.

import AnistreamKit
import AnistreamStore
import CoreGraphics
import CoreSpotlight
import Foundation
import ImageIO
import Observation
import UniformTypeIdentifiers

/// One title as Spotlight is given it (native-spec/native-layer.md, Spotlight).
public struct SpotlightItem: Equatable, Sendable {
    public var identifier: String
    /// The list it is on: `watching`, `continue`, after the app's prefix.
    public var domain: String
    public var title: String
    /// Its other names: the other-language title first.
    public var alternateNames: [String]
    /// The line under it: the list and the episode, "Watching · Episode 7 of 24".
    public var line: String
    /// Its genres, format and year.
    public var keywords: [String]
    /// The cover, 120 by 180, as JPEG.
    public var thumbnail: Data?

    public init(_ title: ShelfTitle, prefix: String, thumbnail: Data? = nil) {
        identifier = Self.identifier(title.id)
        domain = prefix + (title.list ?? .continue).rawValue
        self.title = title.title
        alternateNames = title.otherTitles
        line = title.line
        keywords = title.keywords
        self.thumbnail = thumbnail
    }

    /// Every domain the app's titles go under.
    public static func domains(prefix: String) -> [String] {
        Shelf.Source.allCases.map { prefix + $0.rawValue }
    }

    /// A title's identifier in the index: `title/900001`.
    public static func identifier(_ id: AnilistId) -> String {
        "title/\(id)"
    }

    /// The title an identifier names.
    public static func title(_ identifier: String) -> AnilistId? {
        guard identifier.hasPrefix("title/") else { return nil }
        return AnilistId(identifier.dropFirst("title/".count))
    }

    /// The title a Spotlight result the app was opened from names.
    public static func title(in activity: NSUserActivity) -> AnilistId? {
        guard activity.activityType == CSSearchableItemActionType,
              let identifier = activity.userInfo?[CSSearchableItemActivityIdentifier] as? String
        else { return nil }
        return title(identifier)
    }

    /// The item as CoreSpotlight takes it. It does not expire: it stays while the title is on
    /// the shelf, and goes when the title leaves.
    public var searchable: CSSearchableItem {
        let attributes = CSSearchableItemAttributeSet(contentType: .content)
        attributes.title = title
        attributes.displayName = title
        attributes.alternateNames = alternateNames
        attributes.contentDescription = line.isEmpty ? nil : line
        attributes.keywords = keywords
        attributes.thumbnailData = thumbnail
        let item = CSSearchableItem(uniqueIdentifier: identifier, domainIdentifier: domain, attributeSet: attributes)
        item.expirationDate = .distantFuture
        return item
    }
}

/// Where the indexer finds covers: the store's artwork, or a test's.
@MainActor
public protocol SpotlightCovers: AnyObject {
    /// The cover at `url`, when it is here.
    func image(_ url: String) -> CGImage?
    /// Ask for the cover at `url`, small.
    func request(cover url: String)
    /// Whether the cover at `url` has been asked for and not answered.
    func isPending(_ url: String) -> Bool
}

extension ArtworkStore: SpotlightCovers {
    public func request(cover url: String) {
        request(url, maxPixelSize: SpotlightIndexer.coverPixels)
    }
}

/// What the indexer asks of Spotlight, so the tests can stand in for it.
public protocol SpotlightIndex: Sendable {
    func index(_ items: [SpotlightItem]) async throws
    func delete(_ identifiers: [String]) async throws
    func deleteAll(domains: [String]) async throws
}

/// The app's own index in Spotlight.
public struct SystemSpotlightIndex: SpotlightIndex {
    public init() {}

    public func index(_ items: [SpotlightItem]) async throws {
        let searchable = items.map(\.searchable)
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().indexSearchableItems(searchable) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }

    public func delete(_ identifiers: [String]) async throws {
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().deleteSearchableItems(withIdentifiers: identifiers) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }

    public func deleteAll(domains: [String]) async throws {
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().deleteSearchableItems(withDomainIdentifiers: domains) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }
}

/// Keeps Spotlight's titles the shelf's: indexes the titles that join it or change, with their
/// covers once those are here, and removes the ones that leave.
///
/// It watches the store's lists and titles, and a moment after they settle compares the shelf
/// with the one it saved: only what differs goes to Spotlight. With nothing saved (a first
/// launch, or the Fixtures app's every launch) it clears its domains first, so nothing indexed
/// before outlives it. "Show titles in Spotlight" off clears them and indexes nothing.
@MainActor
public final class SpotlightIndexer {
    /// The switch, in the app's defaults: on unless turned off.
    public static let preference = "anistream.showInSpotlight"
    /// The long side of the cover in the index, in pixels.
    nonisolated static let coverPixels = 180
    /// How many covers are asked for at once.
    static let coversAtOnce = 8

    private let store: AppStore
    private let index: any SpotlightIndex
    /// What the index's domains start with: nothing for the app, whose domains are its lists'
    /// names, and `fixtures.` for the Fixtures app, so its titles never mix with the app's.
    public let prefix: String
    private let file: URL?
    private let defaults: UserDefaults
    private let delay: Duration
    private let covers: any SpotlightCovers
    private var saved: Shelf?
    /// Whether the domain was cleared this session, for a start with nothing saved.
    private var cleared = false
    private var scheduled: Task<Void, Never>?
    private var running = false
    private var again = false
    /// The covers asked for and not here yet.
    private var asked: Set<String> = []
    /// The covers that did not come, not asked for again this session.
    private var missed: Set<String> = []
    private var defaultsObserver: (any NSObjectProtocol)?
    /// The newest watch on the store.
    private var generation = 0
    /// Called after Spotlight was told of a change to the shelf.
    public var onUpdate: (@MainActor () -> Void)?

    /// - Parameters:
    ///   - file: where the shelf is saved; `nil` keeps nothing between launches.
    ///   - delay: how long the lists stay still before Spotlight is told.
    ///   - covers: where covers come from; the store's artwork by default.
    public init(
        store: AppStore,
        index: any SpotlightIndex = SystemSpotlightIndex(),
        prefix: String = SpotlightIndexer.prefix(),
        file: URL?,
        defaults: UserDefaults = .standard,
        delay: Duration = .seconds(1),
        covers: (any SpotlightCovers)? = nil
    ) {
        self.store = store
        self.index = index
        self.prefix = prefix
        self.file = file
        self.defaults = defaults
        self.delay = delay
        self.covers = covers ?? store.artwork
        defaults.register(defaults: [Self.preference: true])
        saved = file.flatMap(Shelf.load)
    }

    /// The prefix for this app: none for `tv.anistream.app`, and the rest of another's bundle
    /// id after `tv.anistream.`: `fixtures.`.
    public nonisolated static func prefix(bundle: Bundle = .main) -> String {
        prefix(bundleIdentifier: bundle.bundleIdentifier)
    }

    nonisolated static func prefix(bundleIdentifier: String?) -> String {
        guard let id = bundleIdentifier, id != "tv.anistream.app" else { return "" }
        let name = id.hasPrefix("tv.anistream.") ? String(id.dropFirst("tv.anistream.".count)) : id
        return name + "."
    }

    /// Whether titles go to Spotlight.
    public var isOn: Bool { defaults.bool(forKey: Self.preference) }

    /// The shelf as last told to Spotlight.
    public var shelf: Shelf? { saved }

    /// Watch the store, and the switch, from now on.
    public func start() {
        if CSSearchableIndex.isIndexingAvailable() == false {
            AppLog.info("spotlight", "indexing is not available here")
            return
        }
        observe()
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: defaults, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.schedule() }
        }
        schedule()
    }

    /// Note what the shelf is made of, and the covers on their way, and look again once any of
    /// it changes. Only the newest watch looks again, so watches do not pile up.
    private func observe() {
        generation += 1
        let watch = generation
        withObservationTracking {
            _ = Shelf.current(store, over: saved ?? Shelf())
            for url in asked { _ = covers.image(url) }
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self, self.generation == watch else { return }
                self.schedule()
                self.observe()
            }
        }
    }

    /// Tell Spotlight once the lists have been still for `delay`.
    func schedule() {
        scheduled?.cancel()
        scheduled = Task { [weak self, delay] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            await self?.pass()
        }
    }

    /// Bring Spotlight to the shelf as it stands. One at a time: a change during a pass is
    /// taken by another pass right after it.
    func pass() async {
        guard !running else {
            again = true
            return
        }
        running = true
        defer { running = false }
        repeat {
            again = false
            await update()
        } while again
        // The covers asked for in the pass are watched from now on.
        observe()
    }

    private func update() async {
        guard isOn else {
            if saved != nil || !cleared {
                do {
                    try await index.deleteAll(domains: SpotlightItem.domains(prefix: prefix))
                    cleared = true
                    saved = nil
                    if let file { Shelf.remove(file) }
                    onUpdate?()
                } catch {
                    AppLog.error("spotlight", "could not clear the index: \(error.localizedDescription)")
                }
            }
            return
        }
        // Nothing an answered list says yet, and nothing saved: wait, rather than clear.
        let before = saved ?? Shelf()
        var shelf = Shelf.current(store, over: before)
        if saved == nil, shelf.titles.isEmpty, !store.continuing.list.state.isAnswered { return }

        do {
            if saved == nil, !cleared {
                try await index.deleteAll(domains: SpotlightItem.domains(prefix: prefix))
                cleared = true
            }
            let gone = Set(before.titles.keys).subtracting(shelf.titles.keys)
            if !gone.isEmpty {
                try await index.delete(gone.sorted().map(SpotlightItem.identifier))
            }
            var items: [SpotlightItem] = []
            var thumbnails: [AnilistId: Data] = [:]
            for id in shelf.order {
                guard let title = shelf.titles[id] else { continue }
                let changed = saved == nil || before.titles[id] != title
                // Indexed again, an item is replaced whole: its cover goes with it again.
                if changed { shelf.covered.remove(id) }
                var thumbnail: Data?
                if !shelf.covered.contains(id), let url = title.coverUrl, !missed.contains(url) {
                    if let image = covers.image(url) {
                        thumbnail = await Self.jpeg(Picture(image: image))
                        thumbnails[id] = thumbnail
                        asked.remove(url)
                    } else if asked.contains(url) {
                        // Answered with nothing: the plate stays, and it is not asked again.
                        if !covers.isPending(url) {
                            asked.remove(url)
                            missed.insert(url)
                        }
                    } else if asked.count < Self.coversAtOnce {
                        asked.insert(url)
                        covers.request(cover: url)
                    }
                }
                guard changed || thumbnail != nil else { continue }
                items.append(SpotlightItem(title, prefix: prefix, thumbnail: thumbnail))
            }
            if !items.isEmpty {
                try await index.index(items)
            }
            shelf.covered.formUnion(thumbnails.keys)
            saved = shelf
            if let file { try shelf.save(file) }
            if !items.isEmpty || !gone.isEmpty {
                AppLog.info("spotlight", "indexed \(items.count), removed \(gone.count), \(shelf.titles.count) in all")
                onUpdate?()
            }
        } catch {
            AppLog.error("spotlight", "could not update the index: \(error.localizedDescription)")
        }
    }

    /// A cover drawn for the index: JPEG, its long side `coverPixels` at most.
    @concurrent nonisolated static func jpeg(_ picture: Picture) async -> Data? {
        let image = picture.image
        let scale = min(1, Double(coverPixels) / Double(max(image.width, image.height)))
        let width = max(1, Int((Double(image.width) * scale).rounded()))
        let height = max(1, Int((Double(image.height) * scale).rounded()))
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
        ) else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let small = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destination, small, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    /// A decoded cover, carried off the main actor to be drawn small. A CGImage does not change
    /// once made.
    struct Picture: @unchecked Sendable {
        let image: CGImage
    }
}

extension ListState {
    /// Whether the engine has answered: with titles, or that there are none.
    var isAnswered: Bool {
        switch self {
        case .loaded, .empty: true
        case .idle, .loading, .failed: false
        }
    }
}
