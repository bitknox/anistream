// The titles a person keeps: the ones on their Library lists and on Continue. Spotlight indexes
// them, and the shelf is saved, so a list not yet shown this session still counts, and a title
// that left a list is known to have left.

import AnistreamKit
import AnistreamStore
import AnistreamStrings
import Foundation

/// A title on the shelf, as the system's search shows it (native-spec/native-layer.md, Spotlight).
public struct ShelfTitle: Codable, Hashable, Sendable, Identifiable {
    public var id: AnilistId
    /// The title as the screens head it.
    public var title: String
    /// Its other names, romaji, English and native, without the one it is headed by.
    public var otherTitles: [String]
    public var year: UInt16?
    /// AniList's format, as it gives it: "TV", "MOVIE".
    public var format: String?
    public var coverUrl: String?
    public var genres: [String]
    /// The list it is on: a Library list, or Continue alone. `nil` for a title on neither.
    public var list: Shelf.Source?
    /// The episode it plays next, and how many it has, when known.
    public var episode: UInt32?
    public var episodes: UInt32?

    public init(
        id: AnilistId, title: String, otherTitles: [String] = [], year: UInt16? = nil,
        format: String? = nil, coverUrl: String? = nil, genres: [String] = [],
        list: Shelf.Source? = nil, episode: UInt32? = nil, episodes: UInt32? = nil
    ) {
        self.id = id
        self.title = title
        self.otherTitles = otherTitles
        self.year = year
        self.format = format
        self.coverUrl = coverUrl
        self.genres = genres
        self.list = list
        self.episode = episode
        self.episodes = episodes
    }

    public init(_ entry: Entry, list: Shelf.Source? = nil) {
        var seen: Set<String> = [entry.showTitle]
        let others = (entry.otherTitles + [entry.title]).filter { seen.insert($0).inserted }
        self.init(
            id: entry.id,
            title: entry.showTitle,
            otherTitles: others,
            year: entry.year,
            format: entry.format,
            coverUrl: entry.coverUrl,
            genres: entry.genres,
            list: list ?? entry.listStatus.map(Shelf.Source.init),
            episode: entry.progress?.next,
            episodes: entry.episodes
        )
    }

    /// The other-language title: the first of its other names.
    public var otherTitle: String? { otherTitles.first }

    /// The line under the title: the list, and the episode it plays next, "Watching · Episode 7
    /// of 24". A finished or planned title says its list alone.
    public var line: String {
        var parts: [String] = []
        if let list { parts.append(list.name) }
        if let episode, list != .completed, list != .planning, episodes.map({ episode <= $0 }) ?? true {
            parts.append(episodes.map { L.titleEpisodeOf(episode: Int(episode), total: Int($0)) } ?? L.episodeNumber(number: Int(episode)))
        }
        return parts.joined(separator: " · ")
    }

    /// The words it is found by beyond its names: its genres, format and year.
    public var keywords: [String] {
        genres + [format.map(TitleWords.format), year.map(String.init)].compactMap { $0 }
    }
}

/// Where each title on the shelf comes from, and the titles: what was last known, saved.
public struct Shelf: Codable, Equatable, Sendable {
    /// A list the shelf is made of.
    public enum Source: String, Codable, CaseIterable, Sendable {
        case `continue`, watching, planning, completed, paused, dropped

        init(_ segment: LibrarySegment) {
            switch segment {
            case .watching: self = .watching
            case .planning: self = .planning
            case .completed: self = .completed
            case .paused: self = .paused
            case .dropped: self = .dropped
            }
        }

        /// Its name as a line says it: "Watching", "Continue".
        public var name: String {
            segment?.title ?? L.navContinue
        }

        var segment: LibrarySegment? {
            switch self {
            case .continue: nil
            case .watching: .watching
            case .planning: .planning
            case .completed: .completed
            case .paused: .paused
            case .dropped: .dropped
            }
        }
    }

    /// Each list's titles, in its order.
    public var sources: [Source: [AnilistId]] = [:]
    public var titles: [AnilistId: ShelfTitle] = [:]
    /// The titles Spotlight has with their cover.
    public var covered: Set<AnilistId> = []

    public init() {}

    /// Every title on the shelf, Continue's first, then the Library's lists in their order, each
    /// once.
    public var order: [AnilistId] {
        var seen: Set<AnilistId> = []
        return Source.allCases.flatMap { sources[$0] ?? [] }.filter { seen.insert($0).inserted }
    }

    /// The shelf as `store` has it now, over `saved`: a list the engine has answered this
    /// session replaces the saved one, and a list it has not keeps what was saved. Signed out of
    /// every tracker, there is no Library.
    @MainActor
    public static func current(_ store: AppStore, over saved: Shelf) -> Shelf {
        var shelf = Shelf()
        let signedOut = !store.accounts.trackers.isEmpty && !store.accounts.trackers.contains(where: \.connected)
        for source in Source.allCases {
            let list = source.segment.map(store.library.segment) ?? store.continuing.list
            if source.segment != nil, signedOut {
                shelf.sources[source] = []
                continue
            }
            switch list.state {
            case .loaded(let ids):
                let hidden = source == .continue ? store.continuing.hidden : []
                shelf.sources[source] = ids.filter { !hidden.contains($0) }
            case .empty:
                shelf.sources[source] = []
            case .idle, .loading, .failed:
                shelf.sources[source] = saved.sources[source] ?? []
            }
        }
        // Each title under the first Library list that has it, and Continue's otherwise.
        let library = Source.allCases.filter { $0 != .continue } + [.continue]
        for id in shelf.order {
            let list = library.first { shelf.sources[$0]?.contains(id) ?? false }
            if let entry = store.titles[id], !entry.title.isEmpty {
                shelf.titles[id] = ShelfTitle(entry, list: list)
            } else if var kept = saved.titles[id] {
                kept.list = list
                shelf.titles[id] = kept
            }
        }
        shelf.covered = saved.covered.intersection(shelf.titles.keys)
        return shelf
    }

    // MARK: Saving

    /// Where the shelf is saved under the engine's `home`: in its cache, since it is only what
    /// the engine said, and in a home of its own wherever the engine's is overridden (the
    /// Fixtures app's, a test's).
    public static func file(in home: EngineHome) -> URL {
        home.cache.appending(path: "apple/shelf.json", directoryHint: .notDirectory)
    }

    /// The shelf saved at `url`, or `nil` when there is none to read.
    public static func load(_ url: URL) -> Shelf? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(Shelf.self, from: data)
    }

    public func save(_ url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        try encoder.encode(self).write(to: url, options: .atomic)
    }

    public static func remove(_ url: URL) {
        try? FileManager.default.removeItem(at: url)
    }
}
