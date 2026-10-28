// Providers and plugin stores on iPhone, as pages of the Settings sheet, and the sheets for
// joining a store and turning torrents on.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

// MARK: - Providers

extension View {
    /// "Update Mukashi Archive?" before a plugin's update installs: the version it replaces and
    /// the hosts it adds, as the PhoneStore board asks (F19).
    func pluginUpdateAlert(isPresented: Binding<Bool>, store: StoreRow, plugin: StorePluginRow) -> some View {
        modifier(PluginUpdateAlert(isPresented: isPresented, store: store, plugin: plugin))
    }
}

private struct PluginUpdateAlert: ViewModifier {
    @Environment(AppStore.self) private var appStore
    @Binding var isPresented: Bool
    let store: StoreRow
    let plugin: StorePluginRow

    func body(content: Content) -> some View {
        content.alert(L.sourcesUpdateTitle(name: plugin.name), isPresented: $isPresented) {
            Button(L.actionUpdate) { appStore.installPlugin(plugin.id, from: store.id) }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(message)
        }
    }

    private var message: String {
        guard case .update(let from, let added) = plugin.status else { return "" }
        let hosts = added.formatted(.list(type: .and).locale(AirTime.locale))
        let replaces = L.sourcesUpdateDetail(version: plugin.version, store: store.name, from: from)
        return added.isEmpty ? replaces : replaces + " " + L.storesUpdateContacts(hosts: hosts)
    }
}

/// The sources in failover order, each with its switch; Edit to drag them into another order.
struct PhoneProvidersPage: View {
    @Environment(AppStore.self) private var store
    @State private var turningOn = false

    var body: some View {
        let rows = store.orderedProviders
        PhoneList {
            if store.hasNoSources {
                NoSources(legal: false)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
            }
            Section {
                ForEach(rows, id: \.id) { row in
                    PhoneProviderRow(info: store.info(row), turningOn: $turningOn)
                }
                .onMove { from, to in
                    guard let index = from.first else { return }
                    // `to` counts the row's old place; a move down lands one before it.
                    store.moveSource(rows[index].id, to: to > index ? to - 1 : to)
                }
            } footer: {
                VStack(alignment: .leading, spacing: 8) {
                    Text(L.sourcesOrderDetailEdit)
                    Text(L.legalHostsNoContent)
                }
            }
        }
        .navigationTitle(L.sourcesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { EditButton() }
        .sheet(isPresented: $turningOn) { PhoneTorrentSheet() }
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
        }
    }
}

/// One source: its name, where it came from and how it stands, and its switch.
struct PhoneProviderRow: View {
    @Environment(AppStore.self) private var store
    let info: ProviderInfo
    @Binding var turningOn: Bool

    var body: some View {
        HStack {
            if info.isTorrent {
                label
            } else {
                NavigationLink(value: PhoneSettingsPage.plugin(info.row.id)) { label }
            }
            if info.isTorrent {
                Spacer()
                Toggle(L.sourcesTorrents, isOn: Binding(
                    get: { store.settings.isOn(.torrents) },
                    set: { wanted in if wanted { turningOn = true } else { store.setSwitch(.torrents, on: false) } }
                ))
                .labelsHidden()
                .tint(.amber)
            } else {
                // Off without removing it: its source leaves the chain until it is on again.
                Toggle(info.name, isOn: Binding(
                    get: { !store.isPluginDisabled(info.row.id) },
                    set: { store.setPluginDisabled(info.row.id, !$0) }
                ))
                .labelsHidden()
                .tint(.amber)
            }
        }
        .listRowBackground(Palette.raised)
    }

    private var label: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(info.name).font(.ui(17)).foregroundStyle(.silk)
            HStack(spacing: 6) {
                Text(info.origin).foregroundStyle(.haze)
                ProviderStandingLabel(info: info, size: 15)
            }
            .font(.ui(15))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Plugin stores

struct PhoneStoresPage: View {
    @Environment(AppStore.self) private var store
    @State private var joining = false

    var body: some View {
        PhoneList {
            Section {
                ForEach(store.stores.stores, id: \.id) { row in
                    NavigationLink(value: PhoneSettingsPage.store(row.id)) {
                        HStack {
                            Text(row.name).font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            StoreStanding(row: row, size: 17)
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
            } footer: {
                Text(L.storesEmptyDetail)
            }
        }
        .navigationTitle(L.navPluginStores)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { Button(L.storesJoinButton) { joining = true } }
        }
        .sheet(isPresented: $joining) { PhoneJoinStoreSheet(link: "") }
        .task { store.loadStores() }
    }
}

/// One store: its plugins with their actions, another device, leaving, and the key it was joined by.
struct PhoneStorePage: View {
    @Environment(AppStore.self) private var appStore
    let id: String
    @State private var leaving = false
    @State private var inviting = false

    var body: some View {
        if let row = appStore.stores.store(id) {
            PhoneList {
                Section {
                    ForEach(row.plugins, id: \.id) { plugin in
                        NavigationLink(value: PhoneSettingsPage.plugin(plugin.id)) {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(plugin.name).font(.ui(17)).foregroundStyle(.silk)
                                    Text(plugin.contactsLine).font(.ui(15)).foregroundStyle(.haze)
                                }
                                Spacer()
                                PhonePluginAction(store: row, plugin: plugin)
                            }
                        }
                        .listRowBackground(Palette.raised)
                    }
                } header: {
                    PhoneSectionHeader(L.storesPlugins)
                } footer: {
                    Text(L.storesPluginLimits)
                }
                Section {
                    // A device that joined with the store's link joins another the same way.
                    if row.state != .revoked, !row.anonymous {
                        Button(L.storesLinkDeviceTitle) { inviting = true }
                            .fontWeight(.semibold)
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button(L.storesLeave, role: .destructive) { leaving = true }
                        .fontWeight(.semibold)
                        .listRowBackground(Palette.raised)
                } footer: {
                    // Who can join is changed on the Mac, an iPad or the store's admin page.
                    let about = [StoreWords.meta(row), row.open ? L.storesAnyoneShort : nil]
                    let key = [L.storesKeyLine(key: row.fingerprint, joined: row.joinedAs), StoreWords.checked(row)]
                    Text(about.compactMap { $0 }.joined(separator: " · ") + "\n"
                         + key.compactMap { $0 }.joined(separator: " · "))
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .confirmationDialog(L.storesLeaveTitle(name: row.name), isPresented: $leaving, titleVisibility: .visible) {
                Button(L.storesLeave, role: .destructive) { appStore.leaveStore(row.id) }
            } message: {
                Text(L.storesLeaveDetail)
            }
            .sheet(isPresented: $inviting) { InviteSheet(store: row, device: true) }
        } else {
            Text(L.storesNoLongerJoined).foregroundStyle(.haze)
        }
    }
}

/// A plugin's action in a store's list: installed, an update, or install.
struct PhonePluginAction: View {
    @Environment(AppStore.self) private var appStore
    let store: StoreRow
    let plugin: StorePluginRow
    /// Whether the update is asking first, with the hosts it adds.
    @State private var confirming = false

    var body: some View {
        let working = appStore.stores.isWorking(.install(store: store.id, plugin: plugin.id))
        switch plugin.status {
        case .installed:
            HStack(spacing: 6) {
                Image(systemName: "checkmark").foregroundStyle(.water)
                Text(L.storesPluginStateInstalled).foregroundStyle(.haze)
            }
            .font(.ui(15))
        case .update where store.state != .revoked:
            Button(L.actionUpdate) { confirming = true }
                .buttonStyle(PrimaryCapsuleStyle(height: 30, fontSize: 15))
                .disabled(working)
                .pluginUpdateAlert(isPresented: $confirming, store: store, plugin: plugin)
        case .available where store.state == .ready:
            Button(L.actionInstall) { appStore.installPlugin(plugin.id, from: store.id) }
                .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                .disabled(working)
        case .otherStore(let other):
            Text(L.storesPluginFrom(other: other)).font(.ui(15)).foregroundStyle(.haze)
        default:
            EmptyView()
        }
    }
}

/// A plugin: its version, its store, how fast it answers, an update with what it adds, what it
/// may contact, and turning it off or removing it.
struct PhonePluginPage: View {
    @Environment(AppStore.self) private var store
    let id: String
    @State private var removing = false
    @State private var confirmingUpdate = false

    var body: some View {
        let row = store.providers.row(id)
        let listing = store.stores.origin(of: id)
        PhoneList {
            Section {
                value(L.sourcesVersion, listing?.row.installedVersion ?? L.sourcesOriginNoStore)
                if let listing {
                    NavigationLink(value: PhoneSettingsPage.store(listing.store.id)) {
                        HStack {
                            Text(L.storesStoreLabel).font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            Text(listing.store.name).font(.ui(17)).foregroundStyle(.haze)
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
                value(L.sourcesResponseTime, row.map { r in
                    r.standing == .failing ? L.sourcesNoAnswer : r.latencyMs.map { L.sourcesResponseMs(ms: Int($0)) } ?? L.sourcesNotChecked
                } ?? L.sourceNoteGone)
            }
            if let listing, case .update(_, let added) = listing.row.status {
                Section {
                    Button {
                        confirmingUpdate = true
                    } label: {
                        Text(L.sourcesUpdateTo(version: listing.row.version)).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(store.stores.isWorking(.install(store: listing.store.id, plugin: id)))
                    .pluginUpdateAlert(isPresented: $confirmingUpdate, store: listing.store, plugin: listing.row)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                } footer: {
                    if !added.isEmpty {
                        Text(L.storesUpdateContacts(hosts: added.formatted(.list(type: .and).locale(AirTime.locale))))
                    }
                }
            }
            Section {
                ForEach(listing?.row.hosts ?? [], id: \.self) { host in
                    Text(host).font(.ui(17)).foregroundStyle(.silk).listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader(L.sourcesContactsLabel)
            } footer: {
                Text(L.storesPluginLimits)
            }
            Section {
                let disabled = store.isPluginDisabled(id)
                Button(disabled ? "Enable" : "Disable") { store.setPluginDisabled(id, !disabled) }
                    .listRowBackground(Palette.raised)
                Button(L.actionRemove, role: .destructive) { removing = true }
                    .fontWeight(.semibold)
                    .listRowBackground(Palette.raised)
            }
        }
        .navigationTitle(listing?.row.name ?? id)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog(L.storesRemovePluginTitle(name: listing?.row.name ?? id), isPresented: $removing, titleVisibility: .visible) {
            Button(L.actionRemove, role: .destructive) { store.removePlugin(id) }
        }
    }

    private func value(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Text(value).font(.ui(17)).foregroundStyle(.haze)
        }
        .listRowBackground(Palette.raised)
    }
}

// MARK: - Sheets

/// Joining a store on iPhone: the link, what it leads to, and Join at the foot.
struct PhoneJoinStoreSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @State var link: String

    var body: some View {
        NavigationStack {
            PhoneList {
                Section {
                    TextField(L.storesInviteLink, text: $link, prompt: Text(L.storesInviteLinkHint))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader(L.storesInviteLink)
                } footer: {
                    Text(L.storesJoinDetailOpened)
                }
                preview
                if case .store(let invited)? = store.stores.preview, !invited.anonymous {
                    Section {
                        TextField(L.storesDeviceName,
                                  text: Binding(get: { store.stores.deviceName }, set: { store.stores.deviceName = $0 }),
                                  prompt: Text(Device.name))
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader(L.storesDeviceName)
                    } footer: {
                        Text(L.storesDeviceNameDetail)
                    }
                }
            }
            .navigationTitle(L.storesJoinTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(L.actionCancel) { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                Button {
                    store.joinStore(link)
                } label: {
                    Text(L.storesJoinButton).frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                .disabled(!canJoin || store.stores.isWorking(.join))
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .tint(.silk)
        .task(id: link) {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            if InviteLinks.looksLikeInvite(link) { store.previewStore(link) } else { store.stores.clearPreview() }
        }
        .onChange(of: store.stores.joined) { _, joined in
            if joined != nil { dismiss() }
        }
        .onDisappear { store.stores.clearPreview() }
    }

    private var canJoin: Bool {
        if case .store(let invited) = store.stores.preview { return !invited.alreadyJoined }
        return false
    }

    @ViewBuilder
    private var preview: some View {
        switch store.stores.preview {
        case .checking?:
            Section {
                HStack(spacing: 8) {
                    ProgressView()
                    Text(L.storesCheckingLink).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
            }
        case .failed?:
            Section {
                FailedLine(notAStoreWords, size: 15).listRowBackground(Palette.raised)
            }
        case .store(let invited)?:
            Section {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(invited.name).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                        Text(invited.runByLine).font(.ui(13)).foregroundStyle(.haze)
                    }
                    Spacer(minLength: 0)
                    if !invited.plugins.isEmpty {
                        Text(L.storesPluginsCount(count: invited.plugins.count))
                            .font(.ui(15)).monospacedDigit().foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                HStack(spacing: 10) {
                    Image(systemName: "checkmark").font(.ui(15, weight: .semibold)).foregroundStyle(.water)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(L.storesKeyMatches(key: invited.fingerprint)).font(.ui(15)).foregroundStyle(.silk)
                        Text(invited.fingerprint).font(.ui(13, design: .monospaced)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                if invited.open {
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: "person.2").foregroundStyle(.haze)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.storesAnyoneWithLink).font(.ui(17)).foregroundStyle(.silk)
                            if invited.anonymous {
                                Text(L.storesJoinNameless).font(.ui(15)).foregroundStyle(.haze)
                            }
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
                ForEach(invited.plugins, id: \.id) { plugin in
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(plugin.name).font(.ui(17)).foregroundStyle(.silk).lineLimit(1)
                            Text(([plugin.contactsLine] + [plugin.standing].compactMap { $0 }).joined(separator: " · "))
                                .font(.ui(13)).foregroundStyle(.haze).lineLimit(1)
                        }
                        Spacer(minLength: 0)
                        Text(plugin.version).font(.ui(15)).monospacedDigit().foregroundStyle(.haze)
                    }
                    .accessibilityElement(children: .combine)
                    .listRowBackground(Palette.raised)
                }
            } footer: {
                Text(invited.alreadyJoined
                     ? L.storesAlreadyJoinedNote
                     : L.storesNotFromApp)
            }
        case nil:
            EmptyView()
        }
    }
}

/// Turning torrents on, on iPhone: as the Mac's sheet, in a list. And the same, asking for a
/// proxy once torrents are on, or editing the saved one. A proxy is saved only once the
/// connection check through it passes.
struct PhoneTorrentSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    var mode: TorrentSheetMode = .turnOn
    var start: TorrentProtection?
    @State private var draft: TorrentDraft?
    /// What the check under way is for, saved once it passes.
    @State private var checking: TorrentDraft?
    @State private var confirmingNone = false

    private var turningOn: Bool { mode == .turnOn }
    private var editing: Bool { mode == .editProxy }

    var body: some View {
        let draft = Binding(get: { self.draft ?? TorrentDraft(store) }, set: { self.draft = $0 })
        let trial = store.providers.trial
        NavigationStack {
            PhoneList {
                Section {
                    Text(editing
                         ? ProtectionWords.editNote
                         : L.torrentsRisk)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                        .listRowBackground(Color.clear)
                        // In line with the section headers and the rows' words below.
                        .listRowInsets(EdgeInsets(top: 0, leading: 16, bottom: 0, trailing: 16))
                }
                if turningOn {
                    Section {
                        TextField(L.torrentsIndex, text: draft.index, prompt: Text(L.torrentsIndexHint))
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader(L.torrentsIndex)
                    } footer: {
                        Text(L.torrentsIndexDetail + " " + L.legalHostsNoContent)
                    }
                }
                if !editing {
                    Section {
                        ForEach(ProtectionWords.order, id: \.self) { choice in
                            Button { draft.wrappedValue.protection = choice } label: {
                                self.choice(ProtectionWords.label(choice), on: draft.wrappedValue.protection == choice)
                            }
                            .listRowBackground(Palette.raised)
                        }
                    } header: {
                        PhoneSectionHeader(L.torrentsProtectWith)
                    } footer: {
                        // With none, nothing checks the connection, so the check's note would mislead.
                        Text(draft.wrappedValue.protection == TorrentProtection.none
                             ? L.torrentsProtectNoneNote
                             : ProtectionWords.checkNote)
                    }
                }
                if draft.wrappedValue.protection == .proxy {
                    // Each field named beside it, so a filled-in one still says what it is.
                    Section {
                        LabeledContent(L.torrentsProxyHost) {
                            TextField(L.torrentsProxyHost, text: draft.host, prompt: Text("proxy.example")) // i18n-skip
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .multilineTextAlignment(.trailing)
                        }
                        LabeledContent(L.torrentsProxyPort) {
                            TextField(L.torrentsProxyPort, text: draft.port, prompt: Text("1080"))
                                .keyboardType(.numberPad)
                                .multilineTextAlignment(.trailing)
                        }
                        LabeledContent(L.torrentsProxyUser) {
                            TextField(L.torrentsProxyUser, text: draft.user, prompt: Text(L.torrentsProxyOptional))
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .multilineTextAlignment(.trailing)
                        }
                        LabeledContent(L.torrentsProxyPassword) {
                            SecureField(L.torrentsProxyPassword, text: draft.password, prompt: Text(L.torrentsProxyOptional))
                                .multilineTextAlignment(.trailing)
                        }
                    } header: {
                        PhoneSectionHeader(L.torrentsProtectProxy)
                    } footer: {
                        if editing { Text(ProtectionWords.passwordKept) }
                    }
                    .listRowBackground(Palette.raised)
                }
            }
            // Held still while the check through the proxy runs.
            .disabled(trial == .checking)
            .navigationTitle(editing ? L.torrentsSheetEditProxy : turningOn ? L.torrentsSheetTurnOn : L.torrentsSheetProtect)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(L.actionCancel) { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                VStack(alignment: .leading, spacing: 12) {
                    if ProxyCheckLine.shows(trial) {
                        ProxyCheckLine(trial: trial, mode: mode, phone: true).padding(.horizontal, 20)
                    }
                    Button {
                        if draft.wrappedValue.protection == TorrentProtection.none {
                            confirmingNone = true
                        } else {
                            send(draft.wrappedValue)
                        }
                    } label: {
                        Text(turningOn ? L.torrentsTurnOnButton : L.actionSave).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(!draft.wrappedValue.complete || trial == .checking)
                    .padding(.horizontal, 16)
                }
                .padding(.bottom, 8)
            }
        }
        .tint(.silk)
        .onAppear {
            var first = TorrentDraft(store)
            if let start { first.protection = start }
            self.draft = first
        }
        .modifier(TorrentSaving(checking: checking, draft: self.draft, mode: mode) { dismiss() })
        .confirmationDialog(ProtectionWords.noneTitle, isPresented: $confirmingNone, titleVisibility: .visible) {
            Button(L.torrentsProtectNone, role: .destructive) {
                send(draft.wrappedValue)
            }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(ProtectionWords.noneMessage)
        }
    }

    private func send(_ draft: TorrentDraft) {
        if store.save(draft, mode) {
            dismiss()
        } else {
            checking = draft
        }
    }

    private func choice(_ title: String, on: Bool) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            if on { Image(systemName: "checkmark").foregroundStyle(.amber) }
        }
        .contentShape(.rect)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
#endif
