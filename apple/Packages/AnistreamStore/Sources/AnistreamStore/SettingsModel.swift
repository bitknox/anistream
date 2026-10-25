// The config as the engine shows it, and the trackers' sync.

import AnistreamKit
import Foundation
import Observation

/// The settings, as the engine last said them.
@Observable @MainActor
public final class SettingsModel {
    /// The whole view, from the first `Settings` event at launch on.
    public private(set) var view: SettingsView?
    /// The setting whose write was just refused and put back, until the next view.
    public private(set) var refused: SettingKey?

    public init() {}

    func apply(_ view: SettingsView) {
        self.view = view
        refused = view.refused
    }

    /// One row's value as the file holds it, in words.
    public func value(_ key: SettingKey) -> String? {
        row(key)?.value
    }

    /// One row: its value, and how it is changed. From the Settings screen's rows, or the native
    /// apps' own, which the terminal draws none of.
    public func row(_ key: SettingKey) -> SettingRow? {
        view?.rows.first { $0.key == key } ?? view?.nativeRows.first { $0.key == key }
    }

    /// How many shows set `key` for themselves, when any do.
    public func overriddenShows(_ key: SettingKey) -> UInt32? {
        view?.overridden.first { $0.key == key }?.shows
    }

    /// How torrent traffic is kept off the viewer's address, as the file has it.
    public var torrent: ProtectionView? {
        view?.torrent
    }

    /// Whether this engine can publish a Discord presence at all, as it says: the Mac's can, a
    /// phone's has none. False until the first view, so the settings are never shown and then
    /// taken away.
    public var hasPresence: Bool {
        view?.presenceAvailable ?? false
    }

    /// Whether a switch is on.
    public func isOn(_ key: SettingKey) -> Bool {
        row(key)?.isOn ?? false
    }

    /// The sources the config sets up.
    public var sources: ConfiguredSources {
        view?.sources ?? ConfiguredSources(order: [], disabled: [], torrent: false, remote: false, index: nil)
    }
}

/// Each show's own settings, as the engine last said them: what it sets for itself over
/// Settings, the tracks the player remembers for it, and the source it keeps.
@Observable @MainActor
public final class ShowSettingsModel {
    /// The sheets asked for or changed, by show.
    public private(set) var sheets: [AnilistId: ShowSettingsView] = [:]

    public init() {}

    func apply(_ view: ShowSettingsView) {
        sheets[view.id] = view
    }

    /// One of a show's rows.
    public func row(_ key: ShowSettingKey, of id: AnilistId) -> ShowSettingRow? {
        sheets[id]?.rows.first { $0.key == key }
    }
}

/// A sign-in the engine asked the system's sheet for (`Event::Authorize`): open `url` in it, and
/// hand back the redirect it ends on, the one to `callbackScheme`.
public struct SignInRequest: Hashable, Sendable {
    public let tracker: String
    public let url: URL
    public let callbackScheme: String

    public init(tracker: String, url: URL, callbackScheme: String) {
        self.tracker = tracker
        self.url = url
        self.callbackScheme = callbackScheme
    }
}

/// Each tracker's sync state, the conflicts a pull found, and a device code or a sign-in sheet
/// being waited on.
@Observable @MainActor
public final class AccountsModel {
    /// Each tracker's state, by its name ("anilist"), in the order the engine first said them.
    public private(set) var trackers: [SyncState] = []
    public private(set) var conflicts: [ConflictRow] = []
    public private(set) var deviceCode: DeviceCodePrompt?
    /// The sign-in the system's sheet is open for, AniList's or MyAnimeList's.
    public private(set) var signIn: SignInRequest?
    /// Progress queued for sync since the trackers last said their outboxes.
    public private(set) var queued = 0
    /// Whether AniList is out of reach, as the metadata ladder last said: the app is showing
    /// what it has, and changes wait.
    public private(set) var offline = false
    /// When each tracker last said a sync went through with nothing left to send.
    public private(set) var syncedAt: [String: Date] = [:]
    /// The title language the signed-in viewer set on AniList, once asked
    /// (`LoadTitleLanguage`): `nil` signed out, or when AniList did not say.
    public internal(set) var viewerTitleLanguage: TitleLanguage?
    /// The sign-ins and syncs under way, by job: the tracker each is for.
    public private(set) var working: [JobId: String] = [:]

    public init() {}

    public func tracker(_ name: String) -> SyncState? {
        trackers.first { $0.tracker == name }
    }

    func apply(_ state: SyncState, at now: Date = .now) {
        // Signed in: a code kept for the sheet has nothing more to ask.
        if state.connected, deviceCode?.tracker == state.tracker { deviceCode = nil }
        if let index = trackers.firstIndex(where: { $0.tracker == state.tracker }) {
            trackers[index] = state
        } else {
            trackers.append(state)
        }
        queued = 0
        if !state.connected {
            syncedAt[state.tracker] = nil
        } else if state.last != nil, state.outbox == 0, state.pullError == nil {
            syncedAt[state.tracker] = now
        }
    }

    func started(_ tracker: String, job: JobId?) {
        if let job { working[job] = tracker }
    }

    func done(_ job: JobId) {
        working[job] = nil
    }

    /// Whether a sign-in or a sync for `tracker` is under way.
    public func isWorking(_ tracker: String) -> Bool {
        working.values.contains(tracker)
    }

    func progressQueued() {
        queued += 1
    }

    func conflicts(_ rows: [ConflictRow]) {
        conflicts = rows
    }

    /// The engine's code, or its end. A code that ran out stays, so its sheet can say so and
    /// offer a new one; one approved, refused or closed goes.
    func deviceCode(_ prompt: DeviceCodePrompt?, at now: Date = .now) {
        if prompt == nil, let shown = deviceCode, Self.expired(shown, at: now),
           tracker(shown.tracker)?.connected != true {
            return
        }
        deviceCode = prompt
    }

    /// Whether `prompt`'s code has stopped working by `now`, a second early for the engine's
    /// poll to have given up.
    public nonisolated static func expired(_ prompt: DeviceCodePrompt, at now: Date = .now) -> Bool {
        guard let expiresAt = prompt.expiresAt else { return false }
        return Int64(now.timeIntervalSince1970) >= expiresAt - 1
    }

    func authorize(_ request: SignInRequest) {
        signIn = request
    }

    /// The sheet for `request` came back, or was closed.
    func signInEnded(_ request: SignInRequest) {
        if signIn == request { signIn = nil }
    }

    func metaStatus(_ rows: [MetaSourceRow]) {
        offline = rows.first { $0.source.lowercased() == "anilist" }.map { !$0.healthy } ?? false
    }

    /// What the toolbar's sync capsule says, and how.
    public var summary: SyncSummary {
        // AniList first, the tracker the catalogue comes from; any other signed in otherwise.
        let signedIn = trackers.filter(\.connected)
        guard let main = signedIn.first(where: { $0.tracker == "anilist" }) ?? signedIn.first
        else { return SyncSummary(text: "Not connected", state: .signedOut, tracker: nil) }
        let name = Self.name(main.tracker)
        let pending = Int(signedIn.reduce(0) { $0 + $1.outbox }) + queued
        if offline {
            // Quiet, as the brief has it: what waits, or only that AniList is out of reach.
            let text = pending > 0 ? "\(Wording.plural(pending, "change")) waiting to sync" : "Offline"
            return SyncSummary(text: text, state: .waiting, tracker: main.tracker)
        }
        if main.needsReauth {
            return SyncSummary(text: "Sign in to \(name) again", state: .problem, tracker: main.tracker)
        }
        if pending > 0 {
            let changes = Wording.plural(pending, "change")
            return SyncSummary(text: "\(changes) waiting to sync", state: .waiting, tracker: main.tracker)
        }
        if main.pullError != nil {
            return SyncSummary(text: "\(name) not synced", state: .problem, tracker: main.tracker)
        }
        return SyncSummary(text: "\(name) synced", state: .synced, tracker: main.tracker)
    }

    /// A tracker's name as a person writes it.
    public static func name(_ tracker: String) -> String {
        switch tracker {
        case "anilist": "AniList"
        case "mal", "myanimelist": "MyAnimeList"
        case "kitsu": "Kitsu"
        case "simkl": "Simkl"
        case "trakt": "Trakt"
        default: tracker.capitalized
        }
    }
}

/// The sync capsule's words and the state its dot shows.
public struct SyncSummary: Equatable, Sendable {
    public enum State: Equatable, Sendable {
        /// No tracker signed in: a hollow haze dot.
        case signedOut
        /// Synced: a water dot.
        case synced
        /// Changes waiting to go out: a haze dot.
        case waiting
        /// A pull failed or a sign-in expired: a vermilion dot.
        case problem
    }

    public let text: String
    public let state: State
    public let tracker: String?
}
