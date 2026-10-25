// Search: what is typed and every filter as one query, the titles found as covers, and before
// anything is asked, the recent searches and rails of titles. The engine searches as the query
// changes, words every filter, and keeps the recent searches.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Words

extension SearchQuery {
    /// A recent search in one line: its words, then its filters. "hoshi · Mecha · TV".
    var recentLine: String {
        let text = normalized.text
        guard !text.isEmpty else { return headline() }
        let summary = summary()
        return summary.isEmpty ? text : "\(text) · \(summary)"
    }

    /// The year, when `years` is a single one: the only case a season narrows.
    var singleYear: UInt32? {
        years.min != nil && years.min == years.max ? years.min : nil
    }

    /// The engine's words for one of the filters set, as its chip says it.
    func label(_ key: FilterKey) -> String? {
        active().first { $0.key == key }?.label
    }
}

extension Pick {
    /// What a tap makes it: off, then in, then out, then off again.
    var next: Pick {
        switch self {
        case .off: .include
        case .include: .exclude
        case .exclude: .off
        }
    }

    var word: String? {
        switch self {
        case .off: nil
        case .include: "Included"
        case .exclude: "Left out"
        }
    }
}

/// The seasons in the order a year has them.
let searchSeasons: [Season] = [.winter, .spring, .summer, .fall]

/// "Hide obscure titles": at least this many people have the title on a list. The engine's
/// `OBSCURE_BELOW`.
let obscureBelow: UInt32 = 1_000

/// How strongly a tag must apply when nothing else is set. The engine's `DEFAULT_TAG_RANK`.
let defaultTagRank: UInt8 = 60

@MainActor
extension AppStore {
    /// Change the search as a filter does: the engine searches once the changes stop.
    func editSearch(_ change: (inout SearchQuery) -> Void) {
        var query = search.query
        change(&query)
        searchLive(query)
    }
}

// MARK: - The field

/// A genre or tag in the search field, as a token.
struct SearchToken: Identifiable, Hashable {
    enum Kind: Hashable {
        case genre, tag
    }

    let kind: Kind
    let name: String

    var id: String { "\(kind)-\(name)" }

    /// The genres and tags a query includes, in the field's order.
    static func of(_ query: SearchQuery) -> [SearchToken] {
        query.genres.map { SearchToken(kind: .genre, name: $0) }
            + query.tags.map { SearchToken(kind: .tag, name: $0) }
    }
}

/// The titles' search field: the words search as they are typed, Return searches at once, and a
/// genre or tag typed is offered as a token, a filter in the field.
struct TitleSearchField: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    /// What the field holds: the titles' words, or, on the Mac's Settings, its own.
    let text: Binding<String>
    /// Whether the field searches titles: not while it searches Settings.
    var titles = true
    var placement: SearchFieldPlacement = .automatic
    let prompt: String

    func body(content: Content) -> some View {
        content
            .searchable(
                text: text,
                tokens: tokens,
                suggestedTokens: .constant(titles ? suggestions : []),
                isPresented: Binding(get: { navigator.searchPresented }, set: { navigator.searchPresented = $0 }),
                placement: placement,
                prompt: prompt
            ) { token in
                Label(token.name, systemImage: token.kind == .genre ? "theatermasks" : "tag")
            }
            .onSubmit(of: .search) {
                guard titles else { return }
                navigator.section = .search
                store.search(text: navigator.query)
            }
            .onChange(of: navigator.query) { _, words in
                guard titles else { return }
                // Typing anywhere is a search: the Search section shows what it finds.
                if !words.isEmpty, navigator.section != .search { navigator.section = .search }
                store.searchLive(text: words)
            }
    }

    private var tokens: Binding<[SearchToken]> {
        Binding(
            get: { titles ? SearchToken.of(store.search.query) : [] },
            set: { tokens in
                guard titles else { return }
                var query = store.search.query
                let added = tokens.count > SearchToken.of(query).count
                query.genres = tokens.filter { $0.kind == .genre }.map(\.name)
                query.tags = tokens.filter { $0.kind == .tag }.map(\.name)
                query.genresExcluded.removeAll { query.genres.contains($0) }
                query.tagsExcluded.removeAll { query.tags.contains($0) }
                // A token chosen stands for the words that found it.
                if added { query.text = "" }
                store.searchLive(query)
                if added { navigator.query = "" }
            }
        )
    }

    /// The genres and tags whose names start a word with what is typed.
    private var suggestions: [SearchToken] {
        let words = navigator.query.trimmingCharacters(in: .whitespaces)
        guard words.count >= 2, let facets = store.search.facets else { return [] }
        let present = Set(SearchToken.of(store.search.query))
        let genres = facets.genresMatching(prefix: words).map { SearchToken(kind: .genre, name: $0) }
        let tags = facets.tagsMatching(prefix: words, limit: 6).map { SearchToken(kind: .tag, name: $0) }
        return Array((genres + tags).filter { !present.contains($0) }.prefix(8))
    }
}

extension View {
    func titleSearchField(
        text: Binding<String>, titles: Bool = true, placement: SearchFieldPlacement = .automatic, prompt: String
    ) -> some View {
        modifier(TitleSearchField(text: text, titles: titles, placement: placement, prompt: prompt))
    }
}

// MARK: - The screen

extension SearchModel {
    /// The search as a job, once its answer has been waited for past 600 ms: "Searching for
    /// “hoshi”". `nil` otherwise.
    var searchingLine: String? {
        guard overdue, let asked else { return nil }
        return asked.isBrowse() ? "Searching for \(asked.headline())" : "Searching for “\(asked.text)”"
    }

    /// Whether Search's start stays on screen: before anything is asked, and while the first
    /// answer is under 600 ms away, as the earlier screen stays until an answer replaces it.
    var startShown: Bool {
        atStart || (answered == nil && pending && !overdue)
    }
}

/// Search: the filters, always there, over Search's start or the titles found.
struct SearchScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// The iPhone's: sentence case, larger type, three columns.
    var compact = false
    #if os(macOS)
    /// The screen's width with the filters panel closed, which opening it does not change.
    @State private var fullWidth: CGFloat?
    #endif

    private var margins: EdgeInsets {
        compact ? EdgeInsets(top: 8, leading: 16, bottom: 24, trailing: 16)
                : EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16)
    }

    /// The width the results take once the filters panel has opened or closed, set at once: the
    /// grid lays out a single time and its covers move to their new places with the panel, rather
    /// than reflowing at every width the panel passes through. `nil` until the screen is measured,
    /// and on iPhone and iPad, whose system sheet and inspector lay out once.
    private var settledWidth: CGFloat? {
        #if os(macOS)
        guard let fullWidth else { return nil }
        let panel = navigator.searchFiltersShown ? MacFiltersPanel.width : 0
        return max(0, fullWidth - panel - margins.leading - margins.trailing)
        #else
        return nil
        #endif
    }

    var body: some View {
        @Bindable var navigator = navigator
        let startShown = store.search.startShown
        ScrollView {
            VStack(alignment: .leading, spacing: compact ? 16 : 20) {
                SearchFilterRow(compact: compact)
                ActiveFilters(compact: compact)
                if store.search.livePaused, !store.search.query.isBrowse() {
                    LivePaused(compact: compact)
                }
                // Start and results crossfade (standard), the one in place of the other.
                ZStack(alignment: .topLeading) {
                    if startShown {
                        SearchStart(compact: compact)
                            .transition(.opacity)
                    } else {
                        SearchResults(compact: compact)
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: startShown)
            }
            .frame(width: settledWidth, alignment: .leading)
            // With Reduce Motion the results take their new width at once while the panel fades.
            .transaction(value: navigator.searchFiltersShown) { transaction in
                if reduceMotion { transaction.animation = nil }
            }
            .padding(margins)
            // Wider than the scroll view while the panel slides away: from the leading edge,
            // under the panel, never centred.
            .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.never)
        #if os(iOS)
        .navigationSubtitle(store.search.searchingLine ?? "")
        // The ground inside the inspector: it hosts the screen in a column of its own, which
        // draws the system's black over the ground TabStack puts behind it.
        .background(Palette.ground)
        .inspector(isPresented: $navigator.searchFiltersShown) {
            SearchFiltersPanel(compact: compact)
                .inspectorColumnWidth(min: 300, ideal: 340, max: 420)
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
        #else
        // Every filter in a panel at the trailing edge, inside Search rather than the system's
        // inspector column: an inspector splits the window's title bar and AppKit draws its own
        // toolbar backdrop across it, a grey band in place of the ground. Here the results'
        // scroll view still spans the window under the toolbar, so the header never changes.
        // The panel stays mounted and slides in from off the edge: inserting it made the glass
        // toolbar items flash dark. The results lay out once at their settled width (above), so
        // the cards glide to their new columns instead of stepping through every count between.
        .overlay(alignment: .trailing) {
            let shown = navigator.searchFiltersShown
            MacFiltersPanel()
                .offset(x: shown || reduceMotion ? 0 : MacFiltersPanel.width + 1)
                .opacity(shown || !reduceMotion ? 1 : 0)
                .allowsHitTesting(shown)
                .disabled(!shown)
                .accessibilityHidden(!shown)
        }
        .animation(.standard, value: navigator.searchFiltersShown)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { fullWidth = $0 }
        #endif
        .task {
            store.loadSearchStart()
            if let launch = navigator.launchSearch {
                navigator.launchSearch = nil
                navigator.search(launch, in: store)
            } else if store.search.asked == nil, !navigator.query.isEmpty {
                store.search(text: navigator.query)
            }
            if navigator.launchSheet == "typing" {
                navigator.launchSheet = nil
                try? await Task.sleep(for: .milliseconds(300))
                navigator.searchPresented = true
            }
            if navigator.launchSheet == "filters" {
                navigator.launchSheet = nil
                // Once the window is up: an inspector open as the window first lays out loops
                // (iPad).
                try? await Task.sleep(for: .milliseconds(300))
                navigator.searchFiltersShown = true
            }
        }
        // A title opened from what was found keeps the search among the recent ones, as Return
        // would have.
        .onChange(of: navigator.paths[.search]?.count ?? 0) { before, after in
            if after > before, !store.search.atStart { store.rememberSearch() }
        }
    }
}

#if os(macOS)
/// The Mac's filters panel: every filter, beside the results, a hairline at its edge.
private struct MacFiltersPanel: View {
    static let width: CGFloat = 340

    var body: some View {
        SearchFiltersPanel()
            .frame(width: Self.width)
            .overlay(alignment: .leading) {
                Rectangle().fill(Palette.faintRule).frame(width: 1)
            }
    }
}
#endif

/// Typing waits for Return: the AniList budget is nearly spent. Filters still search.
private struct LivePaused: View {
    var compact: Bool

    var body: some View {
        Text(
            compact ? "Results wait for Search while AniList’s rate limit recovers"
                    : "Results wait for Return while AniList’s rate limit recovers"
        )
        .font(compact ? .ui(15) : .mac(13))
        .foregroundStyle(.haze)
    }
}

// MARK: - The filters up front

/// A filter's capsule: its name, or what it is set to in amber.
struct FilterCapsule: View {
    let title: String
    var active = false
    var compact = false
    var symbol: String?
    var badge: UInt32 = 0
    var chevron = true

    var body: some View {
        HStack(spacing: 6) {
            if let symbol { Image(systemName: symbol) }
            Text(title).lineLimit(1)
            if badge > 0 {
                Text("\(badge)").monospacedDigit().foregroundStyle(.haze)
            }
            if chevron {
                Image(systemName: "chevron.down")
                    .font(compact ? .ui(11, weight: .semibold) : .mac(9, weight: .semibold))
            }
        }
        .font(compact ? .ui(15) : .mac(13))
        .foregroundStyle(active ? .amber : .silk)
        .padding(.horizontal, compact ? 14 : 12)
        .frame(height: compact ? 34 : 28)
        .background(active ? Palette.selection : .clear, in: .capsule)
        .overlay(Capsule().strokeBorder(active ? .clear : Palette.faintRule))
        // A 44 point target on touch around a smaller capsule.
        .frame(minHeight: compact ? 44 : 28)
        .contentShape(.rect)
    }
}

/// The filters up front: genres, year, season, format and status, each a menu, then Filters for
/// the rest and the sort. Changing one searches.
struct SearchFilterRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        let query = store.search.query
        if compact {
            // On a phone the row scrolls, to the screen's edge, the sort first.
            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    sort(query)
                    capsules(query)
                }
                .padding(.vertical, 1)
            }
            .scrollIndicators(.never)
            .scrollClipDisabled()
        } else {
            // On the Mac and a wide iPad the capsules go onto a second line where they do not
            // fit beside the sort, as in a narrow window or with the filters open: none is cut
            // or hidden under it.
            HStack(alignment: .top, spacing: 8) {
                FlowLayout(spacing: 8, lineSpacing: 8) {
                    capsules(query)
                }
                sort(query)
            }
            .padding(.vertical, 1)
        }
    }

    @ViewBuilder
    private func capsules(_ query: SearchQuery) -> some View {
        genres(query)
        year(query)
        season(query)
        formats(query)
        statuses(query)
        filters(query)
    }

    private func words(_ mac: String, _ phone: String) -> String {
        compact ? phone : mac
    }

    private func genres(_ query: SearchQuery) -> some View {
        let names = store.search.facets?.genres ?? []
        let set = query.genres.count + query.genresExcluded.count
        let title = set == 0 ? words("Genres", "Genres")
            : set == 1 ? (query.active().first?.label ?? "1 genre")
            : words("\(set) Genres", "\(set) genres")
        return Menu {
            if names.isEmpty { Text("Loading genres") }
            ForEach(names, id: \.self) { name in
                Toggle(name, isOn: pick(name, .include))
            }
            if !names.isEmpty {
                Divider()
                Menu(words("Leave Out", "Leave out")) {
                    ForEach(names, id: \.self) { name in
                        Toggle(name, isOn: pick(name, .exclude))
                    }
                }
            }
            if set > 0 {
                Divider()
                Button(words("Any Genre", "Any genre")) {
                    store.editSearch { $0.genres = []; $0.genresExcluded = [] }
                }
            }
        } label: {
            FilterCapsule(title: title, active: set > 0, compact: compact)
        }
        .filterMenu()
    }

    private func pick(_ name: String, _ pick: Pick) -> Binding<Bool> {
        Binding(
            get: { store.search.query.genrePick(name: name) == pick },
            set: { on in store.searchLive(store.search.query.pickingGenre(name: name, pick: on ? pick : .off)) }
        )
    }

    private func year(_ query: SearchQuery) -> some View {
        let now = UInt32(Calendar(identifier: .gregorian).component(.year, from: .now))
        let label = query.label(.years)
        return Menu {
            Button(words("Any Year", "Any year")) { store.searchLive(query.inYear(year: nil)) }
            Divider()
            ForEach(Array(stride(from: now + 1, through: 1970, by: -1)), id: \.self) { year in
                Toggle(String(year), isOn: Binding(
                    get: { store.search.query.singleYear == year },
                    set: { on in store.searchLive(store.search.query.inYear(year: on ? year : nil)) }
                ))
            }
        } label: {
            FilterCapsule(title: label ?? words("Year", "Year"), active: label != nil, compact: compact)
        }
        .filterMenu()
    }

    private func season(_ query: SearchQuery) -> some View {
        Menu {
            Button(words("Any Season", "Any season")) { store.editSearch { $0.season = nil } }
            Divider()
            ForEach(searchSeasons, id: \.self) { season in
                Toggle(searchSeasonLabel(season: season), isOn: Binding(
                    get: { store.search.query.season == season },
                    set: { on in store.editSearch { $0.season = on ? season : nil } }
                ))
            }
        } label: {
            FilterCapsule(
                title: query.season.map { searchSeasonLabel(season: $0) } ?? words("Season", "Season"),
                active: query.season != nil, compact: compact
            )
        }
        .filterMenu()
        // A season is a season of one year.
        .disabled(query.singleYear == nil && query.season == nil)
        .help(query.singleYear == nil ? "Choose a year first" : "")
    }

    private func formats(_ query: SearchQuery) -> some View {
        let count = query.formats.count
        return Menu {
            ForEach(searchFormats(), id: \.self) { format in
                Toggle(mediaFormatLabel(format: format), isOn: Binding(
                    get: { store.search.query.formats.contains(format) },
                    set: { _ in store.searchLive(store.search.query.togglingFormat(format: format)) }
                ))
            }
            if count > 0 {
                Divider()
                Button(words("Any Format", "Any format")) { store.editSearch { $0.formats = [] } }
            }
        } label: {
            FilterCapsule(
                title: count == 0 ? "Format" : count <= 2 ? (query.label(.formats) ?? "Format")
                    : words("\(count) Formats", "\(count) formats"),
                active: count > 0, compact: compact
            )
        }
        .filterMenu()
    }

    private func statuses(_ query: SearchQuery) -> some View {
        let count = query.statuses.count
        return Menu {
            ForEach(searchStatuses(), id: \.self) { status in
                Toggle(searchStatusLabel(status: status), isOn: Binding(
                    get: { store.search.query.statuses.contains(status) },
                    set: { _ in store.searchLive(store.search.query.togglingStatus(status: status)) }
                ))
            }
            if count > 0 {
                Divider()
                Button(words("Any Status", "Any status")) { store.editSearch { $0.statuses = [] } }
            }
        } label: {
            FilterCapsule(
                title: count == 0 ? "Status" : count == 1 ? (query.label(.statuses) ?? "Status")
                    : words("\(count) Statuses", "\(count) statuses"),
                active: count > 0, compact: compact
            )
        }
        .filterMenu()
    }

    private func filters(_ query: SearchQuery) -> some View {
        let count = query.advancedCount()
        return Button {
            navigator.searchFiltersShown.toggle()
        } label: {
            FilterCapsule(
                title: "Filters", active: count > 0, compact: compact,
                symbol: "line.3.horizontal.decrease", badge: count, chevron: false
            )
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityValue(count == 0 ? "" : "\(count) set")
        .help("Tags, length, score, origin and where to watch")
    }

    private func sort(_ query: SearchQuery) -> some View {
        let current = query.sortInEffect()
        return Menu {
            Picker(words("Sort By", "Sort by"), selection: Binding(
                get: { store.search.query.sortInEffect() },
                set: { sort in store.editSearch { $0.sort = sort } }
            )) {
                ForEach(searchSorts(withText: !query.isBrowse()), id: \.self) { sort in
                    Text(searchSortLabel(sort: sort)).tag(sort)
                }
            }
            .pickerStyle(.inline)
        } label: {
            FilterCapsule(
                title: searchSortLabel(sort: current), active: query.sort != nil, compact: compact,
                symbol: "arrow.up.arrow.down"
            )
        }
        .filterMenu()
        .accessibilityLabel("Sort by \(searchSortLabel(sort: current))")
    }
}

extension View {
    /// A capsule menu in the filter row: no system chrome around the capsule it draws.
    fileprivate func filterMenu() -> some View {
        menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .pointerHover()
    }
}

/// What is set, each as the engine words it, each taken off with a tap.
struct ActiveFilters: View {
    @Environment(AppStore.self) private var store
    var compact = false

    var body: some View {
        let query = store.search.query
        let active = query.active()
        if !active.isEmpty {
            FlowLayout(spacing: 8, lineSpacing: 8) {
                ForEach(active, id: \.key) { filter in
                    Button {
                        store.searchLive(store.search.query.without(key: filter.key))
                    } label: {
                        HStack(spacing: 6) {
                            Text(filter.label).lineLimit(1)
                            Image(systemName: "xmark")
                                .font(compact ? .ui(11, weight: .semibold) : .mac(9, weight: .semibold))
                        }
                        .font(compact ? .ui(15) : .mac(12))
                        .foregroundStyle(.silk)
                        .padding(.horizontal, 10)
                        .frame(height: compact ? 30 : 24)
                        .background(Palette.raised, in: .capsule)
                        .overlay(Capsule().strokeBorder(Palette.faintRule))
                        .frame(minHeight: compact ? 44 : 24)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .pointerHover()
                    .accessibilityLabel("Remove \(filter.label)")
                }
                if active.count > 1 {
                    Button(compact ? "Clear all" : "Clear All") {
                        store.searchLive(store.search.query.cleared())
                    }
                    .buttonStyle(.plain)
                    .font(compact ? .ui(15, weight: .semibold) : .mac(12, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: compact ? 44 : 24)
                }
            }
        }
    }
}

// MARK: - The titles found

/// The titles found, a page at a time: the next is asked for as the end comes near.
struct SearchResults: View {
    @Environment(AppStore.self) private var store
    var compact = false

    var body: some View {
        let search = store.search
        let minimum: CGFloat = compact ? 100 : 128
        let spacing: CGFloat = compact ? 12 : 20
        // The earlier results stay while the answer is under 600 ms away, then give way to
        // placeholders: never results that answer something else for long.
        let state: ListState = search.overdue ? .loading : search.list.state
        VStack(alignment: .leading, spacing: 16) {
            if let caption = search.list.caption, !search.overdue {
                Text(caption)
                    .font(compact ? .ui(15) : .mac(13))
                    .foregroundStyle(.haze)
            }
            ListStates(
                state: state,
                placeholder: AnyView(PosterGridPlaceholder(count: compact ? 12 : 14, minimum: minimum, spacing: spacing)),
                retry: { store.retrySearch() }
            ) { ids in
                VStack(alignment: .leading, spacing: 24) {
                    PosterGrid(entries: ids.compactMap { store.titles[$0] }, minimum: minimum, spacing: spacing) { entry in
                        PosterCell(entry: entry, detail: searchLine(entry), width: minimum)
                    }
                    // One answer replaces another in one crossfade, not cell by cell.
                    .id(search.answered)
                    .transition(.opacity)
                    if search.hasNext {
                        // The next page's covers to come, in the grid's own columns: they ask
                        // for it as they come into sight.
                        PosterGridPlaceholder(count: compact ? 6 : 8, minimum: minimum, spacing: spacing)
                            // Anew for each page, so a page that still leaves them in sight
                            // asks for the next.
                            .id(search.pages)
                            .onScrollVisibilityChange(threshold: 0.01) { visible in
                                if visible { store.searchMore() }
                            }
                            .accessibilityElement()
                            .accessibilityLabel("Loading more titles")
                    }
                }
            } empty: {
                NothingFound(compact: compact)
            }
            .animation(.standard, value: search.answered)
        }
    }

    private func searchLine(_ entry: Entry) -> String? {
        if let status = entry.listStatus {
            let watching = status == .watching ? entry.progress.map { " · Ep \($0.next)" } ?? "" : ""
            return status.label + watching
        }
        return [entry.format, entry.year.map(String.init)].compactMap { $0 }.joined(separator: " · ")
    }
}

/// Nothing found: the system's search empty state, which repeats the words, and with filters set,
/// what was asked in the engine's words and the way out.
private struct NothingFound: View {
    @Environment(AppStore.self) private var store
    var compact = false

    var body: some View {
        let query = store.search.asked ?? store.search.query.normalized
        Group {
            if query.hasFilters() {
                ContentUnavailableView {
                    Label(compact ? "No results" : "No Results", systemImage: "magnifyingglass")
                } description: {
                    Text(query.nothingFound())
                } actions: {
                    Button(compact ? "Clear filters" : "Clear Filters") {
                        store.searchLive(store.search.query.cleared())
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: compact ? 44 : 28, fontSize: compact ? 17 : 13))
                }
            } else {
                ContentUnavailableView.search(text: query.text)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 240)
    }
}

// MARK: - Search's start

/// Search before anything is asked: the recent searches, the rails, and the genres.
struct SearchStart: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            if !store.search.recents.isEmpty {
                RecentSearchList(compact: compact)
            }
            Group {
                if let rails = store.search.rails {
                    VStack(alignment: .leading, spacing: 28) {
                        ForEach(rails.filter { !$0.entries.isEmpty }, id: \.title) { rail in
                            RailRow(rail: rail, compact: compact)
                        }
                    }
                    .transition(.opacity)
                } else {
                    RailPlaceholder(compact: compact)
                        .transition(.opacity)
                }
            }
            .animation(.standard, value: store.search.rails == nil)
            if let genres = store.search.facets?.genres, !genres.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    SectionHeading("Genres", compact: compact)
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: compact ? 100 : 140), spacing: 8)], spacing: 8) {
                        ForEach(genres, id: \.self) { genre in
                            Button(genre) {
                                navigator.search(SearchQuery(genres: [genre]), in: store)
                            }
                            .buttonStyle(ChipStyle(height: compact ? 44 : 32, fontSize: compact ? 17 : 13))
                        }
                    }
                }
            }
        }
    }
}

/// The recent searches, newest first, each with its filters, each searched again with a tap.
private struct RecentSearchList: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: compact ? 0 : 4) {
            HStack {
                SectionHeading(compact ? "Recent searches" : "Recent Searches", compact: compact)
                Spacer()
                Button("Clear") { store.clearRecentSearches() }
                    .buttonStyle(.plain)
                    .font(compact ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: compact ? 44 : 28)
            }
            ForEach(store.search.recents, id: \.self) { query in
                HStack(spacing: 12) {
                    Button {
                        navigator.search(query, in: store)
                    } label: {
                        Label(query.recentLine, systemImage: query.isBrowse() ? "line.3.horizontal.decrease" : "clock")
                            .font(compact ? .ui(17) : .mac(13))
                            .foregroundStyle(.silk)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    Button("Remove \(query.recentLine)", systemImage: "xmark") {
                        store.forgetRecentSearch(query)
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.plain)
                    .foregroundStyle(.haze)
                    .frame(minWidth: compact ? 44 : 20, minHeight: compact ? 44 : 28)
                }
                .frame(minHeight: compact ? 44 : 28)
            }
        }
    }
}

/// One rail: a heading, See All, and its covers in a row that scrolls.
private struct RailRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let rail: BrowseRail
    var compact = false

    var body: some View {
        let width: CGFloat = compact ? 112 : 128
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                // The engine says it in sentence case, as the iPhone has it; the Mac's headings
                // are in title case.
                SectionHeading(compact ? rail.title : rail.title.capitalized, compact: compact)
                if let detail = rail.detail {
                    Text(detail).font(compact ? .ui(15) : .mac(13)).foregroundStyle(.haze)
                }
                Spacer()
                Button(compact ? "See all" : "See All") {
                    navigator.search(rail.query, in: store)
                }
                .buttonStyle(.plain)
                .font(compact ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(minHeight: compact ? 44 : 28)
                .pointerHover()
            }
            ScrollView(.horizontal) {
                LazyHStack(alignment: .top, spacing: compact ? 12 : 20) {
                    ForEach(rail.entries, id: \.id) { entry in
                        PosterCell(entry: store.titles[entry.id] ?? entry, detail: line(entry), width: width)
                            .frame(width: width)
                    }
                }
                .padding(.horizontal, compact ? 16 : 12)
            }
            .scrollIndicators(.never)
            .padding(.horizontal, compact ? -16 : -12)
        }
    }

    private func line(_ entry: Entry) -> String? {
        entry.genres.first
    }
}

/// Covers to come, while the rails are on their way: nothing for 600 ms, then rails of plates at
/// the size the rails will have.
private struct RailPlaceholder: View {
    var compact = false
    @State private var shown = false

    var body: some View {
        let width: CGFloat = compact ? 112 : 128
        VStack(alignment: .leading, spacing: 28) {
            ForEach(0..<3, id: \.self) { _ in
                VStack(alignment: .leading, spacing: 12) {
                    SectionHeading("Trending Now", compact: compact)
                        .frame(minHeight: compact ? 44 : 28)
                    HStack(alignment: .top, spacing: compact ? 12 : 20) {
                        ForEach(0..<8, id: \.self) { _ in
                            PosterCellPlaceholder(width: width).frame(width: width)
                        }
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .clipped()
                }
            }
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .opacity(shown ? 1 : 0)
        .task {
            try? await Task.sleep(for: SearchModel.quiet)
            withAnimation(.standard) { shown = true }
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Every filter

/// Every filter, in sections: the Mac's panel beside the results, the iPad's inspector, the
/// iPhone's sheet. Each change searches; there is nothing to apply.
struct SearchFiltersPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false
    @State private var find = ""
    @State private var spoilers = false

    var body: some View {
        form
    }

    private func reset() {
        store.searchLive(store.search.query.cleared())
    }

    private var form: some View {
        let query = store.search.query
        let facets = store.search.facets
        return ScrollView {
            VStack(alignment: .leading, spacing: compact ? 28 : 24) {
                header(query)
                PanelSection(compact: compact, "Genres") {
                    FlowLayout(spacing: 6, lineSpacing: 6) {
                        ForEach(facets?.genres ?? [], id: \.self) { name in
                            PickChip(name: name, pick: query.genrePick(name: name), compact: compact) { pick in
                                store.searchLive(store.search.query.pickingGenre(name: name, pick: pick))
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
                tags(query, facets)
                PanelSection(compact: compact, "Released") {
                    RangeFields(title: "Years", bounds: query.years, unit: nil, compact: compact) { bounds in
                        store.editSearch {
                            $0.years = bounds
                            if $0.singleYear == nil { $0.season = nil }
                        }
                    }
                }
                PanelSection(compact: compact, "Length") {
                    RangeFields(title: "Episodes", bounds: query.episodes, unit: nil, compact: compact) { bounds in
                        store.editSearch { $0.episodes = bounds }
                    }
                    Presets(
                        choices: [Bounds(min: 1, max: 1), Bounds(min: nil, max: 12), Bounds(min: 13, max: 26), Bounds(min: 50, max: nil)],
                        current: query.episodes, words: { SearchQuery(episodes: $0).label(.episodes) }, compact: compact
                    ) { bounds in store.editSearch { $0.episodes = bounds } }
                    RangeFields(title: "Episode length", bounds: query.minutes, unit: "min", compact: compact) { bounds in
                        store.editSearch { $0.minutes = bounds }
                    }
                    Presets(
                        choices: [Bounds(min: nil, max: 15), Bounds(min: 20, max: 30), Bounds(min: 60, max: nil)],
                        current: query.minutes, words: { SearchQuery(minutes: $0).label(.minutes) }, compact: compact
                    ) { bounds in store.editSearch { $0.minutes = bounds } }
                }
                PanelSection(compact: compact, "Reception") {
                    LabeledContent {
                        HStack(spacing: 10) {
                            Slider(
                                value: Binding(
                                    get: { Double(store.search.query.minScore ?? 0) },
                                    set: { value in store.editSearch { $0.minScore = value < 1 ? nil : UInt8(value) } }
                                ),
                                in: 0...95, step: 5
                            )
                            Text(query.minScore.map { "\($0)%" } ?? "Any")
                                .monospacedDigit()
                                .foregroundStyle(.haze)
                                .frame(width: 40, alignment: .trailing)
                        }
                    } label: {
                        Text("Score at least")
                    }
                    PanelSwitch("Hide obscure titles", isOn: Binding(
                        get: { store.search.query.minPopularity != nil },
                        set: { on in store.editSearch { $0.minPopularity = on ? obscureBelow : nil } }
                    ))
                }
                PanelSection(compact: compact, "Origin") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Source material").foregroundStyle(.haze)
                        FlowLayout(spacing: 6, lineSpacing: 6) {
                            ForEach(searchSources(), id: \.self) { source in
                                ToggleChip(name: searchSourceLabel(source: source), on: query.sources.contains(source), compact: compact) {
                                    store.searchLive(store.search.query.togglingSource(source: source))
                                }
                            }
                        }
                    }
                    .padding(.vertical, 4)
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Country").foregroundStyle(.haze)
                        FlowLayout(spacing: 6, lineSpacing: 6) {
                            ForEach(searchCountries(), id: \.self) { code in
                                ToggleChip(name: searchCountryLabel(code: code), on: query.countries.contains(code), compact: compact) {
                                    store.searchLive(store.search.query.togglingCountry(code: code))
                                }
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
                PanelSection(compact: compact, "Where to watch") {
                    let sites = facets?.streamingSites ?? []
                    if !sites.isEmpty {
                        FlowLayout(spacing: 6, lineSpacing: 6) {
                            ForEach(sites, id: \.self) { site in
                                ToggleChip(name: site, on: query.streamingOn.contains(site), compact: compact) {
                                    store.searchLive(store.search.query.togglingStreaming(site: site))
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    PanelSwitch("Licensed only", isOn: Binding(
                        get: { store.search.query.licensedOnly },
                        set: { on in store.editSearch { $0.licensedOnly = on } }
                    ))
                }
                if store.accounts.summary.state != .signedOut {
                    PanelSection(compact: compact, "My list") {
                        Picker("Titles on my list", selection: Binding<OnList?>(
                            get: { store.search.query.onList },
                            set: { value in store.editSearch { $0.onList = value } }
                        )) {
                            Text("Show").tag(OnList?.none)
                            Text("Hide").tag(OnList?.some(.hide))
                            Text("Only").tag(OnList?.some(.only))
                        }
                        .pickerStyle(.segmented)
                    }
                }
                Text("Updates as you change it.")
                    .font(compact ? .ui(13) : .mac(11))
                    .foregroundStyle(.haze)
            }
            .padding(compact ? 16 : 14)
        }
        .scrollIndicators(.never)
        .background(Palette.ground)
        .tint(.amber)
        .toggleStyle(.switch)
    }

    /// The panel's name and Reset; on iPhone, Done too, as a sheet has.
    @ViewBuilder
    private func header(_ query: SearchQuery) -> some View {
        if compact {
            ZStack {
                Text("Filters").font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                HStack {
                    Button("Reset") { reset() }
                        .disabled(!query.hasFilters())
                    Spacer()
                    Button("Done") { navigator.searchFiltersShown = false }
                        .fontWeight(.semibold)
                }
                .font(.ui(17))
                .foregroundStyle(.silk)
            }
            .frame(minHeight: 44)
        } else {
            HStack {
                Text("Filters").font(.mac(15, weight: .semibold)).foregroundStyle(.silk)
                Spacer()
                Button("Reset") { reset() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .disabled(!query.hasFilters())
            }
        }
    }

    @ViewBuilder
    private func tags(_ query: SearchQuery, _ facets: SearchFacets?) -> some View {
        PanelSection(compact: compact, "Tags") {
            TextField("Find a tag", text: $find)
                .textFieldStyle(.plain)
                .padding(.horizontal, 10)
                .frame(minHeight: compact ? 44 : 28)
                .background(Palette.raised, in: .rect(cornerRadius: 8))
            Stepper(value: Binding(
                get: { Int(store.search.query.minTagRank ?? defaultTagRank) },
                set: { rank in
                    store.editSearch { $0.minTagRank = rank == Int(defaultTagRank) ? nil : UInt8(rank) }
                }
            ), in: 0...100, step: 10) {
                Text("Tag rank at least \(Int(query.minTagRank ?? defaultTagRank))%")
                    .monospacedDigit()
            }
            .disabled(query.tags.isEmpty && query.tagsExcluded.isEmpty)
            let groups = facets?.tagGroups ?? []
            let words = find.trimmingCharacters(in: .whitespaces).lowercased()
            if words.isEmpty {
                ForEach(groups, id: \.category) { group in
                    let set = group.tags.filter { query.tagPick(name: $0.name) != .off }.count
                    DisclosureGroup {
                        ForEach(group.tags.filter { spoilers || !$0.spoiler }, id: \.name) { tag in
                            TagRow(tag: tag, pick: query.tagPick(name: tag.name), compact: compact) { pick in
                                store.searchLive(store.search.query.pickingTag(name: tag.name, pick: pick))
                            }
                        }
                    } label: {
                        HStack {
                            Text(group.category)
                            Spacer()
                            if set > 0 { Text("\(set)").monospacedDigit().foregroundStyle(.haze) }
                        }
                    }
                    .disclosureGroupStyle(PanelDisclosureStyle(compact: compact))
                }
            } else {
                let found = groups.flatMap(\.tags)
                    .filter { (spoilers || !$0.spoiler) && $0.name.lowercased().contains(words) }
                    .prefix(40)
                if found.isEmpty {
                    Text("No tag has that in its name").foregroundStyle(.haze)
                }
                ForEach(Array(found), id: \.name) { tag in
                    TagRow(tag: tag, pick: query.tagPick(name: tag.name), compact: compact) { pick in
                        store.searchLive(store.search.query.pickingTag(name: tag.name, pick: pick))
                    }
                }
            }
            PanelSwitch("Show spoiler tags", isOn: $spoilers)
        }
    }
}

/// A switch at the end of its row, its label at the start.
private struct PanelSwitch: View {
    let title: String
    let isOn: Binding<Bool>

    init(_ title: String, isOn: Binding<Bool>) {
        self.title = title
        self.isOn = isOn
    }

    var body: some View {
        Toggle(isOn: isOn) {
            // The whole row turns it, its words too, not the switch alone.
            Text(title)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
                .onTapGesture { isOn.wrappedValue.toggle() }
        }
        .toggleStyle(.switch)
    }
}

/// A group of tags that opens and closes from anywhere on its row: its name, its count and its
/// chevron, where the Mac's own disclosure answers on the chevron alone.
private struct PanelDisclosureStyle: DisclosureGroupStyle {
    var compact = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Button {
                withAnimation(reduceMotion ? nil : .quick) { configuration.isExpanded.toggle() }
            } label: {
                HStack(spacing: 8) {
                    #if os(macOS)
                    chevron(configuration.isExpanded)
                    configuration.label
                    #else
                    configuration.label
                    chevron(configuration.isExpanded)
                    #endif
                }
                .frame(maxWidth: .infinity, minHeight: compact ? 44 : 22, alignment: .leading)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .accessibilityValue(configuration.isExpanded ? "Expanded" : "Collapsed")
            .accessibilityHint(configuration.isExpanded ? "Closes the group" : "Opens the group")
            if configuration.isExpanded {
                VStack(alignment: .leading, spacing: 10) {
                    configuration.content
                }
                #if os(macOS)
                .padding(.leading, 18)
                #endif
            }
        }
    }

    private func chevron(_ expanded: Bool) -> some View {
        Image(systemName: "chevron.right")
            .font(compact ? .ui(13, weight: .semibold) : .mac(10, weight: .semibold))
            .foregroundStyle(.haze)
            .rotationEffect(.degrees(expanded ? 90 : 0))
            .frame(width: 10)
    }
}

/// A group of filters under its heading, on a raised card.
private struct PanelSection<Content: View>: View {
    var compact = false
    let title: String
    @ViewBuilder let content: Content

    init(compact: Bool, _ title: String, @ViewBuilder content: () -> Content) {
        self.compact = compact
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: compact ? 10 : 8) {
            // Search's own section heading, as over its rails and recent searches: sentence
            // case, the hierarchy from type and space, with no box around the section.
            Text(title)
                .font(compact ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 10) {
                content
            }
            .font(compact ? .ui(17) : .mac(13))
            .foregroundStyle(.silk)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// A tag: off, in or out, a tap moving it on; what it means as its help.
private struct TagRow: View {
    let tag: FacetTag
    let pick: Pick
    var compact = false
    let set: (Pick) -> Void

    var body: some View {
        Button {
            set(pick.next)
        } label: {
            HStack(spacing: 10) {
                Image(systemName: symbol)
                    .foregroundStyle(pick == .off ? .haze : pick == .include ? .amber : .vermilion)
                    .frame(width: 18)
                Text(tag.name).foregroundStyle(.silk)
                Spacer()
                if let word = pick.word {
                    Text(word).font(compact ? .ui(13) : .mac(11)).foregroundStyle(.haze)
                }
            }
            .frame(minHeight: compact ? 44 : 22)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .help(tag.description)
        .contextMenu {
            Button("Include") { set(.include) }
            Button(compact ? "Leave out" : "Leave Out") { set(.exclude) }
            Button("Off") { set(.off) }
        }
        .accessibilityValue(pick.word ?? "Off")
        .accessibilityHint("Tap to include, again to leave out")
    }

    private var symbol: String {
        switch pick {
        case .off: "circle"
        case .include: "checkmark.circle.fill"
        case .exclude: "minus.circle.fill"
        }
    }
}

/// A genre as a chip: off, in or out, a tap moving it on.
private struct PickChip: View {
    let name: String
    let pick: Pick
    var compact = false
    let set: (Pick) -> Void

    var body: some View {
        Button {
            set(pick.next)
        } label: {
            HStack(spacing: 4) {
                switch pick {
                case .include: Image(systemName: "checkmark")
                case .exclude: Image(systemName: "minus")
                case .off: EmptyView()
                }
                Text(name)
            }
            .font(compact ? .ui(15) : .mac(12))
            .foregroundStyle(pick == .off ? .silk : pick == .include ? .amber : .vermilion)
            .padding(.horizontal, 10)
            .frame(height: compact ? 32 : 24)
            .background(background, in: .capsule)
            .overlay(Capsule().strokeBorder(pick == .off ? Palette.faintRule : .clear))
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button("Include") { set(.include) }
            Button(compact ? "Leave out" : "Leave Out") { set(.exclude) }
            Button("Off") { set(.off) }
        }
        .accessibilityValue(pick.word ?? "Off")
    }

    private var background: Color {
        switch pick {
        case .off: .clear
        case .include: Palette.selection
        case .exclude: Palette.vermilion.opacity(0.14)
        }
    }
}

/// A choice that is on or off, as a chip.
private struct ToggleChip: View {
    let name: String
    let on: Bool
    var compact = false
    let toggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            Text(name)
                .font(compact ? .ui(15) : .mac(12))
                .foregroundStyle(on ? .amber : .silk)
                .padding(.horizontal, 10)
                .frame(height: compact ? 32 : 24)
                .background(on ? Palette.selection : .clear, in: .capsule)
                .overlay(Capsule().strokeBorder(on ? .clear : Palette.faintRule))
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// A range: from and to, either open.
private struct RangeFields: View {
    let title: String
    let bounds: Bounds
    let unit: String?
    var compact = false
    let set: (Bounds) -> Void

    var body: some View {
        LabeledContent(title) {
            HStack(spacing: 6) {
                field("From", \.min)
                // Never broken a letter to a line where the panel is narrow.
                Text("to").foregroundStyle(.haze).fixedSize()
                field("To", \.max)
                if let unit { Text(unit).foregroundStyle(.haze).fixedSize() }
            }
        }
    }

    private func field(_ prompt: String, _ side: WritableKeyPath<Bounds, UInt32?>) -> some View {
        TextField(prompt, value: Binding(
            get: { bounds[keyPath: side] },
            set: { value in
                var next = bounds
                next[keyPath: side] = value
                set(next)
            }
        ), format: IntegerFormatStyle<UInt32>().grouping(.never))
        .multilineTextAlignment(.trailing)
        .monospacedDigit()
        .frame(width: compact ? 64 : 56)
        #if os(iOS)
        .keyboardType(.numberPad)
        #endif
    }
}

/// Ranges to pick with a tap, each in the engine's words.
private struct Presets: View {
    let choices: [Bounds]
    let current: Bounds
    let words: (Bounds) -> String?
    var compact = false
    let set: (Bounds) -> Void

    var body: some View {
        FlowLayout(spacing: 6, lineSpacing: 6) {
            ForEach(choices, id: \.self) { bounds in
                ToggleChip(name: words(bounds) ?? "", on: current == bounds, compact: compact) {
                    set(current == bounds ? Bounds() : bounds)
                }
            }
        }
        .padding(.vertical, 2)
    }
}
