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
                            TabStack(section: section) { NotBuiltYet(section: section) }
                        }
                        .defaultVisibility(.hidden, for: .tabBar)
                    }
                }
                Tab(AppSection.settings.title, systemImage: AppSection.settings.symbol, value: AppSection.settings) {
                    TabStack(section: .settings) { NotBuiltYet(section: .settings) }
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
    }

    @ViewBuilder
    private func screen(_ section: AppSection) -> some View {
        let wide = sizeClass == .regular
        switch section {
        case .continue: if wide { ContinueScreen() } else { PhoneContinue() }
        case .calendar: if wide { CalendarScreen() } else { PhoneCalendar() }
        case .seasonal: SeasonalScreen(minimum: wide ? 128 : 100)
        case .library: PhoneLibrary()
        default: NotBuiltYet(section: section)
        }
    }
}

/// A tab's stack: its screen, with a title's page pushed onto it by a zoom from the cover that
/// opened it.
struct TabStack<Screen: View>: View {
    @Environment(Navigator.self) private var navigator
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
                    PhoneTitle(id: id)
                        .navigationTransition(.zoom(sourceID: id, in: zoom))
                }
        }
        .environment(\.zoomNamespace, zoom)
    }
}
#endif
