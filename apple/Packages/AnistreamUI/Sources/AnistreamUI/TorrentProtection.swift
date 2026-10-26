// What protects torrent traffic, and what a failed check does: the words for each choice, the
// turn-on sheet's draft, and the rows that change them once torrents are on. No protection and
// Warn only each ask the viewer to confirm first, and go to the engine acknowledged. A proxy,
// new or changed, is saved only once the connection check through it passes.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The words for each choice, as the turn-on sheet and Sources say them.
enum ProtectionWords {
    // `TorrentProtection.none` named in full throughout: beside an optional, `.none` is nil.
    static let order: [TorrentProtection] = [.proxy, .device, TorrentProtection.none]

    static func label(_ protection: TorrentProtection?) -> String {
        guard let protection else { return "–" }
        switch protection {
        case .proxy: return "SOCKS5 proxy"
        case .device: return "VPN on this device"
        case TorrentProtection.none: return "No protection"
        }
    }

    static func detail(_ protection: TorrentProtection) -> String {
        switch protection {
        case .proxy: "Torrent traffic goes through a proxy you run or rent."
        case .device: "Uses the VPN already running on this \(Device.name)."
        case TorrentProtection.none: "Your address is visible to everyone in the torrent."
        }
    }

    static let failedChecks: [FailedCheck] = [.pause, .stop, .warn]

    static func label(_ check: FailedCheck?) -> String {
        guard let check else { return "–" }
        switch check {
        case .pause: return "Pause torrents"
        case .stop: return "Stop torrents"
        case .warn: return "Warn only"
        }
    }

    static let checkNote = "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you."

    /// What confirming no protection says.
    static let noneTitle = "Use torrents without protection?"
    static let noneMessage = "Other people can see your IP address while you download and share a torrent, and anistream does not check the connection."
    /// What confirming Warn only says.
    static let warnTitle = "Keep torrents running if a check fails?"
    static let warnMessage = "anistream will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address."

    /// Edit proxy's line over its form.
    static let editNote = "Torrents keep using the saved proxy until the new one passes the connection check."
    /// Under the proxy's fields, on Edit proxy.
    static let passwordKept = "Leave the password empty to keep the saved one."
    /// While the check through a proxy runs.
    static let checkingProxy = "Checking the proxy"

    /// Why a proxy was not saved, from the check's `view` of it, as the sheet open says it.
    static func notSaved(_ view: GuardView, _ mode: TorrentSheetMode) -> String {
        let why = view == .leaking
            ? "Torrent traffic left outside this proxy"
            : "The check could not be made through this proxy"
        switch mode {
        case .turnOn: return "Not turned on. \(why)."
        case .change: return "Not saved. \(why), so torrents keep the protection they have."
        case .editProxy: return "Not saved. \(why), so the saved proxy stays."
        }
    }

    /// "proxy.example:1080 · as mika": where the saved proxy is, and who it lets in.
    static func proxyLine(_ proxy: ProxyView) -> String {
        let place = "\(proxy.host):\(proxy.port)"
        guard let user = proxy.user, !user.isEmpty else { return place }
        return "\(place) · as \(user)"
    }
}

extension ProtectionView {
    /// Where the saved proxy is, as Protect with says it, while torrents go through one.
    var proxyLine: String? {
        guard protection == .proxy, let proxy else { return nil }
        return ProtectionWords.proxyLine(proxy)
    }
}

/// Why a torrent sheet is open: torrents turning on, their protection changing, or the saved
/// proxy's details.
enum TorrentSheetMode: Equatable {
    case turnOn, change, editProxy
}

/// The turn-on sheet's choices before they are sent: the index, the protection, and the proxy's
/// fields. The password left empty keeps the one already kept.
struct TorrentDraft: Equatable {
    var index: String
    var protection: TorrentProtection
    var host: String
    var port: String
    var user: String
    var password = ""

    @MainActor init(_ store: AppStore) {
        let torrent = store.settings.torrent
        index = store.settings.sources.index ?? ""
        protection = torrent?.protection ?? .proxy
        host = torrent?.proxy?.host ?? ""
        port = torrent?.proxy.map { String($0.port) } ?? ""
        user = torrent?.proxy?.user ?? ""
    }

    /// The proxy as the engine takes it, when the fields make one.
    var proxy: TorrentProxy? {
        guard protection == .proxy, !host.trimmingCharacters(in: .whitespaces).isEmpty,
              let port = UInt16(port.trimmingCharacters(in: .whitespaces)), port > 0
        else { return nil }
        return TorrentProxy(
            host: host.trimmingCharacters(in: .whitespaces), port: port,
            user: user.isEmpty ? nil : user, password: password.isEmpty ? nil : password)
    }

    /// Whether it can be sent: a proxy needs its host and port.
    var complete: Bool {
        protection != .proxy || proxy != nil
    }
}

extension AppStore {
    /// Save the sheet's choices: through a proxy, only once the check through it passes, which
    /// `TorrentSaving` waits for; anything else at once. Whether it saved now.
    func save(_ draft: TorrentDraft, _ mode: TorrentSheetMode) -> Bool {
        if draft.protection == .proxy, let proxy = draft.proxy {
            checkTorrentProtection(.proxy, proxy: proxy)
            return false
        }
        apply(draft, turningOn: mode == .turnOn)
        return true
    }

    /// Send the sheet's choices: the index when one is typed, the protection, and torrents on when
    /// they are turned on here. No protection goes acknowledged, the viewer having confirmed it.
    func apply(_ draft: TorrentDraft, turningOn: Bool) {
        let index = draft.index.trimmingCharacters(in: .whitespacesAndNewlines)
        if index != (settings.sources.index ?? ""), !index.isEmpty {
            setSetting(.torrentIndex, to: .str(index))
        }
        setTorrentProtection(draft.protection, proxy: draft.proxy, acknowledged: draft.protection == TorrentProtection.none)
        if turningOn { setSwitch(.torrents, on: true) }
    }

    /// What a failed check does, Warn only going acknowledged once confirmed.
    func setFailedCheck(_ check: FailedCheck) {
        guard let torrent = settings.torrent, check != torrent.onFailure else { return }
        if check == .warn {
            setTorrentProtection(torrent.protection, onFailure: .warn, acknowledged: true)
        } else {
            setSetting(.vpnOnFailure, to: .str(check == .stop ? "stop" : "pause"))
        }
    }
}

/// A torrent sheet saving a proxy: once the check through `checking` passes, it is saved and the
/// sheet goes; a failed one saved nothing, and the sheet stays to say why. Changing the form
/// after a failed check clears what it said.
struct TorrentSaving: ViewModifier {
    @Environment(AppStore.self) private var store
    let checking: TorrentDraft?
    let draft: TorrentDraft?
    let mode: TorrentSheetMode
    let saved: () -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: store.providers.trial) { _, trial in
                guard trial?.passed == true, let checking else { return }
                store.providers.clearTrial()
                store.apply(checking, turningOn: mode == .turnOn)
                saved()
            }
            .onChange(of: draft) { before, _ in
                // Not the form's first filling in, as the sheet opens.
                if before != nil, store.providers.trial?.failed != nil { store.providers.clearTrial() }
            }
            .onDisappear { store.providers.clearTrial() }
    }
}

/// Where a torrent sheet's check stands: under way, or why nothing was saved. Nothing before a
/// check, or once one passed.
struct ProxyCheckLine: View {
    let trial: ProtectionTrial?
    let mode: TorrentSheetMode
    var phone = false

    /// Whether there is anything to say.
    static func shows(_ trial: ProtectionTrial?) -> Bool {
        trial == .checking || trial?.failed != nil
    }

    var body: some View {
        switch trial {
        case .checking?:
            HStack(spacing: 8) {
                Working(true)
                Text(ProtectionWords.checkingProxy).foregroundStyle(.haze)
            }
            .font(phone ? .ui(15) : .mac(12))
            .accessibilityElement(children: .combine)
        case .said(let view)? where trial?.failed != nil:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Circle()
                    .fill(.vermilion)
                    .frame(width: phone ? 8 : 6, height: phone ? 8 : 6)
                    .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + (phone ? 5 : 4) }
                Text(ProtectionWords.notSaved(view, mode))
                    .foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .font(phone ? .ui(15) : .mac(12))
            .accessibilityElement(children: .combine)
        default:
            EmptyView()
        }
    }
}

/// Edit proxy, beside Protect with once a proxy is set: opens the proxy's form, filled in.
struct EditProxyButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var editing = false

    var body: some View {
        if store.settings.torrent?.protection == .proxy {
            Button(cased(mac: "Edit Proxy…", touch: "Edit proxy…")) { editing = true }
                .sheet(isPresented: $editing) { TorrentSheet(mode: .editProxy) }
                .onLaunchSheet("proxy", navigator) { editing = true }
        }
    }
}

/// The torrent index's address, typed, sent when the field is left or Return is pressed.
struct TorrentIndexField: View {
    @Environment(AppStore.self) private var store
    var phone = false
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField("Torrent index", text: $text, prompt: Text("https://…/?q={query}"))
            .textFieldStyle(.plain)
            .font(phone ? .ui(17) : .mac(13, design: .monospaced))
            #if os(iOS)
            .keyboardType(.URL)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            #endif
            .focused($focused)
            .onSubmit(commit)
            .onChange(of: focused) { _, now in if !now { commit() } }
            .onAppear { text = store.settings.sources.index ?? "" }
            .onChange(of: store.settings.sources.index) { _, index in if !focused { text = index ?? "" } }
    }

    private func commit() {
        let typed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard typed != (store.settings.sources.index ?? ""), !typed.isEmpty else { return }
        store.setSetting(.torrentIndex, to: .str(typed))
    }
}

/// Protect with, once torrents are on: the device's VPN at once, a proxy through the sheet that
/// asks for it, and no protection once confirmed.
struct ProtectionMenu: View {
    @Environment(AppStore.self) private var store
    var phone = false
    @State private var askingProxy = false
    @State private var confirmingNone = false

    var body: some View {
        let current = store.settings.torrent?.protection
        Menu {
            ForEach(ProtectionWords.order, id: \.self) { choice in
                Toggle(isOn: Binding(get: { current == choice }, set: { if $0 { choose(choice) } })) {
                    Text(ProtectionWords.label(choice) + (choice == .proxy ? "…" : ""))
                }
            }
        } label: {
            MenuValue(text: ProtectionWords.label(current), phone: phone)
        }
        .modifier(MenuLook(phone: phone))
        .disabled(current == nil)
        .accessibilityLabel("Protect with")
        .sheet(isPresented: $askingProxy) { protectionSheet }
        .confirmationDialog(ProtectionWords.noneTitle, isPresented: $confirmingNone, titleVisibility: .visible) {
            Button(cased(mac: "No Protection", touch: "No protection"), role: .destructive) {
                store.setTorrentProtection(TorrentProtection.none, acknowledged: true)
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(ProtectionWords.noneMessage)
        }
    }

    private func choose(_ choice: TorrentProtection) {
        switch choice {
        case .proxy: askingProxy = true
        case .device: store.setTorrentProtection(.device)
        case TorrentProtection.none: confirmingNone = true
        }
    }

    @ViewBuilder private var protectionSheet: some View {
        #if os(iOS)
        PhoneTorrentSheet(mode: .change, start: .proxy)
        #else
        TorrentSheet(mode: .change, start: .proxy)
        #endif
    }
}

/// If a check fails: pause or stop at once, Warn only once confirmed.
struct FailedCheckMenu: View {
    @Environment(AppStore.self) private var store
    var phone = false
    @State private var confirmingWarn = false

    var body: some View {
        let current = store.settings.torrent?.onFailure
        Menu {
            ForEach(ProtectionWords.failedChecks, id: \.self) { check in
                Toggle(isOn: Binding(get: { current == check }, set: { if $0 { choose(check) } })) {
                    Text(ProtectionWords.label(check))
                }
            }
        } label: {
            MenuValue(text: ProtectionWords.label(current), phone: phone)
        }
        .modifier(MenuLook(phone: phone))
        .disabled(current == nil)
        .accessibilityLabel("If a check fails")
        .confirmationDialog(ProtectionWords.warnTitle, isPresented: $confirmingWarn, titleVisibility: .visible) {
            Button(cased(mac: "Warn Only", touch: "Warn only"), role: .destructive) { store.setFailedCheck(.warn) }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(ProtectionWords.warnMessage)
        }
    }

    private func choose(_ check: FailedCheck) {
        if check == .warn { confirmingWarn = true } else { store.setFailedCheck(check) }
    }
}

/// A capsule menu on the Mac and iPad, the plain value with chevrons on an iPhone.
private struct MenuLook: ViewModifier {
    let phone: Bool

    func body(content: Content) -> some View {
        if phone {
            content
        } else {
            content
                .menuStyle(.button)
                .buttonStyle(.plain)
                .menuIndicator(.hidden)
                .fixedSize()
        }
    }
}
