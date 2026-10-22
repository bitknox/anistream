// What protects torrent traffic, and what a failed check does: the words for each choice, the
// turn-on sheet's draft, and the rows that change them once torrents are on. No protection and
// Warn only each ask the viewer to confirm first, and go to the engine acknowledged.

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
    static let noneTitle = "Torrent with no protection?"
    static let noneMessage = "Everyone in a torrent will see your IP address."
    /// What confirming Warn only says.
    static let warnTitle = "Keep torrents running when a check fails?"
    static let warnMessage = "anistream will say the check failed, and torrents will go on with your address showing."
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
            Button("Use No Protection", role: .destructive) {
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
        PhoneTorrentSheet(turningOn: false, start: .proxy)
        #else
        TorrentSheet(turningOn: false, start: .proxy)
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
            Button("Warn Only", role: .destructive) { store.setFailedCheck(.warn) }
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
