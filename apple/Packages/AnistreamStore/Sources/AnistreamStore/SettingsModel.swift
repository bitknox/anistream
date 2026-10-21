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

    /// One row: its value, and how it is changed.
    public func row(_ key: SettingKey) -> SettingRow? {
        view?.rows.first { $0.key == key }
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

/// Each tracker's sync state, the conflicts a pull found, and a device code being waited on.
@Observable @MainActor
public final class AccountsModel {
    /// Each tracker's state, by its name ("anilist"), in the order the engine first said them.
    public private(set) var trackers: [SyncState] = []
    public private(set) var conflicts: [ConflictRow] = []
    public private(set) var deviceCode: DeviceCodePrompt?
    /// Progress queued for sync since the trackers last said their outboxes.
    public private(set) var queued = 0
    /// Whether AniList is out of reach, as the metadata ladder last said: the app is showing
    /// what it has, and changes wait.
    public private(set) var offline = false
    /// When each tracker last said a sync went through with nothing left to send.
    public private(set) var syncedAt: [String: Date] = [:]
    /// The sign-ins and syncs under way, by job: the tracker each is for.
    public private(set) var working: [JobId: String] = [:]

    public init() {}

    public func tracker(_ name: String) -> SyncState? {
        trackers.first { $0.tracker == name }
    }

    func apply(_ state: SyncState, at now: Date = .now) {
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

    func deviceCode(_ prompt: DeviceCodePrompt?) {
        deviceCode = prompt
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
