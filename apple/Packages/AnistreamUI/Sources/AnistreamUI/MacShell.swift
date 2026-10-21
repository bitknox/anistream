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
                .navigationSplitViewColumnWidth(220)
                .toolbar(removing: .sidebarToggle)
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
            .navigationTitle(navigator.titleOnMac == nil ? navigator.section.title : "")
            .navigationSubtitle(subtitle)
            .toolbar {
                if let id = navigator.titleOnMac {
                    TitleToolbar(store: store, navigator: navigator, id: id)
                } else {
                    // What has run past 600 ms, quietly, beside the section's own items.
                    ToolbarItem(placement: .primaryAction) { ActivityIndicator() }
                    SectionToolbar(section: navigator.section)
                    ToolbarItem(placement: .primaryAction) {
                        SyncCapsule { navigator.open(.settings) }
                    }
                }
            }
            .toolbarBackgroundVisibility(navigator.titleOnMac == nil ? .automatic : .hidden, for: .windowToolbar)
            .searchable(text: $navigator.query, placement: .toolbar, prompt: "Search titles")
            .onSubmit(of: .search) { navigator.search(navigator.query, in: store) }
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
        default:
            ToolbarItem(placement: .primaryAction) { EmptyView() }
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
                .font(.system(size: 11, weight: .semibold))
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
        .focusEffectDisabled()
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
                    .font(.system(size: 14))
                    .foregroundStyle(selected ? .amber : .haze)
                    .frame(width: 16)
                Text(section.title)
                    .font(.system(size: 13, weight: selected ? .medium : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .frame(height: 30)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .animation(.quick, value: selected)
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
        default: NotBuiltYet(section: section)
        }
    }
}
#endif
