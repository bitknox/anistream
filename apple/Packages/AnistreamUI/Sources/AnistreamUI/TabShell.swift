// iPhone and iPad: the tabs, Search on its own, a stack per tab with a title's page pushed onto it
// by a zoom from its cover (T1).

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct TabShell: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        // The sidebar on a wide iPad; on iPhone the system's own tab bar, with Search apart.
        if sizeClass == .regular {
            tabs.tabViewStyle(.sidebarAdaptable)
        } else {
            tabs
        }
    }

    private var tabs: some View {
        @Bindable var navigator = navigator
        return TabView(selection: $navigator.section) {
            ForEach([AppSection.continue, .calendar, .seasonal, .library]) { section in
                Tab(section.title, systemImage: section.symbol, value: section) {
                    TabStack(section: section) { screen(section) }
                }
            }
            if sizeClass == .regular {
                TabSection("Sources") {
                    ForEach([AppSection.downloads, .providers, .pluginStores]) { section in
                        Tab(section.title.capitalizedSentence, systemImage: section.symbol, value: section) {
                            TabStack(section: section) { screen(section) }
                        }
                        .defaultVisibility(.hidden, for: .tabBar)
                    }
                }
                Tab(AppSection.settings.title, systemImage: AppSection.settings.symbol, value: AppSection.settings) {
                    TabStack(section: .settings) { SettingsScreen() }
                }
                .defaultVisibility(.hidden, for: .tabBar)
            }
            Tab(value: AppSection.search, role: .search) {
                TabStack(section: .search) { PhoneSearch() }
                    .searchable(text: $navigator.query, prompt: "Search titles")
                    .onSubmit(of: .search) { navigator.search(navigator.query, in: store) }
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        .overlay(alignment: .top) {
            NoticeCapsule().padding(.top, 8)
        }
        .sheet(isPresented: $navigator.messagesShown) {
            MessagesView()
        }
        .sheet(item: $navigator.phoneSettings) { page in
            PhoneSettingsSheet(start: page)
        }
        .sheet(item: Binding<ShowItem?>(
            get: { navigator.showSettings.flatMap { store.titles[$0] }.map(ShowItem.init) },
            set: { (item: ShowItem?) in navigator.showSettings = item?.entry.id }
        )) { item in
            PhoneShowSettingsSheet(entry: item.entry)
        }
        .moreAboutSheet(navigator)
        .modifier(AppSheets())
    }

    @ViewBuilder
    private func screen(_ section: AppSection) -> some View {
        let wide = sizeClass == .regular
        switch section {
        case .continue: if wide { ContinueScreen() } else { PhoneContinue() }
        case .calendar: if wide { CalendarScreen() } else { PhoneCalendar() }
        case .seasonal: SeasonalScreen(minimum: wide ? 128 : 100)
        case .library: PhoneLibrary()
        case .downloads: DownloadsScreen().toolbar { IPadDownloadsToolbar() }
        case .providers: ProvidersScreen().toolbar { IPadProvidersToolbar() }
        case .pluginStores: PluginStoresScreen().toolbar { IPadStoresToolbar() }
        case .settings: SettingsScreen()
        default: NotBuiltYet(section: section)
        }
    }
}

/// A title page's toolbar on iPad: the list it is on, and More.
struct PadTitleToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store
    let id: AnilistId

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Menu {
                StatusItems(id: id)
            } label: {
                HStack(spacing: 6) {
                    Text(store.titles[id]?.listStatus.map { $0.label.capitalizedSentence } ?? "Add to list")
                    Image(systemName: "chevron.up.chevron.down").font(.mac(11, weight: .semibold))
                }
            }
        }
        ToolbarItem(placement: .topBarTrailing) {
            Menu {
                if let entry = store.titles[id] { TitleMoreItems(entry: entry) }
            } label: {
                Label("More", systemImage: "ellipsis")
            }
        }
    }
}

/// Providers' toolbar on iPad: check the stores for plugin updates.
struct IPadProvidersToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button("Check for Updates") { store.loadStores() }
        }
    }
}

/// Plugin Stores' toolbar on iPad: join another.
struct IPadStoresToolbar: ToolbarContent {
    @Environment(Navigator.self) private var navigator

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button("Join Store", systemImage: "plus") { navigator.invite = "" }
        }
    }
}

/// Downloads' toolbar on iPad: pause or resume everything.
struct IPadDownloadsToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            let paused = store.downloads.rows.contains { $0.state == .paused }
            let running = store.downloads.rows.contains { $0.state == .active || $0.state == .queued }
            if running || !paused {
                Button("Pause All") { store.pauseAllDownloads() }.disabled(!running)
            } else {
                Button("Resume All") { store.resumeAllDownloads() }
            }
        }
    }
}

/// A tab's stack: its screen, with a title's page pushed onto it by a zoom from the cover that
/// opened it.
struct TabStack<Screen: View>: View {
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Namespace private var zoom
    let section: AppSection
    @ViewBuilder let screen: () -> Screen

    var body: some View {
        NavigationStack(path: Binding(
            get: { navigator.paths[section] ?? [] },
            set: { navigator.paths[section] = $0 }
        )) {
            screen()
                .navigationTitle(section.title.capitalizedSentence)
                .background(.ground)
                .navigationDestination(for: AnilistId.self) { id in
                    // A wide iPad has the Mac's page (PadTitleMeta); an iPhone and a narrow
                    // window, the iPhone's.
                    Group {
                        if sizeClass == .regular {
                            TitlePage(id: id).toolbar { PadTitleToolbar(id: id) }
                        } else {
                            PhoneTitle(id: id)
                        }
                    }
                    .navigationTransition(.zoom(sourceID: id, in: zoom))
                }
        }
        .environment(\.zoomNamespace, zoom)
    }
}
#endif
