// Seasonal, Library and Search: titles as covers.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Seasonal

/// This season's most popular titles.
struct SeasonalScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        ScrollView {
            ListStates(state: store.seasonal.state) { ids in
                PosterGrid(entries: ids.compactMap { store.titles[$0] }) { entry in
                    PosterCell(entry: entry, detail: entry.genres.first)
                }
            } empty: {
                QuietLine("Nothing is airing this season yet")
            }
            .padding(EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16))
        }
        .scrollIndicators(.never)
        .task { if store.seasonal.state == .idle { store.loadSeasonal() } }
    }
}

extension AppStore {
    /// Which season Seasonal is, as the engine says it, in title case: "Fall 2026".
    var seasonName: String? {
        seasonal.caption.map { caption in
            String(caption.split(separator: "·").first ?? Substring(caption))
                .trimmingCharacters(in: .whitespaces)
                .capitalized
        }
    }
}

// MARK: - Library

/// How the Library orders a list.
enum LibrarySort: String, CaseIterable, Identifiable {
    case lastWatched = "Last Watched"
    case title = "Title"
    case score = "Score"

    var id: String { rawValue }
}

/// The tracker's lists, a grid each; or, signed out, where they would come from.
struct LibraryScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @AppStorage("anistream.librarySort") private var sort: LibrarySort = .lastWatched

    var body: some View {
        Group {
            if store.accounts.summary.state == .signedOut {
                SignedOutLibrary()
            } else {
                ScrollView {
                    ListStates(state: store.library.segment(navigator.librarySegment).state) { ids in
                        PosterGrid(entries: sorted(ids.compactMap { store.titles[$0] })) { entry in
                            PosterCell(
                                entry: entry,
                                detail: libraryLine(entry, in: navigator.librarySegment),
                                fresh: freshLine(entry),
                                fraction: meter(entry, in: navigator.librarySegment)
                            )
                        }
                    } empty: {
                        QuietLine("Nothing on this list")
                    }
                    .padding(EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16))
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
            return entry.episodes.map { Wording.plural(Int($0), "episode") }
        case .planning:
            return [entry.format, entry.year.map(String.init)].compactMap { $0 }.joined(separator: " · ")
        case .watching, .paused, .dropped:
            let next = done + 1
            return entry.episodes.map { "Ep \(min(next, $0)) of \($0)" } ?? "Ep \(next)"
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
        return "Ep \(next) out \(Wording.ago(ago))"
    }
}

/// The Library with no tracker signed in: Nagare, where the lists come from, and the one action.
struct SignedOutLibrary: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(spacing: 0) {
            Spacer(minLength: 24)
            Nagare(cell: 6)
            Text("Your lists come from AniList")
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(.silk)
                .padding(.top, 24)
            Text("Connect AniList to see your Watching, Planning and Completed lists here.")
                .font(.system(size: 15))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)
                .padding(.top, 6)
            Button("Connect AniList") {
                store.send(.connect(tracker: "anilist"))
            }
            .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
            .padding(.top, 20)
            Spacer(minLength: 24)
            LegalLine()
                .padding(.bottom, 24)
        }
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The one legal line, everywhere it appears.
struct LegalLine: View {
    var body: some View {
        Text("anistream ships no sources of its own and hosts no content. Every source is external.")
            .font(.system(size: 13))
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
            .font(.system(size: 13))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, minHeight: 120)
    }
}

// MARK: - Search

/// The genres Search offers before anything is typed, as AniList names them.
let searchGenres = [
    "Action", "Adventure", "Comedy", "Drama", "Fantasy", "Mystery", "Romance", "Slice of Life",
    "Sports",
]

/// Search: before anything is typed, recent searches, this season and the genres; then the
/// titles found.
struct SearchScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        ScrollView {
            Group {
                if store.search.submitted == nil {
                    SearchStart()
                } else {
                    VStack(alignment: .leading, spacing: 16) {
                        if let caption = searchCaption {
                            Text(caption)
                                .font(.system(size: 13))
                                .foregroundStyle(.haze)
                        }
                        ListStates(state: store.search.list.state) { ids in
                            PosterGrid(entries: ids.compactMap { store.titles[$0] }) { entry in
                                PosterCell(entry: entry, detail: searchLine(entry))
                            }
                        } empty: {
                            QuietLine("Nothing found for \u{201c}\(store.search.submitted?.query ?? "")\u{201d}")
                        }
                    }
                }
            }
            .padding(EdgeInsets(top: 8, leading: 12, bottom: 24, trailing: 16))
        }
        .scrollIndicators(.never)
        .task {
            if store.search.submitted == nil, !navigator.query.isEmpty {
                store.search(navigator.query)
            }
            if store.seasonal.state == .idle { store.loadSeasonal() }
        }
    }

    private var searchCaption: String? {
        guard let submitted = store.search.submitted else { return nil }
        if let genre = submitted.filter.genre, submitted.query.isEmpty { return genre }
        return nil
    }

    private func searchLine(_ entry: Entry) -> String? {
        if let status = entry.listStatus {
            let watching = status == .watching ? entry.progress.map { " · Ep \($0.next)" } ?? "" : ""
            return status.label + watching
        }
        return [entry.format, entry.year.map(String.init)].compactMap { $0 }.joined(separator: " · ")
    }
}

/// Search before anything is typed.
struct SearchStart: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    private let recents = RecentSearches.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            if !recents.queries.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        SectionHeading("Recent Searches")
                        Spacer()
                        Button("Clear") { recents.clear() }
                            .buttonStyle(.plain)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(.silk)
                    }
                    ForEach(recents.queries, id: \.self) { query in
                        HStack {
                            Button {
                                navigator.search(query, in: store)
                            } label: {
                                Label(query, systemImage: "clock")
                                    .font(.system(size: 13))
                                    .foregroundStyle(.silk)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .contentShape(.rect)
                            }
                            .buttonStyle(.plain)
                            Button("Remove \(query)", systemImage: "xmark") { recents.remove(query) }
                                .labelStyle(.iconOnly)
                                .buttonStyle(.plain)
                                .foregroundStyle(.haze)
                        }
                        .frame(height: 28)
                    }
                }
            }
            let season = store.seasonal.state.ids.compactMap { store.titles[$0] }
            if !season.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    HStack(alignment: .firstTextBaseline) {
                        SectionHeading("This Season")
                        Spacer()
                        if let name = store.seasonName {
                            Text(name).font(.system(size: 13)).foregroundStyle(.haze)
                        }
                    }
                    PosterGrid(entries: season) { entry in
                        PosterCell(entry: entry, detail: entry.genres.first)
                    }
                }
            }
            VStack(alignment: .leading, spacing: 12) {
                SectionHeading("Genres")
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 8)], spacing: 8) {
                    ForEach(searchGenres, id: \.self) { genre in
                        Button(genre.capitalizedSentence) {
                            navigator.section = .search
                            store.search("", filter: SearchFilters(genre: genre))
                        }
                        .buttonStyle(ChipStyle())
                    }
                }
            }
        }
    }
}

/// A heading over a group on a screen: 15 semibold on the Mac.
struct SectionHeading: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.system(size: 15, weight: .semibold))
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }
}

/// A raised capsule with a hairline edge, filling its cell: a genre, a filter.
struct ChipStyle: ButtonStyle {
    var selected = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13))
            .foregroundStyle(selected ? .amber : .silk)
            .frame(maxWidth: .infinity)
            .frame(height: 32)
            .background(
                selected ? Palette.selection : configuration.isPressed ? Palette.raisedSelected : Palette.raised,
                in: .capsule
            )
            .overlay(Capsule().strokeBorder(.faintRule, lineWidth: 1))
            .contentShape(.capsule)
    }
}

extension String {
    /// The first letter upper case and the rest as it is in sentence case: "Slice of life".
    var capitalizedSentence: String {
        let lower = lowercased()
        return lower.prefix(1).uppercased() + lower.dropFirst()
    }
}
