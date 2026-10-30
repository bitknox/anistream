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

    /// Whether rows are on screen.
    var holdsRows: Bool {
        if case .loaded(let ids) = state { !ids.isEmpty } else { false }
    }

    /// Take the engine's answer. A failure to refresh does not take rows away that are on
    /// screen: they stay, and this says so by returning true.
    @discardableResult
    func apply(_ content: ContentList, titles: TitleCache, local: Bool = false) -> Bool {
        if case .failed = content, holdsRows { return true }
        switch content {
        case .empty:
            state = .empty
            answeredAt = .now
        case .loading:
            state = .loading
        case .entries(let entries):
            titles.merge(entries, local: local)
            state = .loaded(entries.map(\.id))
            answeredAt = .now
        case .failed(let reason):
            state = .failed(reason)
        }
        return false
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
    /// The furthest back and forward the calendar reaches, in epoch seconds, as the engine says.
    public private(set) var floor: Int64?
    public private(set) var ceiling: Int64?
    /// Pages of the future loaded, and whether there is more behind them.
    public private(set) var aheadPages: UInt32 = 0
    public private(set) var moreAhead = false
    /// Whether earlier or later broadcasts are on their way, and whether the last ask failed.
    public private(set) var loadingPast = false
    public private(set) var loadingAhead = false
    public private(set) var pastFailed = false
    public private(set) var aheadFailed = false
    /// The calendar's rows, oldest first: the broadcasts, each a title on a day, so not unique by
    /// title the way another list's are.
    public private(set) var rows: [Entry] = []

    public init() {}

    @discardableResult
    func apply(_ content: ContentList, titles: TitleCache) -> Bool {
        if list.apply(content, titles: titles) { return true }
        rows = content.entriesOrEmpty
        return false
    }

    func window(
        anchor: Int64, pages: UInt32, morePast: Bool,
        floor: Int64, ceiling: Int64, aheadPages: UInt32, moreAhead: Bool
    ) {
        self.anchor = anchor
        self.pages = pages
        self.morePast = morePast
        self.floor = floor
        self.ceiling = ceiling
        self.aheadPages = aheadPages
        self.moreAhead = moreAhead
        loadingPast = false
        loadingAhead = false
        pastFailed = false
        aheadFailed = false
    }

    /// Older broadcasts, above what is there. A page of `nil` failed: nothing to add, and the
    /// same page is asked for again when the person tries again.
    func backfill(page: UInt32?, entries: [Entry], morePast: Bool, titles: TitleCache) {
        loadingPast = false
        guard let page else {
            pastFailed = true
            return
        }
        pastFailed = false
        titles.merge(entries)
        rows.insert(contentsOf: entries, at: 0)
        pages = max(pages, page + 1)
        self.morePast = morePast
    }

    /// Later broadcasts, after what is there; a page of `nil` failed.
    func ahead(page: UInt32?, entries: [Entry], moreAhead: Bool, titles: TitleCache) {
        loadingAhead = false
        guard let page else {
            aheadFailed = true
            return
        }
        aheadFailed = false
        titles.merge(entries)
        rows.append(contentsOf: entries)
        aheadPages = max(aheadPages, page + 1)
        self.moreAhead = moreAhead
    }

    func clearFailures() {
        pastFailed = false
        aheadFailed = false
    }

    func askedPast() {
        loadingPast = true
        pastFailed = false
    }

    func askedAhead() {
        loadingAhead = true
        aheadFailed = false
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
    /// What the list on screen answers: `asked` once its answer is in, an earlier query while
    /// it is on its way, `nil` before any.
    public private(set) var answered: SearchQuery?
    /// Whether the answer to `asked` has been waited for past 600 ms: the screens then show the
    /// search as a job and placeholders in place of the earlier results. Never while typing
    /// waits for Return.
    public private(set) var overdue = false
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
    /// What the engine was last told, so an edit that changes nothing it searches for is not
    /// sent again.
    @ObservationIgnored private var told: SearchQuery?
    @ObservationIgnored private var overdueTimer: Task<Void, Never>?

    /// How long an answer is waited for before it shows as a job: the TUI's 600 ms.
    public static let quiet: Duration = .milliseconds(600)

    public init() {}

    /// Whether Search shows its start: rails and recent searches rather than results. One typed
    /// letter, which the engine does not search for as it is typed, is the start still.
    public var atStart: Bool {
        let normal = query.normalized
        return normal.isBlank() || (normal.waitsForWords() && normal.withoutWords.isBlank() && asked != normal)
    }

    /// Whether the answer to what was asked is still on its way.
    public var pending: Bool {
        asked != nil && answered != asked
    }

    /// The search changed: a keystroke, a filter, or Return when `submitted`. Whether the
    /// engine is to be told: not when only spaces around the text changed.
    func edit(_ query: SearchQuery, submitted: Bool = false) -> Bool {
        self.query = query
        let normal = query.normalized
        // One letter as it is typed: the engine waits for a second (the model's rule).
        let waits = !submitted && normal.waitsForWords()
        if normal.isBlank() || (waits && normal.withoutWords.isBlank()) {
            asked = nil
            answered = nil
            list.state = .idle
            list.caption = nil
            hasNext = false
            loadingMore = false
            pages = 0
            watchOverdue()
        } else if !waits, normal != asked {
            asked = normal
            // What is on screen stays until the answer replaces it, so typing does not flash;
            // past 600 ms the screens show placeholders instead (`overdue`).
            list.asked()
            hasNext = false
            loadingMore = false
            pages = 0
            watchOverdue()
        }
        defer { told = normal }
        return submitted || normal != told
    }

    /// Search again for what is asked, as when titles are named another way.
    func again() -> SearchQuery? {
        guard let asked else { return nil }
        list.asked()
        return asked
    }

    /// Try again after a failure: placeholders until the answer.
    func retry() -> SearchQuery? {
        guard let asked else { return nil }
        answered = nil
        list.state = .loading
        list.caption = nil
        watchOverdue()
        return asked
    }

    /// Whether an answer tagged with `query` is for what is on screen.
    func takes(_ query: SearchQuery) -> Bool {
        asked == query
    }

    /// The answer to `query`, when it is the one asked.
    @discardableResult
    func answer(_ content: ContentList, for query: SearchQuery, titles: TitleCache) -> Bool {
        guard takes(query) else { return false }
        let kept = list.apply(content, titles: titles)
        if case .loading = content { return false }
        answered = query
        overdue = false
        overdueTimer?.cancel()
        return kept
    }

    /// Count the 600 ms from now, for what was just asked.
    private func watchOverdue() {
        overdueTimer?.cancel()
        guard pending else {
            overdue = false
            return
        }
        // Placeholders already up stay for the next search, rather than the results before them
        // coming back until it is overdue in turn.
        guard !overdue else { return }
        overdueTimer = Task { [weak self] in
            try? await Task.sleep(for: Self.quiet)
            guard let self, !Task.isCancelled, pending, !livePaused else { return }
            overdue = true
        }
    }

    func paging(_ query: SearchQuery, hasNext: Bool) {
        guard takes(query) else { return }
        self.hasNext = hasNext
        pages = 1
        loadingMore = false
    }

    /// The next page to ask for, when there is one and none is on its way.
    func nextPage() -> (SearchQuery, UInt32)? {
        guard let asked, !pending, hasNext, !loadingMore, case .loaded = list.state else { return nil }
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
        if paused { overdue = false }
    }
}

extension SearchQuery {
    /// The query without its words: the filters and sort alone.
    public var withoutWords: SearchQuery {
        var query = self
        query.text = ""
        return query
    }

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
