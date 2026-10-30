// Continue on the Mac and iPad: the queue down the left, and the selected title beside it.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Continue in a wide window: the queue, grouped In progress and New this week, and a preview of
/// the selected title with the one amber action, what it plays.
struct ContinueScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.playerUp) private var playerUp
    @State private var selection: AnilistId? = LaunchRoute.current.selected
    /// When the selection last moved, and whether that was quick on the heels of the one before:
    /// a run of arrow keys changes the preview at once, without the fade (M2).
    @State private var movedAt = ContinuousClock.now
    /// The width the screen has: below 780 points, as in Split View or a narrow window, the list
    /// stands alone, the width of the screen, and a row opens its title.
    @State private var width: CGFloat = 1_000

    /// Whether the preview fits beside the list: the queue's 360 points and at least 380 for the
    /// preview, as an iPad in portrait has.
    private var previewFits: Bool {
        #if os(macOS)
        true
        #else
        width >= 780
        #endif
    }

    /// The selection as the list moves it: the preview fades in (M2), but not when the last
    /// move was under 0.15 s before, so a held arrow key runs through the list at once. The
    /// first step of a run fades; the next step cuts it.
    private var moving: Binding<AnilistId?> {
        Binding(
            get: { selection },
            set: { id in
                let now = ContinuousClock.now
                let fast = now - movedAt < .milliseconds(150)
                movedAt = now
                withAnimation(fast ? nil : .fadeIn) { selection = id }
            }
        )
    }

    var body: some View {
        let queue = store.continuing.queue(store.titles)
        ListStates(
            state: store.continuing.list.state,
            placeholder: AnyView(QueueColumn(queue: nil, selection: moving).frame(maxWidth: .infinity, alignment: .leading)),
            activity: "loading continue watching", // the engine job this list waits on
            failure: L.continueLoadFailed,
            retry: { store.loadContinue(force: true) },
            // In line with the queue's group headers.
            offlineInset: 16
        ) { _ in
            if queue.isEmpty {
                NothingToContinue()
            } else {
                HStack(alignment: .top, spacing: 0) {
                    QueueColumn(queue: queue, selection: moving, opensOnTap: !previewFits)
                    if previewFits, let item = queue.all.first(where: { $0.id == selection }) ?? queue.all.first {
                        ContinuePreview(item: item)
                            .id(item.id)
                            .transition(.opacity)
                    }
                }
                #if os(iOS)
                // On the Mac the sidebar sets the queue off the edge; an iPad has none.
                .padding(.leading, 12)
                #endif
            }
        } empty: {
            ScrollView {
                WelcomeContinue()
                    .padding(EdgeInsets(top: 4, leading: 12, bottom: 24, trailing: 16))
            }
            .scrollIndicators(.never)
            #if os(macOS)
            .hoverCards()
            #endif
        }
        // The ground under the whole screen, however narrow the list.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task { store.loadContinue() }
        // A watch moves the queue on: asked again once the player has gone, fresh or not.
        .onChange(of: playerUp) { _, up in if !up { store.loadContinue(force: true) } }
        .onChange(of: queue.all.map(\.id), initial: true) { _, ids in
            // Kept while the queue loads, so a selection made before it arrived survives.
            if !ids.isEmpty, selection == nil || !ids.contains(where: { $0 == selection }) {
                selection = ids.first
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .onChange(of: navigator.continueSelection) { _, id in
            if let id { moving.wrappedValue = id; navigator.continueSelection = nil }
        }
        .onChange(of: selection, initial: true) { _, id in
            if let id { navigator.focusedTitle = id }
        }
        .task(id: selection) {
            // The page's fields (where else it streams), the episode's name and the stills for a
            // missing banner, once the selection rests for the standard spring's 0.3 s: an arrow
            // key held through the queue asks AniList nothing on the way.
            guard let id = selection else { return }
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
        }
    }
}

/// The queue down the left: 360 points wide, rows 62 high with 36 by 54 covers.
struct QueueColumn: View {
    @Environment(AppStore.self) private var store
    /// `nil` while the list loads: placeholder rows, static, no shimmer.
    let queue: ContinueQueue?
    @Binding var selection: AnilistId?
    /// Whether a row opens its title rather than selecting it: when there is no preview.
    var opensOnTap = false

    var body: some View {
        #if os(iOS)
        // On iPad a list, so the rows take their swipes (native-ux), drawn as the Mac's scroll
        // view is: the same insets, and 2 points between rows.
        List {
            rows(first: EdgeInsets(top: 4, leading: 4, bottom: 0, trailing: 8),
                 rest: EdgeInsets(top: 2, leading: 4, bottom: 0, trailing: 8))
            Color.clear.frame(height: 16).bareRow()
        }
        .bareList()
        .scrollIndicators(.never)
        .modifier(QueueKeys(queue: queue, selection: $selection, opensOnTap: opensOnTap))
        #else
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                rows(first: nil, rest: nil)
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
        .modifier(QueueKeys(queue: queue, selection: $selection, opensOnTap: opensOnTap))
        #endif
    }

    /// The headers and the rows, set in by `first` and `rest` in a list.
    @ViewBuilder
    private func rows(first: EdgeInsets?, rest: EdgeInsets?) -> some View {
        if let queue {
            if !queue.inProgress.isEmpty {
                GroupHeader(title: L.groupInProgress, first: true).listed(first)
                ForEach(queue.inProgress) {
                    QueueRow(item: $0, selection: $selection, opensOnTap: opensOnTap).swiped($0).listed(rest)
                }
            }
            if !queue.newThisWeek.isEmpty {
                GroupHeader(title: L.groupNewThisWeek, first: queue.inProgress.isEmpty).listed(queue.inProgress.isEmpty ? first : rest)
                ForEach(queue.newThisWeek) {
                    QueueRow(item: $0, selection: $selection, opensOnTap: opensOnTap).swiped($0).listed(rest)
                }
            }
        } else {
            GroupHeader(title: L.groupInProgress, first: true).listed(first)
            ForEach(0..<3, id: \.self) { _ in PlaceholderRow().listed(rest) }
        }
    }
}

/// The queue's width, focus and arrow keys, the same in a list and a scroll view.
private struct QueueKeys: ViewModifier {
    let queue: ContinueQueue?
    @Binding var selection: AnilistId?
    let opensOnTap: Bool

    func body(content: Content) -> some View {
        content
            .frame(width: opensOnTap ? nil : 360)
            .frame(maxWidth: opensOnTap ? .infinity : nil, alignment: .leading)
            .focusable()
            .listFocusEffect()
            .onKeyPress(.upArrow) { step(-1) }
            .onKeyPress(.downArrow) { step(1) }
    }

    /// Move the selection `delta` rows, stopping at the ends.
    private func step(_ delta: Int) -> KeyPress.Result {
        guard let ids = queue?.all.map(\.id), !ids.isEmpty else { return .ignored }
        let index = ids.firstIndex { $0 == selection } ?? 0
        selection = ids[min(max(index + delta, 0), ids.count - 1)]
        return .handled
    }
}

/// A group's header in a list: "In progress".
struct GroupHeader: View {
    let title: String
    var first = false

    var body: some View {
        Text(title)
            .groupHeaderStyle()
            .accessibilityAddTraits(.isHeader)
            .padding(EdgeInsets(top: first ? 10 : 18, leading: 12, bottom: 6, trailing: 12))
    }
}

/// One title in the queue: its cover, its title, what it plays and, when it was left part-way, how
/// far into the episode.
struct QueueRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem
    @Binding var selection: AnilistId?
    var opensOnTap = false

    var body: some View {
        let selected = selection == item.id && !opensOnTap
        Button {
            if opensOnTap { navigator.openTitle(item.id) } else { selection = item.id }
        } label: {
            HStack(spacing: 12) {
                ArtworkView(url: item.entry.coverUrl, color: item.entry.plateColor, maxPixelSize: 108)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 36)))
                VStack(alignment: .leading, spacing: 0) {
                    Text(item.entry.showTitle)
                        .font(.macShowTitle(size: 15))
                        .macLineHeight(20)
                        .foregroundStyle(selected ? .amber : .silk)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        if item.isNew || !item.playable {
                            Circle().fill(.water).frame(width: 6, height: 6)
                        }
                        Text(item.action)
                            .monospacedDigit()
                    }
                    .font(.mac(13))
                    .macLineHeight(18)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 2)
                    Held(value: item.fraction) { fraction in
                        if let fraction { Meter(fraction: fraction, height: 3, spoken: item.timeLeft).padding(.top, 6) }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .frame(height: 62)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .rowHover()
        // The tint moves at once, as in any Mac list (M2).
        .titleElement(item.entry, value: item.spokenProgress) { QueueMenu(item: item) }
        .accessibilityAddTraits(selected ? .isSelected : [])
        .contextMenu { QueueMenu(item: item) }
    }
}

/// A row's context menu, as MacHome draws it. What has no screen or player yet is there,
/// disabled, rather than missing.
struct QueueMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem

    var body: some View {
        let n = item.episode
        if item.playable {
            // Said as the row's button says it: "Resume episode 7 · Dubbed · 10:06".
            Button(item.buttonTitle) {
                store.send(.play(id: item.id, episode: String(n)))
            }
            // A part watched: the episode again from its first second, its place let go.
            if item.resume != nil {
                Button(L.episodePlayFromBeginning) {
                    store.send(.play(id: item.id, episode: String(n), fromStart: true))
                }
            }
            Button(L.episodeMarkNWatched(episode: Int(n))) {
                store.send(.setWatched(id: item.id, episodes: [String(n)], watched: true))
                navigator.watchedByHand += 1
            }
        } else {
            // Nothing to play yet: when it airs, said and not offered.
            Button(item.statusTitle) {}
                .disabled(true)
        }
        Divider()
        Button(L.titleOpen) { navigator.openTitle(item.id, fromPreview: true) }
        Divider()
        Button(L.continueHide) {
            store.hideFromContinue(item.id)
        }
        Button(L.continueDrop) {
            store.send(.setStatus(id: item.id, status: .dropped))
        }
    }
}

/// A row while the list loads.
struct PlaceholderRow: View {
    var body: some View {
        HStack(spacing: 12) {
            RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 36, height: 54)
            VStack(alignment: .leading, spacing: 2) {
                Text("Hoshikuzu no Tabibito").font(.macShowTitle(size: 15)) // i18n-skip
                Text("Resume E7 · 10:06").font(.mac(13)) // i18n-skip
            }
            .redacted(reason: .placeholder)
            .foregroundStyle(.haze)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .frame(height: 62)
        .accessibilityHidden(true)
    }
}

/// The selected title: its wide art (see `HeroArt`), the title block, the episode it plays and
/// how far into it, the action, and the synopsis. With no wide art the cover stands beside the
/// title block instead.
struct ContinuePreview: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.bannerSpace) private var bannerSpace
    let item: ContinueItem

    var body: some View {
        let entry = store.titles[item.id] ?? item.entry
        let art = store.heroArt(entry)
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if art.isCompact {
                    HStack(alignment: .bottom, spacing: 20) {
                        CoverArt(entry: entry, width: 120)
                            .onTapGesture { navigator.openTitle(entry.id, fromPreview: true) }
                        titleBlock(entry)
                    }
                } else {
                    // 290 high, or 16:9 where the preview is narrower, as on an iPad in portrait.
                    BannerFrame(maximum: 290) {
                        if navigator.titleOnMac == entry.id {
                            Color.clear
                        } else {
                            HeroArtView(art: art, entry: entry)
                                .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                                .modifier(BannerMatch(id: entry.id, space: bannerSpace))
                                .playerZoomSource(entry.id)
                                .onTapGesture { navigator.openTitle(entry.id, fromPreview: true) }
                        }
                    }
                    titleBlock(entry)
                }

                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text(item.episodeLine(store.titles))
                            .foregroundStyle(.silk)
                        Spacer()
                        Held(value: item.minutesLeft) { minutes in
                            if let minutes { MinutesLeft(minutes: minutes) }
                        }
                    }
                    .font(.mac(13))
                    Held(value: item.fraction) { fraction in
                        if let fraction { Meter(fraction: fraction, height: 4, spoken: item.timeLeft) }
                    }
                }

                HStack(spacing: 10) {
                    if item.playable {
                        Button {
                            store.send(.play(id: item.id, episode: String(item.episode)))
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.mac(10))
                                Text(item.buttonTitle)
                            }
                        }
                        .buttonStyle(PrimaryCapsuleStyle())
                        .keyboardShortcut(.defaultAction)
                        Button(L.titleEpisodes) { navigator.openTitle(entry.id, fromPreview: true) }
                            .buttonStyle(SecondaryCapsuleStyle())
                    } else {
                        // Caught up: when the next episode airs, and the episodes to go back to.
                        AirsStatus(text: item.statusTitle)
                        Button(L.titleEpisodes) { navigator.openTitle(entry.id, fromPreview: true) }
                            .buttonStyle(SecondaryCapsuleStyle())
                            .keyboardShortcut(.defaultAction)
                    }
                }

                if !entry.synopsis.isEmpty {
                    ClampedText(text: entry.synopsis)
                        .font(.mac(13))
                        .macLineHeight(20)
                        .foregroundStyle(.haze)
                        .frame(maxWidth: 620, alignment: .leading)
                        .id(entry.id)
                }
                if let alsoOn = entry.alsoOn {
                    Text(alsoOn)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
            }
            .padding(EdgeInsets(top: 4, leading: 12, bottom: 20, trailing: 24))
        }
        .scrollIndicators(.never)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        // The selection's episodes, which Continue asks for once it rests, bring the stills that
        // stand in for a missing banner.
    }

    private func titleBlock(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(entry.showTitle)
                .font(.macShowTitle(size: 30, relativeTo: .largeTitle))
                .macLineHeight(38, relativeTo: .largeTitle)
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            if let alternate = entry.alternateTitle {
                Text(alternate)
                    .font(.mac(15))
                    .foregroundStyle(.haze)
            }
            Text(entry.metadataLine)
                .groupHeaderStyle()
                .padding(.top, 4)
        }
    }
}

/// The wide art's frame: the width it is given, and `maximum` high, or 16:9 of the width where
/// that is lower, so a narrower column shows the same picture rather than a taller crop of it.
struct BannerFrame: Layout {
    var maximum: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? maximum * 16 / 9
        return CGSize(width: width, height: min(maximum, (width * 9 / 16).rounded()))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for subview in subviews {
            subview.place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
        }
    }
}

/// Continue when everything in it has been hidden: Nagare, and where to start.
struct NothingToContinue: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        EmptyState(title: L.continueEmptyTitle, line: L.continueEmptyDetail) {
            Button(L.continueEmptyAction) { navigator.open(.seasonal) }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }
}
