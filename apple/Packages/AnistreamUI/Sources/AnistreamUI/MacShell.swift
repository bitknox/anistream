// The Mac's main window: the sidebar, the toolbar each section dresses, the section on screen and
// a title's page over it, the palette and the notice capsule.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct MacShell: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Namespace private var bannerSpace

    var body: some View {
        @Bindable var navigator = navigator
        NavigationSplitView {
            Sidebar(selection: $navigator.section)
                // 220 wide, as the boards draw it. macOS 26 takes the column width; macOS 27
                // ignores it and opens the column at 144, where "Plugin Stores" is cut, or at
                // whatever it saved. A minimum on the column's content holds on both, and brings
                // a saved 144 back to 220.
                .frame(minWidth: 220)
                .navigationSplitViewColumnWidth(220)
                .toolbar(removing: .sidebarToggle)
                // The brief's navy glass. macOS draws its sidebar glass a slate of its own, grey
                // in an inactive window, and has no tint for it: `.navigationSplitView` container
                // backgrounds are iOS only, and a tint on the sidebar's NSGlassEffectView lifts it
                // to a light grey. So the column lays its tint over that glass, inside its shape.
                .background(.sidebarFill)
        } detail: {
            ZStack {
                SectionView(section: navigator.section)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .background(.ground)
                if let id = navigator.titleOnMac {
                    TitlePage(id: id)
                        .transition(.opacity)
                        .zIndex(1)
                }
            }
            .environment(\.bannerSpace, bannerSpace)
            // Downloads says its own, with the storage meter beside the line (MacDownloads).
            .navigationTitle(navigator.titleOnMac == nil && navigator.section != .downloads ? navigator.section.title : "")
            .navigationSubtitle(navigator.section == .downloads ? "" : subtitle)
            .toolbar {
                if let id = navigator.titleOnMac {
                    TitleToolbar(store: store, navigator: navigator, id: id)
                } else {
                    if navigator.section == .downloads {
                        // The title on the toolbar itself, as a title is, not in a glass capsule.
                        ToolbarItem(placement: .navigation) { DownloadsTitle() }
                            .sharedBackgroundVisibility(.hidden)
                    }
                    // What has run past 600 ms, quietly, beside the section's own items.
                    ToolbarItem(placement: .primaryAction) { ActivityIndicator() }
                    SectionToolbar(section: navigator.section)
                    if store.tv.connected != nil {
                        ToolbarItem(placement: .primaryAction) { TVNowPlayingButton() }
                    }
                    ToolbarItem(placement: .primaryAction) {
                        SyncCapsule { navigator.openSettings(.accounts) }
                    }
                }
            }
            .toolbarBackgroundVisibility(navigator.titleOnMac == nil ? .automatic : .hidden, for: .windowToolbar)
            .titleSearchField(
                text: navigator.section == .settings ? $navigator.settingsQuery : $navigator.query,
                titles: navigator.section != .settings,
                placement: .toolbar,
                prompt: navigator.section == .settings ? "Search Settings" : "Search titles"
            )
        }
        .containerBackground(Palette.ground, for: .window)
        .overlay {
            if navigator.paletteShown {
                PaletteOverlay()
            }
        }
        .overlay(alignment: .bottom) {
            NoticeCapsule().padding(.bottom, 20)
        }
        .sheet(isPresented: $navigator.messagesShown) {
            MessagesView()
        }
        .modifier(AppSheets())
        .sheet(item: Binding<ShowItem?>(
            get: { navigator.showSettings.flatMap { store.titles[$0] }.map(ShowItem.init) },
            set: { (item: ShowItem?) in navigator.showSettings = item?.entry.id }
        )) { item in
            ShowSettingsSheet(entry: item.entry)
        }
        .moreAboutSheet(navigator)
    }

    /// What the toolbar says under the section's name.
    private var subtitle: String {
        guard navigator.titleOnMac == nil else { return "" }
        switch navigator.section {
        case .continue:
            return store.continuing.list.state == .empty ? "Nothing watched yet" : ""
        case .calendar:
            return CalendarWords.range(weekDays(offset: navigator.calendarWeek))
        case .seasonal:
            return store.seasonName ?? ""
        case .search:
            // The search as a job past 600 ms, then what is asked (Running jobs, native-ux.md).
            return store.search.searchingLine ?? (store.search.atStart ? "" : store.search.query.headline())
        case .downloads:
            return downloadsSubtitle(store)
        default:
            return ""
        }
    }
}

/// The toolbar items a section adds beside the sync capsule and search.
struct SectionToolbar: ToolbarContent {
    let section: AppSection

    var body: some ToolbarContent {
        switch section {
        case .calendar:
            ToolbarItem(placement: .primaryAction) { CalendarToolbar() }
        case .library:
            ToolbarItem(placement: .primaryAction) {
                HStack(spacing: 8) {
                    LibrarySegments()
                    LibraryMenus()
                }
            }
        case .seasonal:
            ToolbarItem(placement: .primaryAction) { SeasonalToolbar() }
        case .providers:
            ToolbarItem(placement: .primaryAction) { ProvidersToolbar() }
        case .pluginStores:
            ToolbarItem(placement: .primaryAction) { StoresToolbar() }
        case .downloads:
            ToolbarItem(placement: .primaryAction) { DownloadsToolbar() }
        default:
            ToolbarItem(placement: .primaryAction) { EmptyView() }
        }
    }
}

/// Providers' toolbar: check the stores for plugin updates.
struct ProvidersToolbar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Button("Check for Updates") { store.loadStores() }
    }
}

/// Plugin Stores' toolbar: join another.
struct StoresToolbar: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.invite = ""
        } label: {
            Label("Join Store…", systemImage: "plus").labelStyle(.titleAndIcon)
        }
    }
}

/// Downloads' toolbar: pause or resume everything, and the folder.
struct DownloadsToolbar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let running = store.downloads.rows.contains { $0.state == .active || $0.state == .queued }
        let paused = store.downloads.rows.contains { $0.state == .paused }
        HStack(spacing: 8) {
            if running || !paused {
                Button("Pause All") { store.pauseAllDownloads() }
                    .disabled(!running)
            } else {
                Button("Resume All") { store.resumeAllDownloads() }
            }
            Button {
                let path = store.settings.row(.downloadDir)?.typedText ?? ""
                if !path.isEmpty { NSWorkspace.shared.open(URL(fileURLWithPath: path)) }
            } label: {
                Label("Show in Finder", systemImage: "folder").labelStyle(.titleAndIcon)
            }
            .disabled((store.settings.row(.downloadDir)?.typedText ?? "").isEmpty)
        }
    }
}

/// The Library's lists, with how many titles each holds.
struct LibrarySegments: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        if store.accounts.summary.state != .signedOut {
            Picker("List", selection: $navigator.librarySegment) {
                ForEach(LibraryModel.order, id: \.self) { segment in
                    let count = store.library.segment(segment).state.ids.count
                    Text("\(segment.label.capitalizedSentence) \(count)").tag(segment)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
        }
    }
}

/// The Library's order, and More: Sync Now and Export.
struct LibraryMenus: View {
    @Environment(AppStore.self) private var store
    @AppStorage("anistream.librarySort") private var sort: LibrarySort = .lastWatched

    var body: some View {
        if store.accounts.summary.state != .signedOut {
            HStack(spacing: 8) {
                Menu {
                    Picker("Order", selection: $sort) {
                        ForEach(LibrarySort.allCases) { Text($0.rawValue).tag($0) }
                    }
                    .pickerStyle(.inline)
                } label: {
                    Text(sort.rawValue)
                }
                .fixedSize()
                Menu {
                    Button("Sync Now") { store.send(.syncNow) }
                    Divider()
                    Button("Export…") {}
                        .disabled(true)
                } label: {
                    Label("More", systemImage: "ellipsis")
                }
            }
        }
    }
}

/// The sidebar: the sections, then Sources, then Settings alone at the foot. Its rows draw
/// their own selection, the amber tint under an amber label and icon, which the brief asks of
/// every selection and which a system list draws grey whenever the window is not key.
struct Sidebar: View {
    @Binding var selection: AppSection
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(AppSection.browse) { row($0) }
            Text("Sources")
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 18, leading: 10, bottom: 6, trailing: 10))
                .accessibilityAddTraits(.isHeader)
            ForEach(AppSection.sources) { row($0) }
            Spacer(minLength: 0)
            row(.settings)
        }
        .padding(EdgeInsets(top: 0, leading: 10, bottom: 12, trailing: 10))
        .focusable()
        .focused($focused)
        .listFocusEffect()
        .onKeyPress(.upArrow) { step(-1) }
        .onKeyPress(.downArrow) { step(1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Sections")
    }

    private func row(_ section: AppSection) -> some View {
        let selected = selection == section
        return Button {
            // M1: a section changes at once, with no animation.
            var instant = Transaction()
            instant.disablesAnimations = true
            withTransaction(instant) { selection = section }
        } label: {
            HStack(spacing: 10) {
                Image(systemName: section.symbol)
                    .font(.mac(14))
                    .foregroundStyle(selected ? .amber : .haze)
                    .frame(width: 16)
                Text(section.title)
                    .font(.mac(13, weight: selected ? .medium : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .frame(height: 30)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        // Nothing moves on a section change (M1), the tint included.
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private func step(_ delta: Int) -> KeyPress.Result {
        let order = AppSection.browse + AppSection.sources + [.settings]
        let index = order.firstIndex(of: selection) ?? 0
        selection = order[min(max(index + delta, 0), order.count - 1)]
        return .handled
    }
}

/// The section on screen.
struct SectionView: View {
    let section: AppSection

    var body: some View {
        switch section {
        case .continue: ContinueScreen()
        case .calendar: CalendarScreen()
        case .seasonal: SeasonalScreen()
        case .search: SearchScreen()
        case .library: LibraryScreen()
        case .settings: SettingsScreen()
        case .downloads: DownloadsScreen()
        case .providers: ProvidersScreen()
        case .pluginStores: PluginStoresScreen()
        }
    }
}
#endif
