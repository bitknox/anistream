// The Mac's main window: the sidebar, the toolbar with the sync capsule and search, and the
// section on screen.

#if os(macOS)
import AnistreamDesign
import AnistreamStore
import SwiftUI

struct MacShell: View {
    @Environment(AppStore.self) private var store
    @State private var section: AppSection = LaunchRoute.current.section
    @State private var query = ""

    var body: some View {
        NavigationSplitView {
            Sidebar(selection: $section)
                .navigationSplitViewColumnWidth(220)
                .toolbar(removing: .sidebarToggle)
        } detail: {
            SectionView(section: section)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(.ground)
                .navigationTitle(section.title)
                .toolbar {
                    ToolbarItem(placement: .primaryAction) {
                        SyncCapsule { section = .settings }
                    }
                }
                .searchable(text: $query, placement: .toolbar, prompt: "Search titles")
                .onSubmit(of: .search) {
                    store.search(query)
                    section = .search
                }
        }
        .containerBackground(Palette.ground, for: .window)
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
            selection = section
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
        case .continue:
            ContinueScreen()
        default:
            NotBuiltYet(section: section)
        }
    }
}
#endif
