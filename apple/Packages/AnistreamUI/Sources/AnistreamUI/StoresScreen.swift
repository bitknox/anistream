// Plugin stores on the Mac and a wide iPad: the stores joined down the left, the selected one's
// plugins beside them with what each may contact, and the sheets for joining, updating and what
// an owner does.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct PluginStoresScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var selection: String?

    var body: some View {
        let stores = store.stores.stores
        Group {
            if store.stores.loaded, stores.isEmpty {
                NoStores()
            } else {
                HStack(alignment: .top, spacing: 0) {
                    StoreList(selection: $selection)
                        .frame(width: 300)
                    if let row = stores.first(where: { $0.id == selection }) ?? stores.first {
                        StoreDetail(row: row)
                            .id(row.id)
                    }
                }
                .padding(.leading, 12)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task { store.loadStores() }
        .onChange(of: store.stores.joined, initial: true) { _, joined in
            if let joined {
                selection = joined
                store.stores.clearJoined()
            }
        }
    }
}

/// No store joined yet: what stores are, and the way to join one.
struct NoStores: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        VStack(spacing: 12) {
            Text("No plugin stores yet")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text("Plugin stores are run by people you know. Join one with the invite link its owner sends you.")
                .font(.mac(13))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            Button("Join Store…") { navigator.invite = "" }
                .buttonStyle(SecondaryCapsuleStyle())
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The stores joined, each with how it stands.
struct StoreList: View {
    @Environment(AppStore.self) private var store
    @Binding var selection: String?

    var body: some View {
        let stores = store.stores.stores
        let current = selection ?? stores.first?.id
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                Text("Joined")
                    .groupHeaderStyle()
                    .accessibilityAddTraits(.isHeader)
                    .macLineHeight(14)
                    .padding(EdgeInsets(top: 10, leading: 12, bottom: 8, trailing: 12))
                ForEach(stores, id: \.id) { row in
                    let selected = row.id == current
                    Button {
                        selection = row.id
                    } label: {
                        HStack(spacing: 12) {
                            Text(row.name)
                                .font(.mac(13, weight: selected ? .medium : .regular))
                                .foregroundStyle(selected ? .amber : .silk)
                                .lineLimit(1)
                            Spacer(minLength: 0)
                            StoreStanding(row: row, selected: selected)
                        }
                        .padding(.horizontal, 12)
                        .frame(height: 36)
                        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
                        .contentShape(.rect(cornerRadius: 8))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selected ? .isSelected : [])
                }
                Text("Plugin stores are run by people you know. Join one with the invite link its owner sends you.")
                    .font(.mac(12))
                    .macLineHeight(17)
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(EdgeInsets(top: 14, leading: 12, bottom: 0, trailing: 12))
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
    }
}

/// How a store stands, beside its name: updates waiting, offline, or no longer letting you in.
struct StoreStanding: View {
    let row: StoreRow
    var selected = false
    var size: CGFloat = 12

    var body: some View {
        switch row.state {
        case .revoked:
            label("Access removed", dot: .vermilion, color: selected ? .silk : .vermilion)
        case .offline:
            label("Offline", ring: true, color: .haze)
        case .ready where row.updates > 0:
            label(row.updates == 1 ? "1 update" : "\(row.updates) updates", dot: .water, color: .silk)
        case .ready:
            EmptyView()
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

/// One store: its name, its address, its plugins with what each may contact and the action
/// each takes, and the key it was joined by.
struct StoreDetail: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let row: StoreRow
    @State private var sheet: StoreSheet?
    @State private var leaving = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(row.name)
                            .font(.mac(22, weight: .bold))
                            .macLineHeight(28)
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                        // "Run by" and when it was last checked wait on E9's fields on `StoreRow`.
                        Text(StoreWords.meta(row))
                            .font(.mac(13))
                            .foregroundStyle(.haze)
                        if row.open {
                            Text("Anyone with the link can join")
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                        }
                    }
                    Spacer()
                    StoreMenu(row: row, sheet: $sheet, leaving: $leaving)
                }
                if case .offline(let reason) = row.state {
                    Text("Offline: \(reason). The plugins below are the last list that was checked.")
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                        .padding(.top, 8)
                }
                if row.state == .revoked {
                    Text("This store no longer lets this \(Device.name) in. Its plugins stay installed until you remove them.")
                        .font(.mac(12))
                        .foregroundStyle(.vermilion)
                        .padding(.top, 8)
                }
                Text("Plugins")
                    .groupHeaderStyle()
                    .accessibilityAddTraits(.isHeader)
                    .macLineHeight(14)
                    .padding(EdgeInsets(top: 28, leading: 18, bottom: 8, trailing: 0))
                VStack(spacing: 0) {
                    Rectangle().fill(.faintRule).frame(height: 1)
                    ForEach(row.plugins, id: \.id) { plugin in
                        StorePluginLine(store: row, plugin: plugin, sheet: $sheet)
                        Rectangle().fill(.faintRule).frame(height: 1)
                    }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("Each plugin can contact only the hosts listed with it and cannot read your files.")
                    Text("Store key \(row.fingerprint) · \(row.joinedAs)")
                }
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 20, leading: 18, bottom: 0, trailing: 0))
            }
            .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
        }
        .scrollIndicators(.never)
        .sheet(item: $sheet) { sheet in
            switch sheet {
            case .update(let plugin): UpdatePluginSheet(store: row, plugin: plugin)
            case .invite(let device): InviteSheet(store: row, device: device)
            case .members: MembersSheet(store: row)
            case .joining: OpenJoinSheet(store: row)
            case .publish: PublishSheet(store: row)
            }
        }
        .onLaunchSheet("update", navigator) {
            if let plugin = row.plugins.first(where: { if case .update = $0.status { true } else { false } }) {
                sheet = .update(plugin)
            }
        }
        .confirmationDialog("Leave \(row.name)?", isPresented: $leaving) {
            Button("Leave Store", role: .destructive) { store.leaveStore(row.id) }
        } message: {
            Text("This \(Device.name) can no longer install or update its plugins. Plugins already installed stay.")
        }
    }
}

/// A sheet a store screen opens.
enum StoreSheet: Identifiable, Hashable {
    case update(StorePluginRow)
    case invite(device: Bool)
    case members
    case joining
    case publish

    var id: String {
        switch self {
        case .update(let plugin): "update-\(plugin.id)"
        case .invite(let device): "invite-\(device)"
        case .members: "members"
        case .joining: "joining"
        case .publish: "publish"
        }
    }
}

/// One plugin in a store: its name, what it may contact, and its action.
struct StorePluginLine: View {
    @Environment(AppStore.self) private var appStore
    let store: StoreRow
    let plugin: StorePluginRow
    @Binding var sheet: StoreSheet?

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(plugin.name)
                    .font(.mac(13, weight: .medium))
                    .macLineHeight(17)
                    .foregroundStyle(.silk)
                Text(line)
                    .font(.mac(12))
                    .macLineHeight(15)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            action
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .frame(minHeight: 48)
        .accessibilityElement(children: .combine)
    }

    private var line: String {
        if case .addedByHand = plugin.status {
            return "Installed by hand · " + plugin.contactsLine.prefix(1).lowercased() + plugin.contactsLine.dropFirst()
        }
        return plugin.contactsLine
    }

    @ViewBuilder
    private var action: some View {
        let live = store.state != .revoked
        let working = appStore.stores.isWorking(.install(store: store.id, plugin: plugin.id))
        switch plugin.status {
        case .installed:
            HStack(spacing: 6) {
                Image(systemName: "checkmark").font(.mac(11, weight: .semibold)).foregroundStyle(.water)
                Text("Installed").font(.mac(12)).foregroundStyle(.haze)
            }
        case .update where live:
            Button("Update") { sheet = .update(plugin) }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .disabled(working)
                .accessibilityLabel("Update \(plugin.name) to \(plugin.version)")
        case .available where store.state == .ready:
            Button("Install") { appStore.installPlugin(plugin.id, from: store.id) }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(working)
                .accessibilityLabel("Install \(plugin.name)")
        case .available:
            Text(live ? "Offline" : "Not available").font(.mac(12)).foregroundStyle(.haze)
        case .addedByHand where live:
            Button("Replace") { sheet = .update(plugin) }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(working)
                .accessibilityLabel("Replace the copy of \(plugin.name) added by hand")
        case .otherStore(let other):
            Text("From \(other)").font(.mac(12)).foregroundStyle(.haze)
        default:
            EmptyView()
        }
    }
}

/// The store's actions: invites, members and who can join for its owner, another device, and
/// leaving.
struct StoreMenu: View {
    @Environment(AppStore.self) private var store
    let row: StoreRow
    @Binding var sheet: StoreSheet?
    @Binding var leaving: Bool

    var body: some View {
        Menu {
            if row.state != .revoked {
                if row.owner {
                    Button("Invite Someone…") { sheet = .invite(device: false) }
                    Button("Show Members") { sheet = .members }
                    Button("Who Can Join…") { sheet = .joining }
                    Button("Publish a Plugin…") { sheet = .publish }
                    Button("Open Admin Page") { store.openAdmin(row.id) }
                    Divider()
                }
                // A device that joined with the store's link joins another the same way.
                if !row.anonymous {
                    Button("Link Another Device…") { sheet = .invite(device: true) }
                    Divider()
                }
            }
            Button("Leave Store…") { leaving = true }
        } label: {
            Image(systemName: "ellipsis")
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: 28, height: 28)
                .background(.raised, in: .circle)
                .overlay(Circle().strokeBorder(.faintRule, lineWidth: 1))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel("Actions for \(row.name)")
    }
}

enum StoreWords {
    /// Where the store is: its host.
    static func meta(_ row: StoreRow) -> String {
        URL(string: row.url)?.host ?? row.url
    }
}

// MARK: - Sheets

/// Confirming an update, or replacing a copy added by hand: the version, and what it adds to
/// what the plugin may contact.
struct UpdatePluginSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    let plugin: StorePluginRow

    var body: some View {
        let added: [String] = if case .update(_, let new) = plugin.status { new } else { [] }
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(subtitle)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            if !added.isEmpty {
                Text("This update also contacts \(Text(added.joined(separator: ", ")).fontWeight(.semibold)).")
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                    .padding(.top, 16)
            }
            HStack(alignment: .top, spacing: 24) {
                Text("Contacts").foregroundStyle(.haze)
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(plugin.hosts, id: \.self) { host in
                        HStack {
                            Text(host).foregroundStyle(.silk)
                            Spacer()
                            if added.contains(host) {
                                Text("New in \(plugin.version)").foregroundStyle(.haze)
                            }
                        }
                    }
                }
            }
            .font(.mac(13))
            .padding(14)
            .background(.ground, in: .rect(cornerRadius: 10))
            .padding(.top, 14)
            Text("Plugins cannot read your files or contact a host that is not listed.")
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 14)
            HStack(spacing: 10) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(confirm) {
                    appStore.installPlugin(plugin.id, from: store.id)
                    dismiss()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(540)
        .background(.raised)
    }

    private var byHand: Bool {
        if case .addedByHand = plugin.status { true } else { false }
    }

    private var title: String {
        byHand ? "Replace \(plugin.name)?" : "Update \(plugin.name)?"
    }

    private var subtitle: String {
        if case .update(let from, _) = plugin.status {
            return "Version \(plugin.version) from \(store.name) replaces \(from)."
        }
        return "Version \(plugin.version) from \(store.name) replaces the copy added by hand."
    }

    private var confirm: String {
        byHand ? "Replace" : "Update"
    }
}

/// Joining a store from an invite link: pasted, or the one the app was opened with, checked
/// against the store's key before anything is spent.
struct JoinStoreSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @State var link: String

    var body: some View {
        let working = store.stores.isWorking(.join)
        VStack(alignment: .leading, spacing: 0) {
            Text("Join a Plugin Store")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text("Paste the invite or the store's link you were given.")
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            Text("Link")
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 18)
            TextField("Link", text: $link, prompt: Text("https://…/join/…"))
                .textFieldStyle(.plain)
                .font(.mac(13))
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .labelsHidden()
                .padding(.top, 6)
            preview.padding(.top, 14)
            Text("Plugins come from the store's owner, not from anistream. You can leave a store at any time.")
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 14)
            HStack(spacing: 10) {
                Spacer()
                if working { ProgressView().controlSize(.small) }
                Button("Cancel") { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button("Join Store") { store.joinStore(link) }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canJoin || working)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(600)
        .background(.raised)
        .task(id: link) {
            // A pause while typing, then the link is checked.
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            if InviteLinks.looksLikeInvite(link) { store.previewStore(link) } else { store.stores.clearPreview() }
        }
        .onChange(of: store.stores.joined) { _, joined in
            if joined != nil {
                navigator.open(.pluginStores)
                dismiss()
            }
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
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Checking the link…").font(.mac(13)).foregroundStyle(.haze)
            }
        case .failed(let reason)?:
            Text(reason.firstCapitalized + ".")
                .font(.mac(13))
                .foregroundStyle(.vermilion)
        case .store(let invited)?:
            VStack(alignment: .leading, spacing: 6) {
                // The plugin count, who runs it and its plugins wait on E9's fields on `InvitedStore`.
                Text(invited.name)
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                Text(URL(string: invited.url)?.host ?? invited.url)
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                HStack(spacing: 6) {
                    Image(systemName: "checkmark").font(.mac(11, weight: .semibold)).foregroundStyle(.water)
                    Text("Key \(Text(invited.fingerprint).font(.mac(13, design: .monospaced)).foregroundStyle(.silk)) matches the \(invited.anonymous ? "link" : "invite")")
                        .font(.mac(13))
                        .foregroundStyle(.haze)
                }
                if invited.open {
                    HStack(spacing: 6) {
                        Image(systemName: "person.2").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                        Text("Anyone with the link can join").font(.mac(13)).foregroundStyle(.silk)
                    }
                }
                if invited.anonymous {
                    Text("You join without a name. Another \(Device.name) joins with the same link.")
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
                if invited.alreadyJoined {
                    Text("This \(Device.name) has joined this store already.")
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.ground, in: .rect(cornerRadius: 10))
        case nil:
            EmptyView()
        }
    }
}

/// An invite link to send: for someone new, named, or for another device of yours.
struct InviteSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    let device: Bool
    @State private var name = ""

    var body: some View {
        let link = appStore.stores.invite.flatMap { $0.store == store.id ? $0 : nil }
        VStack(alignment: .leading, spacing: 14) {
            Text(device ? "Link another device" : "Invite someone to \(store.name)")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let link {
                Text(device
                     ? "Open this link on the other device. It works once, until \(Self.expiry(link.expiresAt))."
                     : "Send this link to \(link.forWhom ?? "them"). It works once, until \(Self.expiry(link.expiresAt)).")
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                Text(link.link)
                    .font(.mac(12, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.ground, in: .rect(cornerRadius: 8))
                HStack {
                    Spacer()
                    ShareLink(item: link.link) { Text("Share…") }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    Button("Copy Link") { Pasteboard.copy(link.link) }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    Button("Done") { dismiss() }
                        .buttonStyle(PrimaryCapsuleStyle(height: 28))
                        .keyboardShortcut(.defaultAction)
                }
            } else {
                if !device {
                    TextField("Their name", text: $name)
                        .textFieldStyle(SheetFieldStyle())
                }
                HStack {
                    Spacer()
                    Button("Cancel") { dismiss() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .keyboardShortcut(.cancelAction)
                    Button("Make Link") { appStore.createInvite(store.id, for: device ? nil : name) }
                        .buttonStyle(PrimaryCapsuleStyle(height: 28))
                        .keyboardShortcut(.defaultAction)
                        .disabled((!device && name.trimmingCharacters(in: .whitespaces).isEmpty)
                                  || appStore.stores.isWorking(.invite(store: store.id)))
                }
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .onAppear { appStore.stores.clearInvite() }
    }

    static func expiry(_ seconds: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(date: .abbreviated, time: .shortened)
    }
}

/// A store's members and unused invites, for its owner.
struct MembersSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow

    var body: some View {
        let list = appStore.stores.members[store.id]
        VStack(alignment: .leading, spacing: 14) {
            Text("Members of \(store.name)")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let list {
                VStack(spacing: 0) {
                    ForEach(list.members, id: \.id) { member in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(member.name + (member.you ? " (you)" : "")).foregroundStyle(.silk)
                                Text(member.owner ? "Owner · \(Wording.plural(Int(member.devices), "device"))"
                                                  : Wording.plural(Int(member.devices), "device"))
                                    .font(.mac(12)).foregroundStyle(.haze)
                            }
                            Spacer()
                            if !member.you {
                                Button("Remove") { appStore.removeMember(member.id, from: store.id) }
                                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            }
                        }
                        .font(.mac(13))
                        .padding(.vertical, 8)
                    }
                    ForEach(list.invites, id: \.id) { invite in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(invite.device ? "Another device" : invite.name).foregroundStyle(.silk)
                                Text("Invited, until \(InviteSheet.expiry(invite.expiresAt))")
                                    .font(.mac(12)).foregroundStyle(.haze)
                            }
                            Spacer()
                            Button("Cancel Invite") { appStore.cancelInvite(invite.id, in: store.id) }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        }
                        .font(.mac(13))
                        .padding(.vertical, 8)
                    }
                }
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .task { appStore.loadMembers(store.id) }
    }
}

/// Who can join a store, for its owner: only people they invite, or anyone with the store's link.
/// While it is open, the link to copy and share; either way, the devices that joined with it,
/// which have no names, to remove one at a time or all at once.
struct OpenJoinSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    @State private var removingAll = false
    /// Long enough without an answer that the store's server is taken to have no open joining.
    @State private var waited = false

    var body: some View {
        let view = appStore.stores.openJoin[store.id]
        let working = appStore.stores.isWorking(.openJoin(store: store.id))
            || appStore.stores.isWorking(.removeAnonymous(store: store.id))
        VStack(alignment: .leading, spacing: 14) {
            Text("Who can join \(store.name)")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let view {
                Toggle(isOn: Binding(
                    get: { view.open },
                    set: { appStore.setOpenJoin(store.id, open: $0) }
                )) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Anyone with the store's link").foregroundStyle(.silk)
                        Text(view.open
                             ? "They join without a name. Turning this off stops the link; turning it on again makes a new one."
                             : "Only people you invite can join. Turned on, anyone with the link joins, without a name.")
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .toggleStyle(.switch)
                .disabled(working)
                .font(.mac(13))
                if let link = view.link {
                    Text(link)
                        .font(.mac(12, design: .monospaced))
                        .foregroundStyle(.silk)
                        .textSelection(.enabled)
                        .padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.ground, in: .rect(cornerRadius: 8))
                    HStack {
                        Spacer()
                        ShareLink(item: link) { Text("Share…") }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        Button("Copy Link") { Pasteboard.copy(link) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
                if view.devices > 0 {
                    Rectangle().fill(.faintRule).frame(height: 1)
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(Wording.plural(Int(view.devices), "device")) joined with the link")
                                .foregroundStyle(.silk)
                            Text(view.open ? "No names are kept." : "They keep working until you remove them.")
                                .font(.mac(12)).foregroundStyle(.haze)
                        }
                        Spacer()
                        Button(view.devices == 1 ? "Remove" : "Remove All") { removingAll = true }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            .disabled(working)
                    }
                    .font(.mac(13))
                    ForEach(view.recent, id: \.id) { device in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Joined \(Self.when(device.joinedAt))").foregroundStyle(.silk)
                                Text(device.lastSeen.map { "Seen \(Self.when($0))" } ?? "Not seen since")
                                    .font(.mac(12)).foregroundStyle(.haze)
                            }
                            Spacer()
                            Button("Remove") { appStore.removeAnonymous(device.id, from: store.id) }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                                .disabled(working)
                        }
                        .font(.mac(13))
                    }
                    if view.devices > UInt32(view.recent.count) {
                        Text("The newest \(view.recent.count) are listed.")
                            .font(.mac(12)).foregroundStyle(.haze)
                    }
                }
            } else if waited {
                Text("This store's server is older, and lets in only people you invite. Updating it adds the choice.")
                    .font(.mac(13)).foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(520)
        .background(.raised)
        .task {
            appStore.loadMembers(store.id)
            try? await Task.sleep(for: .seconds(4))
            waited = true
        }
        .confirmationDialog("Remove the devices that joined with the link?", isPresented: $removingAll) {
            Button("Remove", role: .destructive) { appStore.removeAnonymous(nil, from: store.id) }
        } message: {
            Text("They stop working with \(store.name) at once. Plugins already installed on them stay.")
        }
    }

    static func when(_ seconds: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(.relative(presentation: .named))
    }
}

/// The local plugins an owner could publish to the store.
struct PublishSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow

    var body: some View {
        let plugins = appStore.stores.localPlugins[store.id]
        VStack(alignment: .leading, spacing: 14) {
            Text("Publish a plugin to \(store.name)")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let plugins {
                if plugins.isEmpty {
                    Text("No plugins on this \(Device.name) to publish.")
                        .font(.mac(13)).foregroundStyle(.haze)
                }
                ForEach(plugins, id: \.id) { plugin in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(plugin.name) \(plugin.version)").foregroundStyle(.silk)
                            Text(plugin.published.map { "The store offers \($0)" } ?? "Not in the store yet")
                                .font(.mac(12)).foregroundStyle(.haze)
                        }
                        Spacer()
                        Button("Publish") { appStore.publishPlugin(plugin.id, to: store.id) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            .disabled(plugin.published == plugin.version)
                    }
                    .font(.mac(13))
                }
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .task { appStore.loadLocalPlugins(for: store.id) }
    }
}

/// The system pasteboard.
enum Pasteboard {
    /// The invite link on the pasteboard, when there is one.
    @MainActor
    static var invite: String? {
        #if os(macOS)
        let text = NSPasteboard.general.string(forType: .string)
        #else
        let text = UIPasteboard.general.string
        #endif
        return text.flatMap { URL(string: $0.trimmingCharacters(in: .whitespacesAndNewlines)) }
            .flatMap(InviteLinks.invite(in:))
    }

    @MainActor
    static func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.string = text
        #endif
    }
}
