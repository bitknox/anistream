// New-episode notifications as the screens show them: the week coming up, whether the app may
// notify, each show's own switch, and turning them on, which asks for permission the first time.

import AnistreamKit
import Foundation
import Observation

/// What the Notifications pane and a show's settings show of new-episode notifications.
@Observable @MainActor
public final class NotificationsModel {
    /// The week as the engine last said it.
    public private(set) var week: UpcomingNotices?
    /// Whether the app may notify, as the system last said.
    public private(set) var permission: NoticePermission = .notAsked
    /// Whether the last switch turned on was declined, which its line says.
    public private(set) var declined = false
    /// Each show's own switch, as its settings last said: on, off, or following Settings.
    public private(set) var showSwitch: [AnilistId: Bool?] = [:]

    /// Where notifications are scheduled: the system's, or a quiet one in the Fixtures app and
    /// the tests. Set by the app at launch; with none, nothing is scheduled.
    @ObservationIgnored public var scheduler: NoticeScheduler? {
        didSet { Task { await refreshPermission() } }
    }

    public init() {}

    /// Take what an engine event says of notifications, and schedule the week it brings.
    func observe(_ event: Event) {
        switch event {
        case .upcomingNotices(let week):
            self.week = week
            if let scheduler {
                Task { await scheduler.reschedule(week) }
            }
        case .showSettings(let sheet):
            let row = sheet.rows.first { $0.key == .notifyNewEpisodes }
            if let row {
                if case .bool(let on) = row.own {
                    showSwitch[sheet.id] = .some(on)
                } else {
                    showSwitch[sheet.id] = .some(nil)
                }
            }
        default:
            break
        }
    }

    /// Whether the app schedules anything: it has a centre, and may notify. Until then, nothing
    /// asks for the week but the Notifications pane.
    public var schedules: Bool {
        scheduler != nil && permission == .allowed
    }

    /// Whether `event` can change the week: an episode ended or was marked, a sync pulled the
    /// list, or the titles changed form.
    nonisolated static func reschedules(after event: Event) -> Bool {
        switch event {
        case .playbackEnded, .progressQueued, .sync, .titlesChanged: true
        default: false
        }
    }

    /// Ask the system again whether the app may notify: at launch and on coming back to it, as
    /// it can change in the system's settings meanwhile.
    public func refreshPermission() async {
        guard let scheduler else { return }
        permission = await scheduler.centre.permission()
    }

    /// Permission to notify, asked for when it never was: what turning a switch on does first.
    /// False when it was declined, now or before.
    func permit() async -> Bool {
        guard let scheduler else { return false }
        var allowed = await scheduler.centre.permission()
        if allowed == .notAsked {
            allowed = await scheduler.centre.askPermission() ? .allowed : .denied
        }
        permission = allowed
        declined = allowed != .allowed
        return allowed == .allowed
    }

    // MARK: The pane

    /// What will notify in the next seven days, and the episodes that follow Settings while it
    /// is off: everything but what a show's own switch leaves out, in the order it airs.
    public var comingUp: [UpcomingNotice] {
        (week?.airings ?? []).filter { $0.reason != .switchedOff }
    }

    /// The shows the week leaves out because their own switch is off, each once.
    public var switchedOff: [String] {
        var seen = Set<AnilistId>()
        return (week?.airings ?? []).filter { $0.reason == .switchedOff && seen.insert($0.id).inserted }.map(\.title)
    }

    /// Whether a row of the week notifies: dimmed in the pane when it does not.
    public func notifies(_ row: UpcomingNotice) -> Bool {
        NoticePlan.notifies(row.reason, enabled: week?.enabled ?? false)
    }

    // MARK: A show

    /// Whether `id` is being watched: on the list as watching or rewatching, or watched here
    /// lately, so that following Settings notifies of it.
    public func isWatching(_ id: AnilistId) -> Bool {
        week?.watching.contains(id) ?? false
    }

    /// A show's own switch, as its settings sheet shows it.
    public func choice(_ id: AnilistId) -> ShowNoticeChoice {
        switch showSwitch[id] ?? nil {
        case true?: .on
        case false?: .off
        case nil: .followSettings
        }
    }
}

/// A show's own new-episode switch.
public enum ShowNoticeChoice: Hashable, Sendable, CaseIterable {
    case followSettings, on, off

    /// What the show's settings sheet says of it, given whether Settings has notifications on
    /// and whether the show is being watched. A show not being watched that follows Settings is
    /// not notified of, so it says off, and why. `capitalised` for the Mac's title case.
    public func label(settingOn: Bool, watching: Bool, capitalised: Bool) -> String {
        let useDefault = capitalised ? "Use Default" : "Use default"
        switch self {
        case .on: return "On"
        case .off: return "Off"
        case .followSettings:
            guard watching else { return "Off · not watching" }
            return "\(useDefault) · \(settingOn ? "On" : "Off")"
        }
    }

    /// What the menu offers it as.
    public func name(capitalised: Bool) -> String {
        switch self {
        case .followSettings: capitalised ? "Use Default" : "Use default"
        case .on: "On"
        case .off: "Off"
        }
    }
}

extension AppStore {
    /// Ask the engine for the week, which schedules it: soon, and once however many ask at once.
    public func refreshNotices() {
        noticeRefresh?.cancel()
        noticeRefresh = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(400))
            guard !Task.isCancelled else { return }
            self?.send(.upcomingNotices)
        }
    }

    /// Turn new-episode notifications on or off on this device. On asks for permission the
    /// first time, and stays off when it is declined.
    public func setEpisodeNotifications(_ on: Bool) async {
        if on, !(await notifications.permit()) { return }
        setSwitch(.episodeNotifications, on: on)
        refreshNotices()
    }

    /// A show's own switch. On asks for permission as the setting's does.
    public func setShowNotices(_ id: AnilistId, to choice: ShowNoticeChoice) async {
        if choice == .on, !(await notifications.permit()) { return }
        let value: SettingValue? = switch choice {
        case .followSettings: nil
        case .on: .bool(true)
        case .off: .bool(false)
        }
        send(.setShowSetting(id: id, key: .notifyNewEpisodes, value: value))
        refreshNotices()
    }

    /// A show's own settings, for its switch.
    public func loadShowNotices(_ id: AnilistId) {
        send(.loadShowSettings(id: id))
    }

    /// Questions for the engine this store holds, answered from the events it applies.
    public var questions: EngineQuestions {
        EngineQuestions(answers: answers) { [weak self] command in
            Task { @MainActor in self?.send(command) }
        }
    }
}
