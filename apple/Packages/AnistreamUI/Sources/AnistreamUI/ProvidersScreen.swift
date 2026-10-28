// Providers on the Mac and a wide iPad: the sources in the order failover tries them, moved by
// dragging or ⌥⌘↑ and ⌥⌘↓, and the selected one's version, speed and what it may contact.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What the Providers screen says of one source, from its row and the stores it came from.
struct ProviderInfo {
    let row: ProviderRow
    let name: String
    let origin: String
    /// The store that keeps it updated, and its listing there.
    let listing: (store: StoreRow, plugin: StorePluginRow)?

    var isTorrent: Bool { row.kind == "torrent" }
    var isPlugin: Bool { row.kind == "plugin" }

    /// A newer version its store offers.
    var update: String? {
        if case .update = listing?.plugin.status { return listing?.plugin.version }
        return nil
    }

    var hosts: [String] { listing?.plugin.hosts ?? [] }
}

extension AppStore {
    /// What Providers says of `row`: its name, where it came from and its store's listing.
    func info(_ row: ProviderRow) -> ProviderInfo {
        let listing = stores.origin(of: row.id)
        let name: String
        let origin: String
        switch row.kind {
        case "torrent":
            (name, origin) = (L.sourcesTorrents, L.sourcesOriginBuiltIn)
        case "remote":
            (name, origin) = (L.sourcesRemote, L.sourcesOriginConfig)
        default:
            name = listing?.row.name ?? row.id
            if case .addedByHand = listing?.row.status {
                origin = L.sourcesOriginByHand
            } else {
                origin = listing.map { $0.store.name } ?? L.sourcesOriginByHand
            }
        }
        return ProviderInfo(row: row, name: name, origin: origin, listing: listing.map { ($0.store, $0.row) })
    }
}

extension AppStore {
    /// Whether the engine has said there is nothing to watch from: no plugin, and no torrent
    /// source that can play.
    var hasNoSources: Bool {
        providers.loaded && providers.rows.allSatisfy { $0.kind == "torrent" && !$0.usable }
    }
}

/// Providers with no source: Nagare, what a source is and where one comes from, and the one way
/// to get one (the spec's empty states).
struct NoSources: View {
    @Environment(Navigator.self) private var navigator
    /// Whether the legal line closes it, where no footer under the list says it.
    var legal = true

    var body: some View {
        EmptyState(
            title: L.sourcesEmptyTitle,
            line: L.sourcesEmptyDetail + (legal ? " " + L.legalHostsNoContent : "")
        ) {
            Button(L.storesJoinAction) { navigator.invite = "" }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }
}

struct ProvidersScreen: View {
    @Environment(AppStore.self) private var store
    @State private var selection: String?

    var body: some View {
        let rows = store.orderedProviders
        if store.hasNoSources {
            NoSources()
                .task {
                    store.checkProviders()
                    if !store.stores.loaded { store.loadStores() }
                }
        } else {
            list(rows)
        }
    }

    private func list(_ rows: [ProviderRow]) -> some View {
        HStack(alignment: .top, spacing: 0) {
            SourceOrderList(rows: rows, selection: $selection)
                .frame(width: 460)
            if let row = rows.first(where: { $0.id == selection }) ?? rows.first {
                ProviderDetail(info: store.info(row))
                    .id(row.id)
            } else if let note = store.providers.note {
                Text(note)
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .padding(24)
            }
        }
        .padding(.leading, 6)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
        }
    }
}

/// The sources in failover order: drag one, or ⌥⌘↑ and ⌥⌘↓ on the selected one, to move it.
struct SourceOrderList: View {
    @Environment(AppStore.self) private var store
    let rows: [ProviderRow]
    @Binding var selection: String?
    @FocusState private var focused: Bool

    var body: some View {
        let current = selection ?? rows.first?.id
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(L.sourcesOrder)
                    .groupHeaderStyle()
                    .accessibilityAddTraits(.isHeader)
                    .macLineHeight(14)
                    .padding(EdgeInsets(top: 10, leading: 18, bottom: 8, trailing: 0))
                if store.providers.rebuilding {
                    Text(L.sourcesRebuilding)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                        .padding(.horizontal, 18)
                        .padding(.bottom, 6)
                }
                ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                    SourceLine(info: store.info(row), selected: row.id == current)
                        .onTapGesture { selection = row.id }
                        .accessibilityAction(named: L.sourcesMoveUp) { store.moveSource(row.id, by: -1) }
                        .accessibilityAction(named: L.sourcesMoveDown) { store.moveSource(row.id, by: 1) }
                        .draggable(row.id)
                        .dropDestination(for: String.self) { ids, _ in
                            guard let id = ids.first, id != row.id else { return false }
                            store.moveSource(id, to: index)
                            selection = id
                            return true
                        }
                }
                VStack(alignment: .leading, spacing: 8) {
                    Text(L.sourcesOrderDetail)
                    Text(L.legalHostsNoContent)
                }
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(EdgeInsets(top: 14, leading: 18, bottom: 0, trailing: 12))
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
        .focusable()
        .focused($focused)
        .listFocusEffect()
        .onKeyPress(keys: [.upArrow, .downArrow], phases: .down) { press in
            let up = press.key == .upArrow
            guard let id = current else { return .ignored }
            if press.modifiers.contains(.option), press.modifiers.contains(.command) {
                store.moveSource(id, by: up ? -1 : 1)
                return .handled
            }
            guard let at = rows.firstIndex(where: { $0.id == id }) else { return .ignored }
            selection = rows[min(max(at + (up ? -1 : 1), 0), rows.count - 1)].id
            return .handled
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.sourcesOrder)
    }
}

/// One source in the order: its handle, name, where it came from, and how it stands.
struct SourceLine: View {
    let info: ProviderInfo
    let selected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "line.3.horizontal")
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
                .frame(width: 12)
            VStack(alignment: .leading, spacing: 1) {
                Text(info.name)
                    .font(.mac(13, weight: .medium))
                    .macLineHeight(17)
                    .foregroundStyle(selected ? .amber : .silk)
                Text(info.origin)
                    .font(.mac(12))
                    .macLineHeight(15)
                    .foregroundStyle(.haze)
            }
            Spacer(minLength: 0)
            ProviderStandingLabel(info: info, selected: selected)
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// How a source stands, beside its name: failing, held back, slow, or with an update waiting.
struct ProviderStandingLabel: View {
    let info: ProviderInfo
    var selected = false
    var size: CGFloat = 12

    var body: some View {
        switch info.row.standing {
        case .failing:
            // Silk words beside the vermilion dot, as the boards have it: the dot says what is
            // wrong, and red text would only shout it.
            label(L.sourcesStatusNotAnswering, dot: .vermilion, color: .silk)
        case .heldBack:
            label(L.sourcesStatusHeldBack, ring: true, color: .haze)
        case .off:
            label(L.valueOff, ring: true, color: .silk)
        case .slow:
            label(L.sourcesStatusSlow, dot: .haze, color: .haze)
        case .working:
            if info.update != nil {
                label(L.sourcesStatusUpdate, dot: .water, color: .silk)
            }
        }
    }

    private func label(_ text: String, dot: Color? = nil, ring: Bool = false, color: Color) -> some View {
        HStack(spacing: 6) {
            if ring {
                Circle().strokeBorder(.haze, lineWidth: 1.5).frame(width: 6, height: 6)
            } else if let dot {
                Circle().fill(dot).frame(width: 6, height: 6)
            }
            Text(text).font(.mac(size)).foregroundStyle(color)
        }
    }
}

/// The selected source: what it is, what can be done with it, and what it may contact.
struct ProviderDetail: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let info: ProviderInfo
    @State private var removing = false
    @State private var updating: StorePluginRow?
    @State private var turningOn = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(info.name)
                    .font(.mac(22, weight: .bold))
                    .macLineHeight(28)
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Text(info.isPlugin && info.origin != L.sourcesOriginByHand ? L.sourcesOriginFrom(name: info.origin) : info.origin)
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .padding(.top, 2)
                if let line = stateLine {
                    HStack(spacing: 8) {
                        Circle().fill(info.row.standing == .failing ? .vermilion : .haze).frame(width: 6, height: 6)
                        Text(line)
                            .font(.mac(13))
                            .macLineHeight(18)
                            .foregroundStyle(.haze)
                    }
                    .padding(.top, 14)
                }
                if info.isTorrent { torrentActions } else { pluginActions }
                if !info.isTorrent { facts }
            }
            .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
        }
        .scrollIndicators(.never)
        .confirmationDialog(L.sourcesRemoveTitle(name: info.name), isPresented: $removing) {
            Button(L.actionRemove, role: .destructive) { store.removePlugin(info.row.id) }
        } message: {
            Text(L.sourcesRemoveDetail)
        }
        .sheet(item: $updating) { plugin in
            if let listing = info.listing { UpdatePluginSheet(store: listing.store, plugin: plugin) }
        }
        .sheet(isPresented: $turningOn) { TorrentSheet() }
    }

    private var stateLine: String? {
        switch info.row.standing {
        // The site's own error is in Messages and the bug report, not here (copy rule 5).
        case .failing: L.sourcesDetailNotAnsweringPlain
        case .heldBack: L.sourcesDetailHeldBack
        default: nil
        }
    }

    private var torrentActions: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                if store.settings.isOn(.torrents) {
                    Button(L.actionDisable) { store.setSwitch(.torrents, on: false) }
                } else {
                    Button(L.actionEnable) { turningOn = true }
                }
                Button(L.sourcesOpenSettings) { navigator.openSettings(.sources) }
            }
            .buttonStyle(SecondaryCapsuleStyle())
            Text(L.sourcesTorrentsNote)
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, 16)
    }

    private var pluginActions: some View {
        HStack(spacing: 8) {
            if let version = info.update, let plugin = info.listing?.plugin {
                Button(L.sourcesUpdateTo(version: version)) { updating = plugin }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
            }
            if info.row.standing == .failing {
                let checking = store.jobs.running.contains { $0.activity == "checking providers" }
                Button(L.actionCheckAgain) { store.checkProviders() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .disabled(checking)
                Working(checking)
            }
            // Off without removing it: its source leaves the chain until it is on again.
            let disabled = store.isPluginDisabled(info.row.id)
            Button(disabled ? L.actionEnable : L.actionDisable) { store.setPluginDisabled(info.row.id, !disabled) }
                .buttonStyle(SecondaryCapsuleStyle())
            Button(L.actionRemove) { removing = true }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(store.stores.isWorking(.remove(plugin: info.row.id)))
        }
        .padding(.top, 16)
    }

    private var facts: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(spacing: 0) {
                fact(L.sourcesVersion) {
                    if let listing = info.listing {
                        Text(listing.plugin.installedVersion).foregroundStyle(.silk)
                        if let update = info.update { Text(L.sourcesUpdateAvailableVersion(version: update)).foregroundStyle(.haze) }
                    } else {
                        Text(L.sourcesOriginNoStore).foregroundStyle(.haze)
                    }
                }
                Rectangle().fill(.faintRule).frame(height: 1)
                fact(L.sourcesResponseTime) {
                    if info.row.standing == .failing {
                        Text(L.sourcesNoAnswer).foregroundStyle(.silk)
                    } else if let latency = info.row.latencyMs {
                        Text(L.sourcesResponseMs(ms: Int(latency))).foregroundStyle(.silk).monospacedDigit()
                    } else {
                        Text(L.sourcesNotChecked).foregroundStyle(.haze)
                    }
                }
                Rectangle().fill(.faintRule).frame(height: 1)
                fact(L.sourcesContactsLabel) {
                    VStack(alignment: .leading, spacing: 6) {
                        if info.hosts.isEmpty {
                            Text(L.sourcesListedByStore).foregroundStyle(.haze)
                        }
                        ForEach(info.hosts, id: \.self) { Text($0).foregroundStyle(.silk) }
                    }
                }
            }
            .padding(.horizontal, 14)
            .background(.raised, in: .rect(cornerRadius: 12))
            Text("\(info.name) can contact only these hosts and cannot read your files.")
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 8, leading: 14, bottom: 0, trailing: 0))
        }
        .padding(.top, 12)
    }

    private func fact(_ label: String, @ViewBuilder value: () -> some View) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            Text(label).foregroundStyle(.haze).frame(width: 120, alignment: .leading)
            HStack(alignment: .firstTextBaseline, spacing: 8) { value() }
            Spacer(minLength: 0)
        }
        .font(.mac(13))
        .padding(.vertical, 12)
    }
}

extension StorePluginRow: @retroactive Identifiable {}
