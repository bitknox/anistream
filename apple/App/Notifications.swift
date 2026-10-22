// New-episode notifications as the app runs them: the centre they go to, the delegate their
// actions come back through, and the background refresh that keeps the week scheduled and the
// widgets' record written: iOS's app refresh task, and the Mac's background activity.
//
// The Fixtures app never reaches the system's centre on the Mac: its notifications go to a quiet
// one that shows nothing and asks nothing. On iOS it uses the system's only when launched with
// -AnistreamRealNotifications, whose notifications stay inside the Simulator.

import AnistreamKit
import AnistreamStore
import AnistreamUI
import Foundation
import UserNotifications
import WidgetKit
#if os(iOS)
import BackgroundTasks
import UIKit
#endif

@MainActor
final class AppNotifications: NSObject {
    /// The app refresh task's identifier, which Info.plist permits.
    static let refreshTask = "tv.anistream.refresh"
    /// How long after the app leaves that the next refresh is asked for. The system decides.
    static let refreshEvery: TimeInterval = 6 * 3_600
    /// The launch argument that lets the Fixtures app reach the system's centre, on iOS only.
    static let realArgument = "-AnistreamRealNotifications"

    private let store: AppStore
    private let navigator: Navigator
    /// Whether the store's engine was started, or is being: a background launch has none.
    private let storeStarted: @MainActor () -> Bool
    /// The engine a background launch starts for itself.
    private let backgroundLaunch: @MainActor () -> (EngineLaunch, AppHost)?
    /// Whether notifications reach the system's centre.
    let real: Bool
    let centre: any NoticeCentre
    /// A refresh or a mark running on an engine of its own, which the app's start waits out.
    private(set) var backgroundRun: Task<Bool, Never>?
    #if os(iOS)
    private var registered = false
    #else
    private var activity: NSBackgroundActivityScheduler?
    #endif

    init(
        store: AppStore,
        navigator: Navigator,
        storeStarted: @escaping @MainActor () -> Bool,
        backgroundLaunch: @escaping @MainActor () -> (EngineLaunch, AppHost)?,
        arguments: [String] = ProcessInfo.processInfo.arguments
    ) {
        self.store = store
        self.navigator = navigator
        self.storeStarted = storeStarted
        self.backgroundLaunch = backgroundLaunch
        #if FIXTURES
        #if os(iOS)
        real = arguments.contains(Self.realArgument)
        #else
        real = false
        #endif
        #else
        real = true
        #endif
        centre = real ? SystemNoticeCentre() : QuietNoticeCentre()
        super.init()
    }

    /// At launch, before it finishes: the delegate and the actions, the store's scheduler, and
    /// the background refresh, which is kept scheduled while anything notifies (Settings' switch
    /// on, or a show switched on for itself) or any of the widgets is placed, and cancelled once
    /// neither holds.
    func launch() {
        if real {
            UNUserNotificationCenter.current().delegate = self
            centre.registerCategories()
        }
        let questions = store.questions
        store.notifications.scheduler = NoticeScheduler(centre: centre) { await questions.art($0) }
        store.notifications.onRefreshes = { [weak self] wanted in self?.keepRefreshing(wanted) }
        #if os(iOS)
        registerRefresh()
        #endif
        lookAtWidgets()
    }

    /// Schedule the background refresh while anything notifies or a widget is placed, and cancel
    /// it once neither holds.
    private func keepRefreshing(_ wanted: Bool) {
        #if os(iOS)
        if wanted { submitRefresh() } else { cancelRefresh() }
        #else
        if wanted { startActivity() } else { stopActivity() }
        #endif
    }

    /// The app came forward: ask again whether it may notify, schedule the week, and look at the
    /// widgets placed meanwhile.
    func cameForward() {
        Task {
            await store.notifications.refreshPermission()
            if store.notifications.schedules { store.refreshNotices() }
        }
        lookAtWidgets()
    }

    /// Look at which widgets are placed: while any of the app's is, the background refresh keeps
    /// their record fresh, whether anything notifies or not. Nothing changes when WidgetKit cannot
    /// say.
    private func lookAtWidgets() {
        WidgetCenter.shared.getCurrentConfigurations { [weak self] result in
            guard case .success(let placed) = result else { return }
            let kinds = placed.map(\.kind)
            Task { @MainActor in self?.store.notifications.widgetsPlaced(kinds) }
        }
    }

    /// The app left: ask for the next refresh, while anything notifies or a widget is placed.
    func wentAway() {
        #if os(iOS)
        if store.notifications.refreshes == true { submitRefresh() }
        #endif
    }

    /// The app is starting its own engine: stop whatever ran on one of its own, and wait for it.
    func yieldBackgroundEngine() async {
        guard let run = backgroundRun else { return }
        run.cancel()
        _ = await run.value
    }

    // MARK: The refresh

    /// One refresh: what aired that no notification said, the week, and the widgets' record; with
    /// nothing notifying, the widgets' record alone. On the store's engine when the app has one,
    /// or on one started for it, with none of the launch's own background work, whose own check
    /// says nothing when nothing notifies. The widgets placed are looked at again after it. True
    /// when it ran to the end.
    func refresh() async -> Bool {
        defer { lookAtWidgets() }
        let widgets = WidgetRecordFile.shared()
        if storeStarted() {
            // An engine that failed to start, or stopped, answers nothing.
            guard store.phase == .running || store.phase == .starting else { return false }
            let outcome = await NoticeRefresh(
                questions: store.questions, scheduler: store.notifications.scheduler, widgets: widgets,
                notifying: store.notifications.notifying != false
            ).run()
            if outcome.wroteWidgets { WidgetCenter.shared.reloadAllTimelines() }
            return outcome.finished
        }
        return await onOwnEngine { engine in
            let scheduler = NoticeScheduler(centre: self.centre) { await engine.questions.art($0) }
            let outcome = await NoticeRefresh(questions: engine.questions, scheduler: scheduler, widgets: widgets).run()
            if outcome.wroteWidgets { WidgetCenter.shared.reloadAllTimelines() }
            return outcome.finished
        }
    }

    /// Run `work` on an engine started for it, and shut it down after.
    private func onOwnEngine(_ work: @escaping @MainActor (BackgroundEngine) async -> Bool) async -> Bool {
        guard let (launch, host) = backgroundLaunch() else { return false }
        let run = Task { @MainActor () -> Bool in
            let started = await Task.detached(priority: .utility) { () -> BackgroundEngine? in
                do {
                    return try BackgroundEngine(launch, host: host)
                } catch {
                    AppLog.error("notices", "no engine for the background: \(error.localizedDescription)")
                    return nil
                }
            }.value
            guard let engine = started else { return false }
            let done = await work(engine)
            await Task.detached { engine.shutdown() }.value
            return done
        }
        backgroundRun = run
        let done = await run.value
        if backgroundRun == run { backgroundRun = nil }
        return done
    }

    // MARK: Where an action goes

    func route(_ route: NoticeRoute) async {
        switch route {
        case .open(let id):
            store.loadDetail(id)
            navigator.openTitle(id)
        case .play:
            store.perform(route)
        case .markWatched(let id, let episode):
            if storeStarted() {
                store.perform(route)
            } else {
                _ = await onOwnEngine { engine in
                    engine.send(.setWatched(id: id, episodes: [String(episode)], watched: true))
                    return true
                }
            }
        }
    }

    #if os(iOS)
    // MARK: iOS's app refresh task

    private func registerRefresh() {
        let permitted = Bundle.main.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String]
        guard permitted?.contains(Self.refreshTask) == true else { return }
        registered = BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshTask, using: .main) { task in
            let task = SendableTask(task)
            MainActor.assumeIsolated { self.handle(task) }
        }
    }

    private func cancelRefresh() {
        guard registered else { return }
        BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: Self.refreshTask)
    }

    private func submitRefresh() {
        guard registered else { return }
        let request = BGAppRefreshTaskRequest(identifier: Self.refreshTask)
        request.earliestBeginDate = Date(timeIntervalSinceNow: Self.refreshEvery)
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch {
            AppLog.error("notices", "could not ask for a refresh: \(error.localizedDescription)")
        }
    }

    private func handle(_ task: SendableTask) {
        // Asked for while something notified or a widget was placed. The app's settings and the
        // widgets say whether that still holds; with neither known yet, it is asked for again, and
        // the app cancels it when it comes forward.
        if store.notifications.refreshes != false { submitRefresh() }
        let run = Task { @MainActor in await self.refresh() }
        task.task.expirationHandler = { run.cancel() }
        Task { @MainActor in
            let done = await run.value
            task.task.setTaskCompleted(success: done)
        }
    }
    #else
    // MARK: The Mac's background activity

    private func startActivity() {
        guard self.activity == nil else { return }
        let activity = NSBackgroundActivityScheduler(identifier: Self.refreshTask)
        activity.repeats = true
        activity.interval = Self.refreshEvery
        activity.tolerance = 3_600
        activity.qualityOfService = .utility
        activity.schedule { [weak self] completion in
            Task { @MainActor in
                _ = await self?.refresh()
                completion(.finished)
            }
        }
        self.activity = activity
    }

    private func stopActivity() {
        activity?.invalidate()
        activity = nil
    }
    #endif
}

extension AppNotifications: UNUserNotificationCenterDelegate {
    /// While the app is in front, a new episode still shows: in the list, and as a banner.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter, willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .list, .sound]
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse
    ) async {
        guard let route = NoticeRoute(
            action: response.actionIdentifier, userInfo: response.notification.request.content.userInfo)
        else { return }
        await self.route(route)
    }
}

#if os(iOS)
/// A background task, handed from the scheduler's queue to the main actor, where it is used.
private struct SendableTask: @unchecked Sendable {
    let task: BGTask
    init(_ task: BGTask) { self.task = task }
}
#endif
