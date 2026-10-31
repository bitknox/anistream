// Home: the first section of every app. The engine sends the whole view (the spotlight, the
// sections in order with their headings, the closing line) and this draws it. It decides nothing
// about order, counts or which section shows (native-ux, Home).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Whether Home lays out for a desktop: the Mac and an iPad that is not in a compact window.
private struct HomeWideKey: EnvironmentKey {
    static let defaultValue = true
}

extension EnvironmentValues {
    var homeWide: Bool {
        get { self[HomeWideKey.self] }
        set { self[HomeWideKey.self] = newValue }
    }
}

/// The sizes Home draws at, by surface (native-ux, Home).
enum HomeMetrics {
    static func card(wide: Bool) -> CGSize { wide ? CGSize(width: 272, height: 153) : CGSize(width: 240, height: 135) }
    static func poster(wide: Bool) -> CGSize { wide ? CGSize(width: 128, height: 192) : CGSize(width: 110, height: 165) }
    static func margin(wide: Bool) -> CGFloat { wide ? 24 : 16 }
    /// The space between shelves.
    static func gap(wide: Bool) -> CGFloat { wide ? 36 : 28 }
    /// The spotlight's art takes 56% of the content width on a desktop.
    static let artShare: CGFloat = 0.56
}

struct HomeScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.playerUp) private var playerUp
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif
    @State private var width: CGFloat = 1_000

    private var wide: Bool {
        #if os(iOS)
        sizeClass != .compact
        #else
        true
        #endif
    }

    var body: some View {
        let margin = HomeMetrics.margin(wide: wide)
        ScrollView {
            VStack(alignment: .leading, spacing: HomeMetrics.gap(wide: wide)) {
                HomeContent(contentWidth: max(0, width - margin * 2))
            }
            .padding(.horizontal, margin)
            .padding(.top, 8)
            .padding(.bottom, 32)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollIndicators(.never)
        .environment(\.homeWide, wide)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
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

    /// Under the title: sync on an iPhone.
    private var subtitle: String {
        #if os(iOS)
        wide ? "" : store.syncSubtitle()
        #else
        ""
        #endif
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
    }

    @ViewBuilder
    private func loaded(_ view: HomeView) -> some View {
        let leaving = store.discovery.leaving
        #if os(iOS)
        if view.offline, let since = view.offlineSince {
            HomeOfflineLine(since: since)
        }
        #endif
        if let spot = view.spotlight, !leaving.contains(spot.entry.id) {
            HomeSpotlightView(spot: spot, contentWidth: contentWidth)
                .id(spot.entry.id)
        }
        ForEach(view.sections, id: \.kind) { section in
            let items = section.items.filter { !leaving.contains($0.entry.id) }
            if !items.isEmpty {
                HomeSectionView(section: section, items: items, season: view.season)
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

    private enum Stage { case waiting, placeholders }
    @State private var stage = Stage.waiting
    @State private var slow = false

    var body: some View {
        Group {
            if active, stage == .placeholders {
                VStack(alignment: .leading, spacing: HomeMetrics.gap(wide: true)) {
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

/// "Offline. Showing Home from 14:20." in haze beside the glyph, under the title. The Mac says
/// it in the toolbar's subtitle.
#if os(iOS)
private struct HomeOfflineLine: View {
    let since: Int64

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "wifi.slash").accessibilityHidden(true)
            Text(L.homeOffline(time: AirTime.clock(Date(timeIntervalSince1970: TimeInterval(since)))))
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

/// The spotlight as it will stand: art and text, static and redacted.
struct HomeSpotlightPlaceholder: View {
    @Environment(\.homeWide) private var wide
    let contentWidth: CGFloat

    var body: some View {
        let art = wide ? contentWidth * HomeMetrics.artShare : contentWidth
        let plate = RoundedRectangle(cornerRadius: ArtShape.bannerRadius).fill(.raised)
            .frame(width: art, height: art * 9 / 16)
        let text = VStack(alignment: .leading, spacing: 8) {
            Text("Continue watching").font(.ui(13, weight: .medium)) // i18n-skip
            Text("Hoshikuzu no Tabibito").font(.showTitle(size: wide ? 34 : 26, relativeTo: .largeTitle)) // i18n-skip
            Text("Episode 3 · 12 min left").font(.ui(15)) // i18n-skip
        }
        .foregroundStyle(.haze)
        if wide {
            HStack(alignment: .center, spacing: 32) {
                plate
                text
                Spacer(minLength: 0)
            }
        } else {
            VStack(alignment: .leading, spacing: 14) {
                plate
                text
            }
        }
    }
}

/// A shelf as it will stand: the heading and a row of cards or posters.
struct HomeShelfPlaceholder: View {
    @Environment(\.homeWide) private var wide
    let posters: Bool

    var body: some View {
        let size = posters ? HomeMetrics.poster(wide: wide) : HomeMetrics.card(wide: wide)
        VStack(alignment: .leading, spacing: 12) {
            Text("This season").font(.ui(20, weight: .semibold)) // i18n-skip
            HStack(alignment: .top, spacing: 12) {
                ForEach(0..<(posters ? 6 : 3), id: \.self) { _ in
                    VStack(alignment: .leading, spacing: 6) {
                        RoundedRectangle(cornerRadius: posters ? ArtShape.coverRadius(width: size.width) : ArtShape.bannerRadius)
                            .fill(.raised)
                            .frame(width: size.width, height: size.height)
                        Text("Kumo no Ue no Toshokan").font(.showTitle(size: 15, relativeTo: .body)) // i18n-skip
                            .lineLimit(1)
                        Text("Episode 4 out").font(.ui(13)) // i18n-skip
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
