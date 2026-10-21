// The lists the engine sends, each tagged with the view it answers.

import AnistreamKit
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

/// Search, which takes only the answer to what was asked last: a slow answer to an earlier
/// query never replaces the results of a later one.
@Observable @MainActor
public final class SearchModel {
    public let list = ListModel()
    /// The query and filters as last submitted.
    public private(set) var submitted: (query: String, filter: SearchFilters)?

    public init() {}

    func submit(query: String, filter: SearchFilters) {
        submitted = (query, filter)
        list.state = .loading
        list.caption = nil
    }

    func takes(query: String, filter: SearchFilters) -> Bool {
        submitted.map { $0.query == query && $0.filter == filter } ?? false
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
