// Home: the first section of every app. The engine sends the whole view (the spotlight, the
// sections in order with their headings, the closing line) and this draws it. It decides nothing
// about order, counts or which section shows (native-ux, Home).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Which of the board's four layouts Home draws: the Mac, an iPad across and upright, a phone.
enum HomeSurface: Equatable {
    case mac, padWide, padUpright, phone

    /// An iPad's column is the upright board's below 960 points, as in a portrait window.
    static func touch(regular: Bool, column: CGFloat) -> HomeSurface {
        regular ? (column < 960 ? .padUpright : .padWide) : .phone
    }
}

/// The sizes Home draws at, by surface, as board v3 sets them (native-ux, Home).
struct HomeLayout: Equatable {
    var surface: HomeSurface

    /// The Mac and an iPad that is not in a compact window: shelves page, the spotlight sits
    /// beside its text.
    var wide: Bool { surface != .phone }
    var margin: CGFloat { wide ? 24 : 16 }
    /// The space between shelves.
    var shelfGap: CGFloat { pick(40, 44, 40, 36) }
    var cardGap: CGFloat { pick(16, 20, 16, 12) }
    var card: CGSize { let w: CGFloat = pick(272, 300, 260, 260); return CGSize(width: w, height: (w * 9 / 16).rounded()) }
    var cardRadius: CGFloat { pick(8, 10, 10, 12) }
    var poster: CGSize { let w: CGFloat = pick(128, 140, 128, 112); return CGSize(width: w, height: w * 1.5) }
    /// Posters keep the app's cover radius (8 at these widths) where the board has 10 on touch.
    var posterRadius: CGFloat { ArtShape.coverRadius(width: poster.width) }

    // The spotlight.
    /// The art's share of the column beside its text: 560 of 992, 620 of 1116, 392 of 772.
    var artShare: CGFloat { pick(0.565, 0.556, 0.508, 1) }
    var spotGap: CGFloat { pick(40, 44, 28, 16) }
    var spotTitle: (size: CGFloat, line: CGFloat) { pick((30, 38), (34, 42), (28, 34), (24, 30)) }
    var spotLabel: CGFloat { pick(13, 15, 15, 13) }
    var spotOther: (size: CGFloat, line: CGFloat) { pick((13, 18), (15, 20), (15, 20), (13, 18)) }
    var synopsis: (size: CGFloat, line: CGFloat) { pick((13, 20), (15, 22), (15, 22), (15, 20)) }
    var synopsisLines: Int { wide ? 3 : 2 }
    var textWidth: CGFloat { pick(420, 440, 360, .infinity) }
    var track: CGFloat { pick(200, 200, 140, 120) }
    var button: (height: CGFloat, font: CGFloat) { pick((34, 13), (44, 17), (44, 17), (50, 17)) }
    var secondaryButton: (height: CGFloat, font: CGFloat) { pick((34, 13), (44, 17), (44, 17), (44, 15)) }
    /// The narrowest the text beside the art may be; below it the spotlight stacks.
    static let textMinimum: CGFloat = 320

    // Headings and cards.
    var heading: (size: CGFloat, line: CGFloat, weight: Font.Weight) {
        pick((17, 22, .semibold), (22, 28, .bold), (22, 28, .bold), (20, 25, .bold))
    }
    /// Words beside a heading, links and lines under a card.
    var secondary: CGFloat { pick(13, 15, 15, 15) }
    var cardTitle: (size: CGFloat, line: CGFloat) { pick((15, 20), (17, 22), (17, 22), (15, 20)) }
    var cardLine: (size: CGFloat, line: CGFloat) { pick((13, 18), (15, 20), (15, 20), (13, 18)) }
    var posterTitle: (size: CGFloat, line: CGFloat) { pick((14, 18), (15, 20), (15, 20), (15, 20)) }
    var posterLine: (size: CGFloat, line: CGFloat) { pick((12, 16), (13, 18), (13, 18), (13, 18)) }

    // This week.
    /// Columns side by side; the phone scrolls a strip of all seven.
    var weekColumns: Int { pick(7, 7, 4, 7) }
    var weekCover: CGFloat { surface == .mac ? 40 : 44 }

    private func pick<T>(_ mac: T, _ padWide: T, _ padUpright: T, _ phone: T) -> T {
        switch surface {
        case .mac: mac
        case .padWide: padWide
        case .padUpright: padUpright
        case .phone: phone
        }
    }
}

private struct HomeLayoutKey: EnvironmentKey {
    static let defaultValue = HomeLayout(surface: .mac)
}

extension EnvironmentValues {
    var homeLayout: HomeLayout {
        get { self[HomeLayoutKey.self] }
        set { self[HomeLayoutKey.self] = newValue }
    }

    /// Whether Home lays out for a desktop: the Mac and an iPad that is not in a compact window.
    var homeWide: Bool { homeLayout.wide }
}

extension View {
    /// The width this view is offered, as it changes: what the column gives it, never what its
    /// content asks for. Home sizes the spotlight's art by this width, so a measure its content
    /// could move would feed back: content a hair wider than the column widens a scroll view, the
    /// art grows with it, and the layout never settles.
    func onOfferedWidth(_ action: @escaping (CGFloat) -> Void) -> some View {
        frame(minWidth: 0, maxWidth: .infinity)
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { action($0) }
    }
}

struct HomeScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.playerUp) private var playerUp
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif
    @State private var width: CGFloat = 1_000

    private var layout: HomeLayout {
        #if os(iOS)
        HomeLayout(surface: .touch(regular: sizeClass != .compact, column: width - 48))
        #else
        HomeLayout(surface: .mac)
        #endif
    }

    private var wide: Bool { layout.wide }

    var body: some View {
        let layout = self.layout
        let margin = layout.margin
        ScrollView {
            VStack(alignment: .leading, spacing: layout.shelfGap) {
                HomeContent(contentWidth: max(0, width - margin * 2))
            }
            .padding(.horizontal, margin)
            .padding(.top, 8)
            .padding(.bottom, 32)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollIndicators(.never)
        .environment(\.homeLayout, layout)
        .onOfferedWidth { width = $0 }
        .pullToRefresh(store, .home)
        #if os(macOS)
        .hoverCards()
        #endif
        #if os(iOS)
        .screenSubtitle(subtitle)
        .toolbar { if !wide { AccountButton() } }
        #endif
        .task {
            store.loadHome()
            store.loadContinue()
        }
        // A watch moves the queue on: asked again once the player has gone.
        .onChange(of: playerUp) { _, up in if !up { store.loadHome(force: true) } }
        .onChange(of: store.offline) { was, now in
            // The connection came back: one refresh.
            if was, !now { store.loadHome(force: true) }
        }
    }

    /// Under the title on an iPhone: sync, or offline in Home's words, the one place it is said.
    private var subtitle: String {
        #if os(iOS)
        wide ? "" : store.homeOfflineLine ?? store.syncSubtitle()
        #else
        ""
        #endif
    }
}

extension AppStore {
    /// "Offline. Showing Home from 14:20.", while Home shows what it kept. Offline is said once
    /// on Home: the app's own offline word (the Mac's sync capsule, the iPhone's subtitle) carries
    /// this line there, and Home adds none of its own. An iPad, with no such word, says it over
    /// Home's content.
    var homeOfflineLine: String? {
        guard let view = discovery.view, view.offline, let since = view.offlineSince else { return nil }
        return L.homeOffline(time: AirTime.clock(Date(timeIntervalSince1970: TimeInterval(since))))
    }
}

/// What Home holds once the engine has answered, or the placeholders before.
private struct HomeContent: View {
    @Environment(AppStore.self) private var store
    @Environment(\.homeWide) private var wide
    let contentWidth: CGFloat

    var body: some View {
        let home = store.discovery
        Group {
            if let view = home.view {
                if view.offline, view.offlineSince == nil, view.spotlight == nil, view.sections.isEmpty {
                    HomeOfflineEmpty()
                } else {
                    loaded(view)
                }
            } else {
                HomeWait(active: home.isLoading || home.view == nil, activity: "loading home") {
                    HomeSpotlightPlaceholder(contentWidth: contentWidth)
                    HomeShelfPlaceholder(posters: false)
                    HomeShelfPlaceholder(posters: true)
                }
            }
        }
        .animation(.standard, value: home.view == nil)
        // The spotlight crossfades when its pick changes, and the shelves close or open the gap
        // on the same spring when a section comes or goes (Home board, prototype 2).
        .animation(.standard, value: home.spotlightChanges)
        .animation(.standard, value: home.view?.sections.map(\.kind) ?? [])
    }

    @ViewBuilder
    private func loaded(_ view: HomeView) -> some View {
        let leaving = store.discovery.leaving
        #if os(iOS)
        if wide, let line = store.homeOfflineLine {
            HomeOfflineLine(line: line)
        }
        #endif
        if let spot = view.spotlight, !leaving.contains(spot.entry.id) {
            // The old and the new spotlight in one place while they crossfade, not one under the
            // other.
            ZStack(alignment: .topLeading) {
                HomeSpotlightView(spot: spot, contentWidth: contentWidth)
                    .id(spot.entry.id)
                    .transition(.opacity)
            }
            .transition(.opacity)
        }
        ForEach(view.sections, id: \.kind) { section in
            let items = section.items.filter { !leaving.contains($0.entry.id) }
            if !items.isEmpty {
                HomeSectionView(section: section, items: items, season: view.season)
                    .transition(.opacity)
            }
        }
        if !view.pending.isEmpty {
            // What is still on its way, below what has arrived: placeholders from 600 ms in
            // their own place, so nothing above moves when it lands.
            HomeWait(active: true, activity: "loading home") {
                HomeShelfPlaceholder(posters: true)
            }
        }
        if let end = view.end {
            HomeEndLine(end: end)
                .transition(.opacity)
        }
    }
}

// MARK: - Waiting

/// Placeholders by the States rules: nothing for 600 ms, then the placeholders in the real layout,
/// held at least 0.3 s, replaced in one crossfade, and past 4 s the activity line under them.
private struct HomeWait<Placeholder: View>: View {
    let active: Bool
    let activity: String
    @ViewBuilder let placeholder: () -> Placeholder
    @Environment(\.homeLayout) private var layout

    private enum Stage { case waiting, placeholders }
    @State private var stage = Stage.waiting
    @State private var slow = false

    var body: some View {
        Group {
            if active, stage == .placeholders {
                VStack(alignment: .leading, spacing: layout.shelfGap) {
                    placeholder()
                    if slow { ActivityLine(activity: activity).padding(.top, -12) }
                }
                .redacted(reason: .placeholder)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
                .transition(.opacity)
            }
        }
        .task(id: active) {
            guard active else {
                stage = .waiting
                slow = false
                return
            }
            try? await Task.sleep(for: JobsModel.delay)
            guard !Task.isCancelled else { return }
            withAnimation(.standard) { stage = .placeholders }
            try? await Task.sleep(for: .seconds(4) - JobsModel.delay)
            guard !Task.isCancelled else { return }
            withAnimation(.fadeIn) { slow = true }
        }
    }
}

// MARK: - Offline and the end

/// "Offline. Showing Home from 14:20." in haze beside the glyph, over Home on an iPad. The Mac's
/// sync capsule and the iPhone's subtitle say it there instead (`AppStore.homeOfflineLine`).
#if os(iOS)
private struct HomeOfflineLine: View {
    let line: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "wifi.slash").accessibilityHidden(true)
            Text(line)
        }
        .font(.plat(mac: 13, touch: 15))
        .foregroundStyle(.haze)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.bottom, -12)
        .transition(.opacity)
    }
}

#endif

/// Offline with nothing saved: Nagare, "You're offline", the line and Open Downloads.
private struct HomeOfflineEmpty: View {
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    var body: some View {
        EmptyState(title: L.homeOfflineEmptyTitle, line: L.homeOfflineEmptyBody) {
            Button(L.homeOpenDownloads) {
                navigator.showDownloads(compact: compact)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
        .frame(maxWidth: .infinity, minHeight: 420)
    }

    private var compact: Bool {
        #if os(iOS)
        sizeClass == .compact
        #else
        false
        #endif
    }
}

/// The closing line: Seasonal and Calendar as links, Sign in to AniList when no tracker is
/// signed in, or Plugin stores when no source is installed.
private struct HomeEndLine: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let end: HomeEnd

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch end {
            case .everything, .signIn:
                Text(L.homeEnd)
                HStack(spacing: 20) {
                    link(L.homeSeasonal) { navigator.open(.seasonal) }
                    link(L.homeFullCalendar) { navigator.open(.calendar) }
                }
                if end == .signIn {
                    Text(L.homeEndSignedOut)
                    Button(L.homeSignIn) { store.send(.connect(tracker: "anilist")) }
                        .buttonStyle(SecondaryCapsuleStyle(height: 40, fontSize: 15))
                }
            case .noSources:
                Text(L.homeEndNoSources)
                link(L.navPluginStores) { navigator.open(.pluginStores) }
            }
        }
        .font(.ui(15))
        .foregroundStyle(.haze)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 4)
    }

    private func link(_ title: String, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .buttonStyle(.plain)
            .foregroundStyle(.silk)
            .font(.ui(15, weight: .medium))
            .pointerHover()
    }
}

// MARK: - Placeholders

/// The spotlight as it will stand: text and art, static and redacted.
struct HomeSpotlightPlaceholder: View {
    @Environment(\.homeLayout) private var layout
    let contentWidth: CGFloat

    var body: some View {
        let beside = HomeSpotlightView.beside(layout, contentWidth: contentWidth)
        let art = beside ? contentWidth * layout.artShare : contentWidth
        let plate = RoundedRectangle(cornerRadius: ArtShape.bannerRadius).fill(.raised)
            .frame(width: art, height: art * 9 / 16)
        let text = VStack(alignment: .leading, spacing: 8) {
            Text("Continue watching").font(.ui(layout.spotLabel, weight: .medium)) // i18n-skip
            Text("Hoshikuzu no Tabibito").font(.showTitle(size: layout.spotTitle.size, relativeTo: .largeTitle)) // i18n-skip
            Text("Episode 3 · 12 min left").font(.ui(layout.secondary)) // i18n-skip
        }
        .foregroundStyle(.haze)
        if beside {
            HStack(alignment: .center, spacing: layout.spotGap) {
                // What the art leaves, as the spotlight's own text has.
                text.frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                plate
            }
        } else {
            VStack(alignment: .leading, spacing: layout.spotGap) {
                plate
                text
            }
        }
    }
}

/// A shelf as it will stand: the heading and a row of cards or posters.
struct HomeShelfPlaceholder: View {
    @Environment(\.homeLayout) private var layout
    let posters: Bool

    var body: some View {
        let size = posters ? layout.poster : layout.card
        VStack(alignment: .leading, spacing: 12) {
            Text("This season").font(.ui(layout.heading.size, weight: layout.heading.weight)) // i18n-skip
            HStack(alignment: .top, spacing: layout.cardGap) {
                ForEach(0..<(posters ? 6 : 3), id: \.self) { _ in
                    VStack(alignment: .leading, spacing: 6) {
                        RoundedRectangle(cornerRadius: posters ? layout.posterRadius : layout.cardRadius)
                            .fill(.raised)
                            .frame(width: size.width, height: size.height)
                        Text("Kumo no Ue no Toshokan").font(.showTitle(size: layout.cardTitle.size, relativeTo: .body)) // i18n-skip
                            .lineLimit(1)
                        Text("Episode 4 out").font(.ui(layout.cardLine.size)) // i18n-skip
                    }
                    .frame(width: size.width, alignment: .leading)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .clipped()
        }
        .foregroundStyle(.haze)
    }
}
