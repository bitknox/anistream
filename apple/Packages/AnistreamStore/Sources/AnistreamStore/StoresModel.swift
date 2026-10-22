// Plugin stores: the ones joined and their plugins, an invite link being checked or spent, and
// what an owner manages: invites, members, who can join, and plugins to publish.

import AnistreamKit
import Foundation
import Observation

/// The plugin stores, as the engine last said them.
@Observable @MainActor
public final class StoresModel {
    /// What a pasted invite link leads to.
    public enum Preview: Equatable, Sendable {
        /// Being checked against the store it names.
        case checking(link: String)
        /// The store, checked against the key the link carries.
        case store(InvitedStore)
        /// Why it leads nowhere, in words to show in its place.
        case failed(String)
    }

    /// What a store screen is waiting on, by the job the engine runs it as.
    public enum Work: Hashable, Sendable {
        case join
        case leave(store: String)
        case install(store: String, plugin: String)
        case remove(plugin: String)
        case invite(store: String)
        /// Letting anyone with the link join, or only invited people.
        case openJoin(store: String)
        /// Removing devices that joined with the link.
        case removeAnonymous(store: String)
    }

    /// Every joined store, in the order the engine lists them.
    public private(set) var stores: [StoreRow] = []
    /// Whether the engine has said the stores at all.
    public private(set) var loaded = false
    /// The invite link being checked, and what it leads to.
    public private(set) var preview: Preview?
    /// The store just joined, for the screen to move into: until it is shown.
    public private(set) var joined: String?
    /// An invite link to show and send.
    public private(set) var invite: InviteLinkView?
    /// Each store's members and unused invites, for its owner.
    public private(set) var members: [String: (members: [StoreMemberRow], invites: [StoreInviteRow])] = [:]
    /// The local plugins an owner could publish to each store.
    public private(set) var localPlugins: [String: [LocalPluginRow]] = [:]
    /// Who can join each store, its open link and the devices that joined with it, for its
    /// owner. Absent for a store whose server has no open joining.
    public private(set) var openJoin: [String: OpenJoinView] = [:]
    /// What is under way.
    public private(set) var working: [JobId: Work] = [:]

    public init() {}

    func apply(_ rows: [StoreRow]) {
        stores = rows
        loaded = true
    }

    func checking(_ link: String) {
        preview = .checking(link: link)
    }

    func previewed(_ answer: InvitePreview) {
        switch answer {
        case .store(let store): preview = .store(store)
        case .failed(let reason): preview = .failed(reason)
        }
    }

    func joined(_ store: String) {
        joined = store
        preview = nil
    }

    func invited(_ link: InviteLinkView) {
        invite = link
    }

    func members(store: String, members: [StoreMemberRow], invites: [StoreInviteRow]) {
        self.members[store] = (members, invites)
    }

    func localPlugins(store: String, plugins: [LocalPluginRow]) {
        localPlugins[store] = plugins
    }

    func openJoin(store: String, view: OpenJoinView) {
        openJoin[store] = view
    }

    func started(_ work: Work, job: JobId?) {
        if let job { working[job] = work }
    }

    func done(_ job: JobId) {
        working[job] = nil
    }

    /// Forget the invite link being checked: the join sheet closed.
    public func clearPreview() {
        preview = nil
    }

    /// The store just joined has been shown.
    public func clearJoined() {
        joined = nil
    }

    /// The invite link has been shown and closed.
    public func clearInvite() {
        invite = nil
    }

    public func isWorking(_ work: Work) -> Bool {
        working.values.contains(work)
    }

    public func store(_ id: String) -> StoreRow? {
        stores.first { $0.id == id }
    }

    /// Updates waiting across every store.
    public var updates: Int {
        stores.reduce(0) { $0 + $1.updates }
    }

    /// Where an installed plugin came from: the store that keeps it updated and its row there,
    /// or the first store that lists it as added by hand.
    public func origin(of plugin: String) -> (store: StoreRow, row: StorePluginRow)? {
        var byHand: (StoreRow, StorePluginRow)?
        for store in stores {
            guard let row = store.plugins.first(where: { $0.id == plugin }) else { continue }
            switch row.status {
            case .installed, .update: return (store, row)
            case .addedByHand: byHand = byHand ?? (store, row)
            case .available, .otherStore: continue
            }
        }
        return byHand
    }
}

extension StoreRow {
    /// Updates waiting in this store. `StoreRow::updates`.
    public var updates: Int {
        plugins.filter { if case .update = $0.status { true } else { false } }.count
    }
}

extension StorePluginRow {
    /// What it may contact, in a line: "Contacts open-archive.example and 1 more".
    public var contactsLine: String {
        guard let first = hosts.first else { return "Contacts no hosts" }
        return hosts.count > 1 ? "Contacts \(first) and \(hosts.count - 1) more" : "Contacts \(first)"
    }

    /// The version installed here: the one an update replaces, or the store's own.
    public var installedVersion: String {
        if case .update(let from, _) = status { return from }
        return version
    }
}

extension StoreRow {
    /// Who this device joined as: a name, or none, for one that joined with the store's link.
    public var joinedAs: String {
        anonymous ? "joined without a name" : "joined as \(member)"
    }
}

/// What an invite link the app was handed holds: the store's own `https://…/join/{code}#{key}`,
/// or its open link, the same with `?open` before the `#`.
public enum InviteLinks {
    /// The https invite in `url`, when it is one: an https link as the store makes it, or one the
    /// app was opened with, `anistream://join?link=<the https link>` or the https link with the
    /// app's scheme in front of the host, `anistream://plugins.example/join/{code}#{key}`.
    public static func invite(in url: URL) -> String? {
        switch url.scheme?.lowercased() {
        case "https":
            return url.path.contains("/join/") ? url.absoluteString : nil
        case "anistream":
            if url.host == "join" {
                let link = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                    .queryItems?.first { $0.name == "link" }?.value
                return link.flatMap(URL.init(string:)).flatMap(invite(in:))
            }
            guard url.host != "auth", var parts = URLComponents(url: url, resolvingAgainstBaseURL: false)
            else { return nil }
            parts.scheme = "https"
            return parts.url.flatMap(invite(in:))
        default:
            return nil
        }
    }

    /// Whether pasted `text` reads as an invite link, before the engine checks it.
    public static func looksLikeInvite(_ text: String) -> Bool {
        URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)).flatMap(invite(in:)) != nil
    }
}
