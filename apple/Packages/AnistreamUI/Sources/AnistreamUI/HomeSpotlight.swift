// The spotlight: the one title at the top of Home. Its art is 16:9 at radius 16, with the text
// beside it on a desktop and under it on a phone. The one amber on the view is its main action.
// There is no scrim or gradient: text never sits on art.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

struct HomeSpotlightView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeWide) private var wide
    let spot: Spotlight
    let contentWidth: CGFloat

    var body: some View {
        let entry = store.titles[spot.entry.id] ?? spot.entry
        let art = wide ? contentWidth * HomeMetrics.artShare : contentWidth
        let facts = HomeFacts(playable: store.discovery.playable(entry.id, sent: spot.playable), action: spot.action)
        Group {
            if wide {
                HStack(alignment: .center, spacing: 32) {
                    artView(entry, width: art)
                    text(entry, facts: facts)
                        // Exactly what the art leaves, even where the words need a hair more:
                        // the row never comes out wider than Home, which sizes the art by it.
                        .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack(alignment: .leading, spacing: 16) {
                    artView(entry, width: art)
                    text(entry, facts: facts)
                }
            }
        }
        .accessibilityElement(children: .contain)
        .task(id: entry.id) {
            store.probe(entry.id)
            if !store.titles.detailed.contains(entry.id) { store.loadDetail(entry.id) }
        }
    }

    // MARK: Art

    private func artView(_ entry: Entry, width: CGFloat) -> some View {
        let hero = store.heroArt(entry)
        return Button {
            navigator.openTitle(entry.id)
        } label: {
            Group {
                if hero.isCompact {
                    CoverArt(entry: entry, width: width * 0.4)
                        .frame(width: width, height: width * 9 / 16, alignment: .center)
                } else {
                    HeroArtView(art: hero, entry: entry)
                        .frame(width: width, height: width * 9 / 16)
                        .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                }
            }
            .matchedTransitionSourceIfAvailable(id: entry.id)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityHidden(true)
        .frame(width: width, height: width * 9 / 16)
    }

    // MARK: Text and actions

    private func text(_ entry: Entry, facts: HomeFacts) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(spot.label.text)
                .font(.ui(wide ? 13 : 15, weight: .medium))
                .foregroundStyle(.haze)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 4) {
                Text(entry.showTitle)
                    .font(.showTitle(size: wide ? 34 : 28, relativeTo: .largeTitle))
                    .foregroundStyle(.silk)
                    .lineLimit(3)
                    .dynamicTypeSize(...DynamicTypeSize.accessibility3)
                    .fixedSize(horizontal: false, vertical: true)
                if let line = lineText(facts: facts) {
                    HStack(spacing: 6) {
                        if spot.case == .continuing, facts.playable != .notFound, spot.resume == nil {
                            Circle().fill(.water).frame(width: 6, height: 6).accessibilityHidden(true)
                        }
                        Text(line)
                    }
                    .font(.ui(wide ? 15 : 17))
                    .foregroundStyle(.haze)
                }
            }
            HomeSynopsis(entry: entry, lines: wide ? 3 : 2)
            actions(entry, facts: facts)
                .padding(.top, 4)
        }
        .frame(maxWidth: wide ? 440 : .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }

    private func lineText(facts: HomeFacts) -> String? {
        if facts.playable == .notFound { return L.homeNotFound }
        if let line = spot.line { return line.text }
        return HomeWords.starts(spot.starts)
    }

    @ViewBuilder
    private func actions(_ entry: Entry, facts: HomeFacts) -> some View {
        let main = HomeMain(action: facts.action, episode: spot.episode)
        let row = HStack(spacing: 10) {
            if let main {
                HomeMainButton(main: main, entry: entry, wide: wide, stretch: !wide)
                    .layoutPriority(1)
            }
            if wide { secondary(entry) }
            moreMenu(entry)
        }
        if wide {
            row
        } else {
            VStack(alignment: .leading, spacing: 10) {
                row
                HStack(spacing: 10) { secondary(entry) }
            }
        }
    }

    @ViewBuilder
    private func secondary(_ entry: Entry) -> some View {
        Button(L.homeDetails) { navigator.openTitle(entry.id) }
            .buttonStyle(SecondaryCapsuleStyle(height: wide ? 34 : 44, fontSize: wide ? 13 : 17))
        if entry.trailerUrl != nil {
            Button(L.titleTrailer) { navigator.playTrailer(entry, store: store) }
                .buttonStyle(SecondaryCapsuleStyle(height: wide ? 34 : 44, fontSize: wide ? 13 : 17))
        }
    }

    private func moreMenu(_ entry: Entry) -> some View {
        Menu {
            if spot.case == .continuing {
                if spot.action == .resume || spot.action == .play {
                    Button(L.homeMarkWatched(episode: Int(spot.episode))) {
                        store.send(.setWatched(id: entry.id, episodes: [String(spot.episode)], watched: true))
                        navigator.watchedByHand += 1
                    }
                }
                Button(L.homeRemoveContinue) { store.hideFromContinue(entry.id) }
            } else {
                Button(L.upNextNotInterested) { store.notInterested(entry, title: entry.showTitle, unaired: spot.starts != nil) }
            }
        } label: {
            Image(systemName: "ellipsis")
                .font(.ui(wide ? 13 : 17, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: wide ? 34 : 44, height: wide ? 34 : 44)
                .background(Palette.raised, in: .circle)
                .contentShape(.circle)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(L.homeMore)
    }
}

/// What a card or the spotlight does once the sources check has spoken: a miss turns Play into
/// Add to Planning.
struct HomeFacts {
    let playable: Playable
    let action: HomeAction

    init(playable: Playable, action: HomeAction) {
        self.playable = playable
        self.action = playable == .notFound && (action == .play || action == .resume) ? .addToPlanning : action
    }
}

/// The main action as a button says it, or `nil` where nothing can be pressed.
struct HomeMain {
    let action: HomeAction
    let episode: UInt32

    init?(action: HomeAction, episode: UInt32) {
        guard action != .nothing else { return nil }
        self.action = action
        self.episode = episode
    }

    var title: String {
        switch action {
        case .resume: L.homeResumeEpisode(episode: Int(episode))
        case .play: L.homePlayEpisode(episode: Int(episode))
        case .addToPlanning, .nothing: L.libraryAddPlanning
        }
    }

    var symbol: String { action == .addToPlanning ? "plus" : "play.fill" }
}

/// The one amber button of the view. Add to Planning says Adding for the moment it saves.
struct HomeMainButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let main: HomeMain
    let entry: Entry
    let wide: Bool
    var stretch = false
    @State private var adding = false

    var body: some View {
        Button {
            switch main.action {
            case .resume, .play:
                store.send(.play(id: entry.id, episode: String(main.episode)))
            case .addToPlanning:
                adding = true
                store.send(.setStatus(id: entry.id, status: .planning))
            case .nothing:
                break
            }
        } label: {
            HStack(spacing: 8) {
                if adding {
                    ProgressView().controlSize(.small).tint(Palette.ground)
                } else {
                    Image(systemName: main.symbol).font(.ui(wide ? 11 : 14))
                }
                Text(adding ? L.homeAdding : main.title)
                    .lineLimit(1)
                    .fixedSize(horizontal: !stretch, vertical: false)
            }
            .frame(maxWidth: stretch ? .infinity : nil)
        }
        .buttonStyle(PrimaryCapsuleStyle(height: wide ? 34 : 48, fontSize: wide ? 13 : 17))
        .disabled(adding)
        .pointerHover()
    }
}

/// The synopsis; text AniList marks as a spoiler is the Discussions spoiler bar, which opens the
/// title for the visit.
struct HomeSynopsis: View {
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let lines: Int

    var body: some View {
        let shown = entry.shownSynopsis
        VStack(alignment: .leading, spacing: 6) {
            if !shown.isEmpty {
                Text(shown)
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                    .lineLimit(lines)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if entry.synopsis.contains("~!") {
                Button {
                    navigator.openTitle(entry.id)
                } label: {
                    Text(L.discussionSpoiler)
                        .font(.ui(13, weight: .medium))
                        .foregroundStyle(.haze)
                        .padding(.horizontal, 10)
                        .frame(height: 22)
                        .background(Palette.raisedSelected, in: .rect(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityHint(L.discussionAppleSpoilerHint)
            }
        }
    }
}

enum HomeWords {
    /// "3 October" or "October 2026", when a title has not started airing.
    static func starts(_ date: FuzzyDate?) -> String? {
        guard let date, let month = date.month, (1...12).contains(Int(month)) else {
            return date.map { String($0.year) }
        }
        var parts = DateComponents(year: Int(date.year), month: Int(month), day: date.day.map(Int.init))
        parts.calendar = .current
        guard let day = parts.date else { return nil }
        let style: Date.FormatStyle = date.day == nil
            ? .dateTime.month(.wide).year()
            : .dateTime.day().month(.wide)
        return day.formatted(style)
    }
}
