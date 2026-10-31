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
    @Environment(\.homeLayout) private var layout
    let spot: Spotlight
    let contentWidth: CGFloat

    var body: some View {
        let entry = store.titles[spot.entry.id] ?? spot.entry
        let facts = HomeFacts(playable: store.discovery.playable(entry.id, sent: spot.playable), action: spot.action)
        let beside = HomeSpotlightView.beside(layout, contentWidth: contentWidth)
        Group {
            if beside {
                // The text first and the art on the right, the text centred on the art.
                HStack(alignment: .center, spacing: layout.spotGap) {
                    text(entry, facts: facts, beside: true)
                        // Exactly what the art leaves, even where the words need a hair more:
                        // the row never comes out wider than Home, which sizes the art by it.
                        .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                    artView(entry, width: contentWidth * layout.artShare)
                }
            } else {
                VStack(alignment: .leading, spacing: layout.spotGap) {
                    artView(entry, width: contentWidth)
                    text(entry, facts: facts, beside: false)
                }
            }
        }
        .accessibilityElement(children: .contain)
        .task(id: entry.id) {
            store.probe(entry.id)
            if !store.titles.detailed.contains(entry.id) { store.loadDetail(entry.id) }
        }
    }

    /// Whether the text sits beside the art: on a desktop, while what the art leaves is wide
    /// enough to read. A narrow window stacks it, as a phone does.
    static func beside(_ layout: HomeLayout, contentWidth: CGFloat) -> Bool {
        layout.wide && contentWidth * (1 - layout.artShare) - layout.spotGap >= HomeLayout.textMinimum
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

    /// The label, the title and its other title, the facts, the synopsis, how far in or what is
    /// out, then the actions; a miss from the sources check under them.
    private func text(_ entry: Entry, facts: HomeFacts, beside: Bool) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(spot.label.text)
                .font(.ui(layout.spotLabel, weight: .medium))
                .foregroundStyle(.haze)
                .accessibilityAddTraits(.isHeader)
                .padding(.bottom, 8)
            Text(entry.showTitle)
                .font(.showTitle(size: layout.spotTitle.size, relativeTo: .largeTitle))
                .fixedLineHeight(layout.spotTitle.line, relativeTo: .largeTitle)
                .foregroundStyle(.silk)
                .lineLimit(2)
                .dynamicTypeSize(...DynamicTypeSize.accessibility3)
                .fixedSize(horizontal: false, vertical: true)
            if let other = entry.otherTitles.first {
                Text(other)
                    .font(.ui(layout.spotOther.size))
                    .fixedLineHeight(layout.spotOther.line, relativeTo: .footnote)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 4)
            }
            if !entry.hoverFacts.isEmpty {
                Text(entry.hoverFacts.uppercased())
                    .font(.ui(11, weight: .medium))
                    .tracking(0.88)
                    .fixedLineHeight(16, relativeTo: .caption2)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 10)
            }
            HomeSynopsis(entry: entry, lines: layout.synopsisLines)
                .padding(.top, 12)
            progress(facts: facts)
                .padding(.top, 16)
            actions(entry, facts: facts, beside: beside)
                .padding(.top, 18)
            if facts.playable == .notFound {
                Text(L.homeNotFound)
                    .font(.ui(layout.secondary))
                    .foregroundStyle(.haze)
                    .padding(.top, 12)
            }
        }
        .frame(maxWidth: beside ? layout.textWidth : .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }

    /// Under the synopsis: the track and the time left for a resume, "Episode 5 is out" in silk
    /// with the dot for a new episode, or when a title starts.
    @ViewBuilder
    private func progress(facts: HomeFacts) -> some View {
        let line = spot.line?.text
        if spot.case == .continuing, let line {
            HStack(spacing: 10) {
                if let fraction = spot.resume?.fraction {
                    Meter(fraction: fraction, height: 4, spoken: line, silk: true)
                        .frame(width: layout.track)
                    Text(line).foregroundStyle(.haze)
                } else {
                    Circle().fill(.water).frame(width: 6, height: 6).accessibilityHidden(true)
                    Text(line).foregroundStyle(.silk)
                }
            }
            .font(.ui(layout.secondary))
            .lineLimit(1)
        } else if let line = line ?? HomeWords.starts(spot.starts) {
            Text(line)
                .font(.ui(layout.secondary))
                .foregroundStyle(.haze)
                .lineLimit(1)
        }
    }

    @ViewBuilder
    private func actions(_ entry: Entry, facts: HomeFacts, beside: Bool) -> some View {
        let main = HomeMain(action: facts.action, episode: spot.episode)
        if layout.wide {
            // One row while it fits; else the main action over the others, never a squeezed
            // "D..." beside it.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    if let main { HomeMainButton(main: main, entry: entry) }
                    secondary(entry, stretch: false)
                    moreMenu(entry)
                }
                VStack(alignment: .leading, spacing: 10) {
                    if let main { HomeMainButton(main: main, entry: entry) }
                    HStack(spacing: 10) {
                        secondary(entry, stretch: false)
                        moreMenu(entry)
                    }
                }
            }
        } else {
            // The phone: the main action across, then Details and More in one row.
            VStack(spacing: 12) {
                if let main { HomeMainButton(main: main, entry: entry, stretch: true) }
                HStack(spacing: 12) {
                    secondary(entry, stretch: true)
                    moreMenu(entry)
                }
            }
        }
    }

    @ViewBuilder
    private func secondary(_ entry: Entry, stretch: Bool) -> some View {
        let size = layout.secondaryButton
        Button {
            navigator.openTitle(entry.id)
        } label: {
            Text(L.homeDetails).lineLimit(1).frame(maxWidth: stretch ? .infinity : nil)
        }
        .buttonStyle(SecondaryCapsuleStyle(height: size.height, fontSize: size.font))
        .fixedSize(horizontal: !stretch, vertical: false)
        if entry.trailerUrl != nil {
            Button {
                navigator.playTrailer(entry, store: store)
            } label: {
                Text(L.titleTrailer).lineLimit(1).frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: size.height, fontSize: size.font))
            .fixedSize(horizontal: !stretch, vertical: false)
        }
    }

    private func moreMenu(_ entry: Entry) -> some View {
        let side = layout.secondaryButton.height
        return Menu {
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
                .font(.ui(layout.secondaryButton.font, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: side, height: side)
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
    @Environment(\.homeLayout) private var layout
    let main: HomeMain
    let entry: Entry
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
                    Image(systemName: main.symbol).font(.ui(layout.button.font < 15 ? 11 : 14))
                }
                Text(adding ? L.homeAdding : main.title)
                    .lineLimit(1)
                    .fixedSize(horizontal: !stretch, vertical: false)
            }
            .frame(maxWidth: stretch ? .infinity : nil)
        }
        .buttonStyle(PrimaryCapsuleStyle(height: layout.button.height, fontSize: layout.button.font))
        .disabled(adding)
        .pointerHover()
    }
}

/// The synopsis; text AniList marks as a spoiler is the Discussions spoiler bar, which opens the
/// title for the visit.
struct HomeSynopsis: View {
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let entry: Entry
    let lines: Int

    var body: some View {
        // One run of text: a paragraph break inside the few lines shown would spend a line on
        // nothing.
        let shown = entry.shownSynopsis.replacing(/\s*\n+\s*/, with: " ")
        VStack(alignment: .leading, spacing: 6) {
            if !shown.isEmpty {
                Text(shown)
                    .font(.ui(layout.synopsis.size))
                    .fixedLineHeight(layout.synopsis.line, relativeTo: .subheadline)
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
