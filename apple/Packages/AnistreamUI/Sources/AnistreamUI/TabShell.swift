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
        // The iPad's sidebar as the brief draws it (PadSidebarStyle). The bar is empty; it is
        // where the style reaches the sidebar from, and unlike a header it moves no row.
        .tabViewSidebarBottomBar { PadSidebarStyle(selection: navigator.section) }
        .overlay(alignment: .top) {
            NoticeCapsule(here: navigator.phoneSettings == nil).padding(.top, 8)
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
        // The list's menu is in the page's header, beside Resume (PadTitleMeta): a narrow
        // window's bar, with the tabs in it, folded it away.
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
    @Environment(AppStore.self) private var store
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
                .toolbar {
                    if store.tv.connected != nil {
                        ToolbarItem(placement: .topBarLeading) { TVNowPlayingButton() }
                    }
                }
                .navigationTitle(section.title.capitalizedSentence)
                .boardTitle(section.title.capitalizedSentence)
                .background(.ground)
                // The ground under the whole stack, edge to edge, however narrow the screen in
                // it: never the window's black at its sides.
                .containerBackground(.ground, for: .navigation)
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

/// The iPad's sidebar as the brief draws it: the brief's navy glass, and its rows with the
/// selection tint under an amber label and icon, every other row silk with a haze icon, and the
/// Sources header in haze. UIKit draws the sidebar itself, grey glass with a solid capsule in the
/// tint under white text, and gives no style for either; this view, in the sidebar's bottom bar,
/// reaches both from there. It draws nothing, and takes no room.
///
/// - The glass: it finds the glass around it and tints it, whenever the sidebar is put on screen
///   or laid out again.
/// - The rows: it styles each row as UIKit asks the sidebar's delegate for it (`SidebarRows`,
///   which hands everything else on to SwiftUI's own delegate), and has them asked again when the
///   section changes, since UIKit moves the selection without asking.
/// - A pressed row: UIKit tells the delegate nothing of a press, so `RowPress` watches the list's
///   touches and lays a faint layer under the row while the finger is down.
struct PadSidebarStyle: UIViewRepresentable {
    let selection: AppSection

    final class Hook: UIView {
        private static let navy = UIColor(Palette.glassOverGround)

        override func didMoveToWindow() {
            super.didMoveToWindow()
            apply()
        }

        override func layoutSubviews() {
            super.layoutSubviews()
            apply()
        }

        private func apply() {
            guard window != nil else { return }
            if let glass {
                tint(glass)
                if let list = list(in: glass.contentView) { RowPress.watch(list) }
            }
            guard let controller = tabBarController else { return }
            if SidebarRows.install(on: controller) { restyle() }
        }

        /// The glass the sidebar sits in.
        private var glass: UIVisualEffectView? {
            var view = superview
            while let current = view {
                if let glass = current as? UIVisualEffectView, glass.effect is UIGlassEffect { return glass }
                view = current.superview
            }
            return nil
        }

        private func tint(_ glass: UIVisualEffectView) {
            guard let effect = glass.effect as? UIGlassEffect, effect.tintColor != Self.navy,
                  let tinted = effect.copy() as? UIGlassEffect else { return }
            tinted.tintColor = Self.navy
            glass.effect = tinted
        }

        /// The sidebar's list of rows.
        private func list(in view: UIView) -> UICollectionView? {
            if let list = view as? UICollectionView { return list }
            for subview in view.subviews {
                if let list = list(in: subview) { return list }
            }
            return nil
        }

        private var tabBarController: UITabBarController? {
            var responder: UIResponder? = self
            while let current = responder {
                if let controller = current as? UITabBarController { return controller }
                responder = current.next
            }
            return nil
        }

        /// Has UIKit ask for every row again, so each is styled as it now stands.
        func restyle() {
            guard let controller = tabBarController else { return }
            func all(_ tabs: [UITab]) -> [UITab] {
                tabs.flatMap { [$0] + (($0 as? UITabGroup).map { all($0.children) } ?? []) }
            }
            for tab in all(controller.tabs) { controller.sidebar.reconfigureItem(for: tab) }
        }
    }

    func makeUIView(context: Context) -> Hook { Hook() }

    func updateUIView(_ uiView: Hook, context: Context) {
        // After this update, once the tab bar controller has the section selected.
        Task { @MainActor in uiView.restyle() }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: Hook, context: Context) -> CGSize? {
        .zero
    }
}

/// The sidebar's delegate: styles each row UIKit asks for, and hands every other call on to the
/// delegate it replaced, SwiftUI's own.
@MainActor
final class SidebarRows: NSObject, UITabBarController.Sidebar.Delegate {
    private static var key: UInt8 = 0
    /// SwiftUI's delegate, which SwiftUI keeps.
    nonisolated(unsafe) private weak var original: (any UITabBarController.Sidebar.Delegate)?

    /// Puts the styling delegate on `controller`'s sidebar, kept with the controller, in front of
    /// whatever delegate is there. True when it was not there already.
    static func install(on controller: UITabBarController) -> Bool {
        let rows: SidebarRows
        if let kept = objc_getAssociatedObject(controller, &key) as? SidebarRows {
            rows = kept
        } else {
            rows = SidebarRows()
            objc_setAssociatedObject(controller, &key, rows, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
        let sidebar = controller.sidebar
        guard sidebar.delegate !== rows else { return false }
        rows.original = sidebar.delegate
        sidebar.delegate = rows
        return true
    }

    override nonisolated func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override nonisolated func forwardingTarget(for selector: Selector!) -> Any? {
        if let original, original.responds(to: selector) { return original }
        return super.forwardingTarget(for: selector)
    }

    func tabBarController(
        _ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar,
        itemFor request: UITabSidebarItem.Request
    ) -> UITabSidebarItem {
        let item = original?.tabBarController?(tabBarController, sidebar: sidebar, itemFor: request)
            ?? UITabSidebarItem(request: request)
        style(item, in: tabBarController)
        return item
    }

    func tabBarController(
        _ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar,
        update item: UITabSidebarItem
    ) {
        original?.tabBarController?(tabBarController, sidebar: sidebar, update: item)
        style(item, in: tabBarController)
    }

    private func style(_ item: UITabSidebarItem, in controller: UITabBarController) {
        guard case .tab(let tab) = item.content,
              var content = item.contentConfiguration as? UIListContentConfiguration else { return }
        content.textProperties.colorTransformer = nil
        if tab is UITabGroup {
            // A section's header, Sources.
            content.textProperties.color = UIColor(Palette.haze)
            item.contentConfiguration = content
            return
        }
        // The controller's selected tab, which is current before the row's own state is.
        let selected = controller.selectedTab.map { $0.identifier == tab.identifier }
            ?? item.configurationState.isSelected
        content.textProperties.color = UIColor(selected ? Palette.amber : Palette.silk)
        content.imageProperties.tintColorTransformer = nil
        content.imageProperties.tintColor = UIColor(selected ? Palette.amber : Palette.haze)
        if selected {
            let font = content.textProperties.font
            let medium = font.fontDescriptor.addingAttributes([.traits: [UIFontDescriptor.TraitKey.weight: UIFont.Weight.medium]])
            content.textProperties.font = UIFont(descriptor: medium, size: 0)
        }
        item.contentConfiguration = content
        var background = item.defaultBackgroundConfiguration()
        background.backgroundColorTransformer = nil
        background.backgroundColor = selected ? UIColor(Palette.selection) : .clear
        background.cornerRadius = 12
        item.backgroundConfiguration = background
    }
}

/// A finger on one of the sidebar's rows. UIKit tells neither the sidebar's delegate nor the rows'
/// configurations that a row is pressed, and its own press was drawn on the colours `SidebarRows`
/// replaces, so this watches the list's touches, alongside the list's own gestures, and lays a
/// layer under the pressed row: a faint silk, or under the selected row enough amber that the two
/// make `Palette.selectionPressed`. It goes when the finger lifts or moves away.
final class RowPress: UILongPressGestureRecognizer, UIGestureRecognizerDelegate {
    private static let underSelection: UIColor = {
        let environment = EnvironmentValues()
        let shown = Double(Palette.selection.resolve(in: environment).opacity)
        let pressed = Double(Palette.selectionPressed.resolve(in: environment).opacity)
        return UIColor(Palette.amber.opacity(1 - (1 - pressed) / (1 - shown)))
    }()

    private var pressLayer: UIView?
    private var start: CGPoint = .zero

    /// Watches `list`'s rows, once.
    static func watch(_ list: UICollectionView) {
        guard !(list.gestureRecognizers ?? []).contains(where: { $0 is RowPress }) else { return }
        list.addGestureRecognizer(RowPress())
    }

    private init() {
        super.init(target: nil, action: nil)
        addTarget(self, action: #selector(changed))
        minimumPressDuration = 0
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
        delegate = self
    }

    @objc private func changed() {
        guard let list = view as? UICollectionView else { return }
        switch state {
        case .began:
            start = location(in: list)
            guard let path = list.indexPathForItem(at: start), let row = list.cellForItem(at: path) else { return }
            let under = UIView(frame: row.bounds)
            under.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            under.isUserInteractionEnabled = false
            under.layer.cornerRadius = 12
            under.layer.cornerCurve = .continuous
            under.backgroundColor = row.isSelected ? Self.underSelection : UIColor(Palette.rowPressed)
            row.insertSubview(under, belowSubview: row.contentView)
            pressLayer = under
        case .changed:
            let point = location(in: list)
            if hypot(point.x - start.x, point.y - start.y) > 10 { lift() }
        default:
            lift()
        }
    }

    private func lift() {
        pressLayer?.removeFromSuperview()
        pressLayer = nil
    }

    func gestureRecognizer(
        _ gestureRecognizer: UIGestureRecognizer,
        shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
    ) -> Bool {
        true
    }
}
#endif
