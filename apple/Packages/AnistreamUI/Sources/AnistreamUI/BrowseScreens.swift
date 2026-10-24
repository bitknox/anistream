// Seasonal and Library: titles as covers, and what Search shares with them.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Seasonal

/// This season's most popular titles.
struct SeasonalScreen: View {
    @Environment(AppStore.self) private var store
    /// The narrowest a column of covers may be.
    var minimum: CGFloat = 128

    var body: some View {
        ScrollView {
            ListStates(state: store.seasonal.state, placeholder: AnyView(PosterGridPlaceholder(minimum: minimum))) { ids in
                PosterGrid(entries: ids.compactMap { store.titles[$0] }, minimum: minimum, spacing: minimum < 128 ? 12 : 20) { entry in
                    PosterCell(entry: entry, detail: entry.genres.first, width: minimum)
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

/// Seasonal's way into Search: the same season, with every filter to narrow it.
struct SeasonalToolbar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.search(.season(), in: store)
        } label: {
            Label("Filter in Search", systemImage: "line.3.horizontal.decrease")
                .labelStyle(.titleAndIcon)
        }
        .help("This season in Search, where every filter narrows it")
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
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var narrow: Bool { sizeClass == .compact }
    #else
    private var narrow: Bool { false }
    #endif
    @AppStorage("anistream.librarySort") private var sort: LibrarySort = .lastWatched
    /// Just the grid, inside a screen of its own, as the iPhone's Library has it.
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
        return ListStates(
            state: store.library.segment(navigator.librarySegment).state,
            placeholder: AnyView(PosterGridPlaceholder(minimum: minimum))
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
            QuietLine("Nothing on this list")
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
                .font(.mac(17, weight: .semibold))
                .foregroundStyle(.silk)
                .padding(.top, 24)
            Text("Connect AniList to see your Watching, Planning and Completed lists here.")
                .font(.mac(15))
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
        Text("anistream hosts no content.")
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
