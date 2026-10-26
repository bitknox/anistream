// Providers and plugin stores on iPhone, as pages of the Settings sheet, and the sheets for
// joining a store and turning torrents on.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
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
        content.alert("Update \(plugin.name)?", isPresented: $isPresented) {
            Button("Update") { appStore.installPlugin(plugin.id, from: store.id) }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(message)
        }
    }

    private var message: String {
        guard case .update(let from, let added) = plugin.status else { return "" }
        var words = "Version \(plugin.version) from \(store.name) replaces \(from)."
        if !added.isEmpty { words += " This update also contacts \(added.joined(separator: ", "))." }
        return words
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
                    Text("Sources are tried top to bottom. Tap Edit to change the order.")
                    Text("anistream hosts no content.")
                }
            }
        }
        .navigationTitle("Providers")
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
                Toggle("Torrents", isOn: Binding(
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
                Text("Plugin stores are run by people you know. Join one with the invite link its owner sends you.")
            }
        }
        .navigationTitle("Plugin stores")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { Button("Join store") { joining = true } }
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
                    PhoneSectionHeader("Plugins")
                } footer: {
                    Text("Each plugin can contact only the hosts listed with it and cannot read your files.")
                }
                Section {
                    // A device that joined with the store's link joins another the same way.
                    if row.state != .revoked, !row.anonymous {
                        Button("Link another device") { inviting = true }
                            .fontWeight(.semibold)
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button("Leave store", role: .destructive) { leaving = true }
                        .fontWeight(.semibold)
                        .listRowBackground(Palette.raised)
                } footer: {
                    // Who can join is changed on the Mac, an iPad or the store's admin page.
                    let about = [StoreWords.meta(row), row.open ? "anyone with the link can join" : nil]
                    let key = ["Key \(row.fingerprint)", row.joinedAs, StoreWords.checked(row)]
                    Text(about.compactMap { $0 }.joined(separator: " · ") + "\n"
                         + key.compactMap { $0 }.joined(separator: " · "))
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .confirmationDialog("Leave \(row.name)?", isPresented: $leaving, titleVisibility: .visible) {
                Button("Leave store", role: .destructive) { appStore.leaveStore(row.id) }
            } message: {
                Text("This \(Device.name) can no longer install or update its plugins. Plugins already installed stay.")
            }
            .sheet(isPresented: $inviting) { InviteSheet(store: row, device: true) }
        } else {
            Text("This store has been left.").foregroundStyle(.haze)
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
                Text("Installed").foregroundStyle(.haze)
            }
            .font(.ui(15))
        case .update where store.state != .revoked:
            Button("Update") { confirming = true }
                .buttonStyle(PrimaryCapsuleStyle(height: 30, fontSize: 15))
                .disabled(working)
                .pluginUpdateAlert(isPresented: $confirming, store: store, plugin: plugin)
        case .available where store.state == .ready:
            Button("Install") { appStore.installPlugin(plugin.id, from: store.id) }
                .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                .disabled(working)
        case .otherStore(let other):
            Text("From \(other)").font(.ui(15)).foregroundStyle(.haze)
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
                value("Version", listing?.row.installedVersion ?? "Not from a store")
                if let listing {
                    NavigationLink(value: PhoneSettingsPage.store(listing.store.id)) {
                        HStack {
                            Text("Store").font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            Text(listing.store.name).font(.ui(17)).foregroundStyle(.haze)
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
                value("Response time", row.map { r in
                    r.standing == .failing ? "No answer" : r.latencyMs.map { "\($0) ms" } ?? "Not checked yet"
                } ?? "Not installed")
            }
            if let listing, case .update(_, let added) = listing.row.status {
                Section {
                    Button {
                        confirmingUpdate = true
                    } label: {
                        Text("Update to \(listing.row.version)").frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(store.stores.isWorking(.install(store: listing.store.id, plugin: id)))
                    .pluginUpdateAlert(isPresented: $confirmingUpdate, store: listing.store, plugin: listing.row)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                } footer: {
                    if !added.isEmpty {
                        Text("This update also contacts \(Text(added.joined(separator: ", ")).foregroundStyle(.silk)).")
                    }
                }
            }
            Section {
                ForEach(listing?.row.hosts ?? [], id: \.self) { host in
                    Text(host).font(.ui(17)).foregroundStyle(.silk).listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader("Contacts")
            } footer: {
                Text("\(listing?.row.name ?? id) can contact only these hosts and cannot read your files.")
            }
            Section {
                let disabled = store.isPluginDisabled(id)
                Button(disabled ? "Enable" : "Disable") { store.setPluginDisabled(id, !disabled) }
                    .listRowBackground(Palette.raised)
                Button("Remove", role: .destructive) { removing = true }
                    .fontWeight(.semibold)
                    .listRowBackground(Palette.raised)
            }
        }
        .navigationTitle(listing?.row.name ?? id)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog("Remove \(listing?.row.name ?? id)?", isPresented: $removing, titleVisibility: .visible) {
            Button("Remove", role: .destructive) { store.removePlugin(id) }
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
                    TextField("Invite link", text: $link, prompt: Text("https://…/join/…"))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader("Invite link")
                } footer: {
                    Text("Paste the link the store's owner sent you. An anistream:// invite link opened from Messages fills this sheet in directly.")
                }
                preview
                if case .store(let invited)? = store.stores.preview, !invited.anonymous {
                    Section {
                        TextField("Device name",
                                  text: Binding(get: { store.stores.deviceName }, set: { store.stores.deviceName = $0 }),
                                  prompt: Text(Device.name))
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader("Device name")
                    } footer: {
                        Text("The store's owner sees this \(Device.name) by this name.")
                    }
                }
            }
            .navigationTitle("Join a plugin store")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Cancel") { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                Button {
                    store.joinStore(link)
                } label: {
                    Text("Join store").frame(maxWidth: .infinity)
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
                    Text("Checking the link…").foregroundStyle(.haze)
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
                        Text(Wording.plural(invited.plugins.count, "plugin"))
                            .font(.ui(15)).monospacedDigit().foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                HStack(spacing: 10) {
                    Image(systemName: "checkmark").font(.ui(15, weight: .semibold)).foregroundStyle(.water)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Key matches the \(invited.anonymous ? "link" : "invite")").font(.ui(15)).foregroundStyle(.silk)
                        Text(invited.fingerprint).font(.ui(13, design: .monospaced)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                if invited.open {
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: "person.2").foregroundStyle(.haze)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Anyone with the link can join").font(.ui(17)).foregroundStyle(.silk)
                            if invited.anonymous {
                                Text("You join without a name.").font(.ui(15)).foregroundStyle(.haze)
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
                     ? "This iPhone has joined this store already."
                     : "Plugins come from the store's owner, not from anistream. You can leave a store at any time.")
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
                         : "Other people can see your IP address while you download and share a torrent.")
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
                }
                if turningOn {
                    Section {
                        TextField("Torrent index", text: draft.index, prompt: Text("https://…/?q={query}"))
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader("Torrent index")
                    } footer: {
                        Text("The RSS search address anistream asks for torrents. anistream hosts no content.")
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
                        PhoneSectionHeader("Protect with")
                    } footer: {
                        // With none, nothing checks the connection, so the check's note would mislead.
                        Text(draft.wrappedValue.protection == TorrentProtection.none
                             ? "With no protection, anistream does not check the connection."
                             : ProtectionWords.checkNote)
                    }
                }
                if draft.wrappedValue.protection == .proxy {
                    Section {
                        TextField("Host", text: draft.host, prompt: Text("Host"))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                        TextField("Port", text: draft.port, prompt: Text("Port")).keyboardType(.numberPad)
                        TextField("User name", text: draft.user, prompt: Text("User name"))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                        SecureField("Password", text: draft.password, prompt: Text("Password"))
                    } header: {
                        PhoneSectionHeader("Proxy")
                    } footer: {
                        if editing { Text(ProtectionWords.passwordKept) }
                    }
                    .listRowBackground(Palette.raised)
                }
            }
            // Held still while the check through the proxy runs.
            .disabled(trial == .checking)
            .navigationTitle(editing ? "Edit proxy" : turningOn ? "Turn on torrents" : "Protect torrents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Cancel") { dismiss() }.tint(.silk) }
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
                        Text(turningOn ? "Turn on" : "Save").frame(maxWidth: .infinity)
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
            Button("No protection", role: .destructive) {
                send(draft.wrappedValue)
            }
            Button("Cancel", role: .cancel) {}
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
