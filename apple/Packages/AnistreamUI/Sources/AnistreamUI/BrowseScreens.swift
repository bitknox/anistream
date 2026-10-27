// Seasonal and Library: titles as covers, and what Search shares with them.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

// MARK: - Seasonal

/// This season's most popular titles.
struct SeasonalScreen: View {
    @Environment(AppStore.self) private var store
    /// The narrowest a column of covers may be.
    var minimum: CGFloat = 128

    var body: some View {
        ScrollView {
            ListStates(
                state: store.seasonal.state,
                placeholder: AnyView(PosterGridPlaceholder(minimum: minimum)),
                activity: "loading this season", // the engine job this list waits on
                failure: L.seasonalLoadFailed,
                retry: { store.loadSeasonal() }
            ) { ids in
                PosterGrid(entries: ids.compactMap { store.titles[$0] }, minimum: minimum, spacing: minimum < 128 ? 12 : 20) { entry in
                    PosterCell(entry: entry, detail: entry.genres.first, width: minimum)
                }
            } empty: {
                QuietLine(L.seasonalEmptyTitle(season: store.seasonalName))
            }
            .padding(EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16))
        }
        .scrollIndicators(.never)
        .pullToRefresh(store, .seasonal)
        .task { if store.seasonal.state == .idle { store.loadSeasonal() } }
    }
}

/// Seasonal's way into Search: the same season, with every filter to narrow it.
struct SeasonalToolbar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.search(.season(), in: store)
        } label: {
            Label(L.seasonalFilterInSearch, systemImage: "line.3.horizontal.decrease")
                .labelStyle(.titleAndIcon)
        }
        .help(L.seasonalFilterHelp)
    }
}

extension AppStore {
    /// Which season Seasonal is: "Autumn 2026".
    var seasonalName: String {
        let query = SearchQuery.season()
        guard let season = query.season, let year = query.years.max else { return "" }
        return L.seasonWithYear(season: TitleWords.season(season), year: String(year))
    }

    /// The same, once Seasonal has its titles.
    var seasonName: String? {
        seasonal.caption == nil ? nil : seasonalName
    }
}

// MARK: - Library

/// How the Library orders a list.
enum LibrarySort: String, CaseIterable, Identifiable {
    case lastWatched = "Last Watched" // i18n-skip
    case title = "Title" // i18n-skip
    case score = "Score" // i18n-skip

    var id: String { rawValue }

    /// The name the sort goes by in a menu.
    var name: String {
        switch self {
        case .lastWatched: L.librarySortUpdated
        case .title: L.librarySortTitle
        case .score: L.librarySortScore
        }
    }
}

/// The tracker's lists, a grid each; or, signed out, where they would come from.
struct LibraryScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var narrow: Bool { sizeClass == .compact }
    #else
    private var narrow: Bool { false }
    #endif
    @AppStorage("anistream.librarySort") private var sort: LibrarySort = .lastWatched
    /// The grid alone, inside a screen of its own, as the iPhone's Library has it.
    var embedded = false

    var body: some View {
        Group {
            if embedded {
                grid
            } else if store.accounts.summary.state == .signedOut {
                SignedOutLibrary()
            } else {
                ScrollView {
                    grid.padding(EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16))
                }
                .scrollIndicators(.never)
            }
        }
        .task {
            for segment in LibraryModel.order where store.library.segment(segment).state == .idle {
                store.loadLibrary(segment)
            }
        }
    }

    private var grid: some View {
        let minimum: CGFloat = narrow ? 100 : 128
        let segment = navigator.librarySegment
        return ListStates(
            state: store.library.segment(segment).state,
            placeholder: AnyView(PosterGridPlaceholder(minimum: minimum)),
            activity: "loading your \(segment.label) list", // the engine job this list waits on
            failure: L.libraryListLoadFailed(list: segment.label),
            retry: { store.loadLibrary(segment) }
        ) { ids in
            PosterGrid(entries: sorted(ids.compactMap { store.titles[$0] }), minimum: minimum,
                       spacing: minimum < 128 ? 12 : 20) { entry in
                PosterCell(
                    entry: entry,
                    detail: libraryLine(entry, in: navigator.librarySegment),
                    fresh: freshLine(entry),
                    fraction: meter(entry, in: navigator.librarySegment),
                    width: minimum
                )
            }
        } empty: {
            QuietLine(L.libraryListEmpty(list: segment.label))
        }
        // A list of its own: switching to one already there shows it at once.
        .id(segment)
    }

    private func sorted(_ entries: [Entry]) -> [Entry] {
        switch sort {
        case .lastWatched: entries
        case .title: entries.sorted { $0.showTitle.localizedStandardCompare($1.showTitle) == .orderedAscending }
        case .score: entries.sorted { ($0.score ?? 0) > ($1.score ?? 0) }
        }
    }

    private func libraryLine(_ entry: Entry, in segment: LibrarySegment) -> String? {
        let done = entry.progress?.done ?? 0
        switch segment {
        case .completed:
            return entry.episodes.map { L.episodesCount(count: Int($0)) }
        case .planning:
            return [entry.format.map(TitleWords.format), entry.year.map(String.init)].compactMap { $0 }.joined(separator: " · ")
        case .watching, .paused, .dropped:
            let next = done + 1
            return entry.episodes.map { L.progressEpisodesShort(done: Int(min(next, $0)), total: Int($0)) }
                ?? L.progressEpisodeShort(done: Int(next))
        }
    }

    private func meter(_ entry: Entry, in segment: LibrarySegment) -> Double? {
        switch segment {
        case .watching, .paused, .dropped: entry.watchedFraction
        case .planning, .completed: nil
        }
    }

    private func freshLine(_ entry: Entry) -> String? {
        let next = (entry.progress?.done ?? 0) + 1
        guard let aired = entry.latestAired, aired.episode == next, let ago = aired.ago,
              ago < ContinueQueue.week
        else { return nil }
        return L.calendarLineOut(episode: Int(next), ago: Wording.ago(ago))
    }
}

/// The Library with no tracker signed in: Nagare, where the lists come from, and the one action.
struct SignedOutLibrary: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(spacing: 0) {
            EmptyState(
                title: L.librarySignedOutTitle,
                line: L.librarySignedOutDetail
            ) {
                Button(L.librarySignedOutAction) {
                    store.send(.connect(tracker: "anilist"))
                }
                .buttonStyle(PrimaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
            }
            LegalLine()
                .padding(.bottom, 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The one legal line, everywhere it appears.
struct LegalLine: View {
    var body: some View {
        Text(L.legalHostsNoContent)
            .font(.mac(13))
            .foregroundStyle(.haze)
            .multilineTextAlignment(.center)
    }
}

/// A quiet line where a list has nothing.
struct QuietLine: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.mac(13))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, minHeight: 120)
    }
}

// MARK: - Shared

/// A heading over a group on a screen: 15 semibold on the Mac.
struct SectionHeading: View {
    let text: String
    var compact = false

    init(_ text: String, compact: Bool = false) {
        self.text = text
        self.compact = compact
    }

    var body: some View {
        Text(text)
            .font(compact ? .ui(20, weight: .semibold) : .mac(15, weight: .semibold))
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }
}

/// A raised capsule with a hairline edge, filling its cell: a genre, a filter.
struct ChipStyle: ButtonStyle {
    var selected = false
    var height: CGFloat = 32
    var fontSize: CGFloat = 13

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.board(fontSize))
            .foregroundStyle(selected ? .amber : .silk)
            .frame(maxWidth: .infinity)
            .frame(minHeight: height)
            .background(
                selected ? Palette.selection : configuration.isPressed ? Palette.raisedSelected : Palette.raised,
                in: .capsule
            )
            .overlay(Capsule().strokeBorder(.faintRule, lineWidth: 1))
            .contentShape(.capsule)
            .pointerHover()
    }
}

extension String {
    /// The first letter upper case and the rest as it is in sentence case: "Slice of life".
    var capitalizedSentence: String {
        let lower = lowercased()
        return lower.prefix(1).uppercased() + lower.dropFirst()
    }
}
