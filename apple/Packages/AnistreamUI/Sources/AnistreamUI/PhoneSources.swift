// Providers and plugin stores on iPhone, as pages of the Settings sheet, and the sheets for
// joining a store and turning torrents on.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Providers

/// The sources in failover order, each with its switch; Edit to drag them into another order.
struct PhoneProvidersPage: View {
    @Environment(AppStore.self) private var store
    @State private var turningOn = false

    var body: some View {
        let rows = store.orderedProviders
        PhoneList {
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
                    Text("anistream ships no sources of its own and hosts no content. Every source is external.")
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
                // E9: `DisablePlugin` turns one plugin off.
                Toggle(info.name, isOn: .constant(true))
                    .labelsHidden()
                    .tint(.amber)
                    .disabled(true)
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
                    if row.state != .revoked {
                        Button("Link another device") { inviting = true }
                            .fontWeight(.semibold)
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button("Leave store", role: .destructive) { leaving = true }
                        .fontWeight(.semibold)
                        .listRowBackground(Palette.raised)
                } footer: {
                    // Who runs it, and when it was checked, wait on E9's fields on `StoreRow`.
                    Text("\(StoreWords.meta(row))\nKey \(row.fingerprint) · joined as \(row.member)")
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .confirmationDialog("Leave \(row.name)?", isPresented: $leaving, titleVisibility: .visible) {
                Button("Leave Store", role: .destructive) { appStore.leaveStore(row.id) }
            } message: {
                Text("This iPhone can no longer install or update its plugins. Plugins already installed stay.")
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
            Text("Update")
                .font(.ui(15, weight: .semibold))
                .foregroundStyle(.ground)
                .padding(.horizontal, 14)
                .frame(height: 30)
                .background(.amber, in: .capsule)
                .opacity(working ? 0.45 : 1)
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
                        store.installPlugin(id, from: listing.store.id)
                    } label: {
                        Text("Update to \(listing.row.version)").frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(store.stores.isWorking(.install(store: listing.store.id, plugin: id)))
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
                // E9: `DisablePlugin`.
                Button("Disable") {}.disabled(true).listRowBackground(Palette.raised)
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
        case .failed(let reason)?:
            Section {
                Text(reason.firstCapitalized + ".").foregroundStyle(.vermilion).listRowBackground(Palette.raised)
            }
        case .store(let invited)?:
            Section {
                VStack(alignment: .leading, spacing: 2) {
                    // The plugin count, who runs it and its plugins wait on E9's fields on `InvitedStore`.
                    Text(invited.name).font(.ui(20, weight: .semibold)).foregroundStyle(.silk)
                    Text(URL(string: invited.url)?.host ?? invited.url).font(.ui(15)).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
                HStack(alignment: .top, spacing: 12) {
                    Image(systemName: "checkmark").foregroundStyle(.water)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Key matches the invite").font(.ui(17)).foregroundStyle(.silk)
                        Text(invited.fingerprint).font(.ui(15, design: .monospaced)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
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

/// Turning torrents on, on iPhone: as the Mac's sheet, in a list.
struct PhoneTorrentSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let mode = store.settings.value(.vpnMode) ?? "socks5"
        NavigationStack {
            PhoneList {
                Section {
                    Text("Other people can see your IP address while you download and share a torrent.")
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
                }
                Section {
                    // E9: `rss_url`.
                    TextField("Torrent index", text: .constant(""), prompt: Text("https://…/?q={query}"))
                        .disabled(true)
                        .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader("Torrent index")
                } footer: {
                    Text("The RSS search address anistream asks for torrents. anistream ships none.")
                }
                Section {
                    // E8: `SetTorrentProtection`; the config file's choice is ticked.
                    choice("SOCKS5 proxy", on: mode == "socks5")
                    choice("VPN on this device", on: mode == "external")
                    choice("No protection", on: mode == "none")
                } header: {
                    PhoneSectionHeader("Protect with")
                } footer: {
                    Text("anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.")
                }
            }
            .navigationTitle("Turn on torrents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Cancel") { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                Button {
                    store.setSwitch(.torrents, on: true)
                    dismiss()
                } label: {
                    Text("Turn on").frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .tint(.amber)
    }

    private func choice(_ title: String, on: Bool) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            if on { Image(systemName: "checkmark").foregroundStyle(.amber) }
        }
        .opacity(0.6)
        .listRowBackground(Palette.raised)
    }
}
#endif
