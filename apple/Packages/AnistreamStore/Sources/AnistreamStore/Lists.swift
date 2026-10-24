// The lists the engine sends, each tagged with the view it answers.

import AnistreamKit
import Foundation
import Observation

/// Where a list stands.
public enum ListState: Equatable, Sendable {
    /// Never asked for.
    case idle
    /// Asked for, with nothing to show yet.
    case loading
    /// These titles, in the engine's order; the rows are in the title cache.
    case loaded([AnilistId])
    /// Nothing there at all: for Continue, nothing watched yet.
    case empty
    /// It could not be loaded, and why, in words to show.
    case failed(String)

    public var ids: [AnilistId] {
        if case .loaded(let ids) = self { ids } else { [] }
    }
}

/// One list: its state and the caption the engine says above it.
@Observable @MainActor
public final class ListModel {
    public internal(set) var state: ListState = .idle
    /// What the list is, said above it: which season Seasonal is, or that it stopped at a page.
    public internal(set) var caption: String?
    /// When the engine last answered with the list, or that it is empty.
    @ObservationIgnored public private(set) var answeredAt: ContinuousClock.Instant?

    public init() {}

    /// Whether the list was answered less than `window` ago, so asking again would only spend a
    /// request on what is on screen.
    public func answered(within window: Duration) -> Bool {
        answeredAt.map { ContinuousClock.now - $0 < window } ?? false
    }

    /// The list has been asked for. Keeps what is on screen until the answer replaces it.
    func asked() {
        switch state {
        case .idle, .failed: state = .loading
        default: break
        }
    }

    func apply(_ content: ContentList, titles: TitleCache) {
        switch content {
        case .empty:
            state = .empty
            answeredAt = .now
        case .loading:
            state = .loading
        case .entries(let entries):
            titles.merge(entries)
            state = .loaded(entries.map(\.id))
            answeredAt = .now
        case .failed(let reason):
            state = .failed(reason)
        }
    }
}

/// The broadcasts around now, and how to page back through earlier ones.
@Observable @MainActor
public final class CalendarModel {
    public let list = ListModel()
    /// The anchor the calendar's pages are numbered against.
    public private(set) var anchor: Int64?
    public private(set) var pages: UInt32 = 0
    public private(set) var morePast = false
    /// The calendar's rows, oldest first: the broadcasts, each a title on a day, so not unique by
    /// title the way another list's are.
    public private(set) var rows: [Entry] = []

    public init() {}

    func apply(_ content: ContentList, titles: TitleCache) {
        list.apply(content, titles: titles)
        rows = content.entriesOrEmpty
    }

    func window(anchor: Int64, pages: UInt32, morePast: Bool) {
        self.anchor = anchor
        self.pages = pages
        self.morePast = morePast
    }

    /// Older broadcasts, above what is there. A page of `nil` failed: nothing to add, and the
    /// same page is asked for again on the next scroll.
    func backfill(page: UInt32?, entries: [Entry], morePast: Bool, titles: TitleCache) {
        guard let page else { return }
        titles.merge(entries)
        rows.insert(contentsOf: entries, at: 0)
        pages = max(pages, page + 1)
        self.morePast = morePast
    }
}

/// Search: the query as it stands, the results for it, and what Search shows before anything
/// is asked. It takes only the answer to what was asked last, so a slow answer to an earlier
/// query never replaces the results of a later one.
@Observable @MainActor
public final class SearchModel {
    public let list = ListModel()
    /// The search as it stands: what is typed and every filter. Blank is Search's start.
    public private(set) var query = SearchQuery()
    /// What the list is the answer to, as the engine tags it: `query` with its text trimmed.
    /// `nil` at Search's start.
    public private(set) var asked: SearchQuery?
    /// Whether AniList has more pages of the results behind those on screen.
    public private(set) var hasNext = false
    /// Whether a further page is on its way.
    public private(set) var loadingMore = false
    /// How many pages of the results are on screen.
    public private(set) var pages: UInt32 = 0
    /// Search's start: rails of titles, each a query of its own. `nil` until the engine sends
    /// them.
    public private(set) var rails: [BrowseRail]?
    /// What the filters choose from. `nil` until the engine sends it.
    public private(set) var facets: SearchFacets?
    /// The recent searches, newest first, each with its filters.
    public private(set) var recents: [SearchQuery] = []
    /// Whether typing waits for Return: the AniList budget ran low. Filters still search.
    public private(set) var livePaused = false

    public init() {}

    /// Whether Search shows its start: rails and recent searches rather than results.
    public var atStart: Bool { query.isBlank() }

    /// The search changed. Whether it asks anything new of the engine: not at the start, and not
    /// when only spaces around the text changed.
    func edit(_ query: SearchQuery) -> Bool {
        self.query = query
        let normal = query.normalized
        if normal.isBlank() {
            asked = nil
            list.state = .idle
            list.caption = nil
            hasNext = false
            loadingMore = false
            pages = 0
            return false
        }
        guard normal != asked else { return false }
        asked = normal
        // What is on screen stays until the answer replaces it, so typing does not flash.
        list.asked()
        hasNext = false
        loadingMore = false
        pages = 0
        return true
    }

    /// Search again for what is asked, as when titles are named another way.
    func again() -> SearchQuery? {
        guard let asked else { return nil }
        list.asked()
        return asked
    }

    /// Whether an answer tagged with `query` is for what is on screen.
    func takes(_ query: SearchQuery) -> Bool {
        asked == query
    }

    func paging(_ query: SearchQuery, hasNext: Bool) {
        guard takes(query) else { return }
        self.hasNext = hasNext
        pages = 1
        loadingMore = false
    }

    /// The next page to ask for, when there is one and none is on its way.
    func nextPage() -> (SearchQuery, UInt32)? {
        guard let asked, hasNext, !loadingMore, case .loaded = list.state else { return nil }
        loadingMore = true
        return (asked, pages + 1)
    }

    /// A further page: after what is there. An empty page failed, and is asked for again on the
    /// next scroll.
    func more(_ query: SearchQuery, page: UInt32, entries: [Entry], hasNext: Bool, titles: TitleCache) {
        guard takes(query) else { return }
        loadingMore = false
        guard !entries.isEmpty else { return }
        titles.merge(entries)
        let ids = list.state.ids
        let seen = Set(ids)
        list.state = .loaded(ids + entries.map(\.id).filter { !seen.contains($0) })
        pages = max(pages, page)
        self.hasNext = hasNext
    }

    func rails(_ rails: [BrowseRail], titles: TitleCache) {
        for rail in rails { titles.merge(rail.entries) }
        self.rails = rails
    }

    func facets(_ facets: SearchFacets) {
        self.facets = facets
    }

    func recents(_ recents: [SearchQuery]) {
        self.recents = recents
    }

    func live(paused: Bool) {
        livePaused = paused
    }
}

extension SearchQuery {
    /// The query as the engine searches for it and tags its answer: its text without the spaces
    /// around it.
    public var normalized: SearchQuery {
        var query = self
        query.text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return query
    }

    /// The season on now, its most popular first: what Seasonal shows, as a search.
    public static func season(at date: Date = .now) -> SearchQuery {
        // In UTC, as the engine reckons the season.
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .gmt
        let parts = calendar.dateComponents([.month, .year], from: date)
        let month = parts.month ?? 1
        let year = UInt32(parts.year ?? 2026)
        let season: Season = switch month {
        case 1...3: .winter
        case 4...6: .spring
        case 7...9: .summer
        default: .fall
        }
        return SearchQuery(sort: .popularity, season: season, years: Bounds(min: year, max: year))
    }
}

/// The tracker's lists, a segment each.
@Observable @MainActor
public final class LibraryModel {
    /// The segments in the tracker's order.
    public static let order: [LibrarySegment] = [.watching, .planning, .completed, .paused, .dropped]

    public let segments: [LibrarySegment: ListModel]

    public init() {
        segments = Dictionary(uniqueKeysWithValues: Self.order.map { ($0, ListModel()) })
    }

    public func segment(_ segment: LibrarySegment) -> ListModel {
        segments[segment]!
    }
}

extension ContentList {
    var entriesOrEmpty: [Entry] {
        if case .entries(let entries) = self { entries } else { [] }
    }
}
