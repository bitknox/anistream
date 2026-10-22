// New-episode notifications: what is scheduled and in what words, how the system is told to group
// them, where their actions take the app, and a background refresh on the testing engine. Nothing
// here reaches the system's notification centre: the centre is a quiet one throughout.

@testable import AnistreamStore
import AnistreamKit
import AnistreamWidgetData
import Foundation
import Testing
import UserNotifications

private let now = Date(timeIntervalSince1970: 1_800_000_000)
private let hour: Int64 = 3_600

private func airing(
    _ id: AnilistId, _ episode: UInt32, in seconds: Int64, _ reason: NoticeReason = .watching,
    next: UInt32? = nil, title: String? = nil, banner: String? = nil
) -> UpcomingNotice {
    UpcomingNotice(
        id: id, title: title ?? "Show \(id)", episode: episode,
        airsAt: Int64(now.timeIntervalSince1970) + seconds, reason: reason,
        nextToWatch: next ?? episode, coverUrl: "https://img.example/\(id).png", bannerUrl: banner)
}

private func week(_ airings: [UpcomingNotice], enabled: Bool = true, watching: [AnilistId] = []) -> UpcomingNotices {
    UpcomingNotices(
        enabled: enabled, generatedAt: Int64(now.timeIntervalSince1970), airings: airings,
        watching: watching, overrides: 0)
}

@Suite struct NoticeScheduling {
    @Test func theWeekNotifiesAtAirTimeSoonestFirst() {
        let planned = NoticePlan.schedule(week([
            airing(3, 2, in: 5 * hour),
            airing(1, 9, in: 2 * hour, banner: "https://img.example/1-banner.jpg"),
            // Aired already: the check's to say, not the schedule's.
            airing(2, 4, in: -hour),
        ]), now: now)
        #expect(planned.map(\.identifier) == ["ep.1.9.sub", "ep.3.2.sub"])
        let first = planned[0]
        #expect(first.fireAt == now.addingTimeInterval(TimeInterval(2 * hour)))
        #expect(first.title == "Show 1")
        #expect(first.body == "Episode 9 is out.")
        #expect(first.category == .newEpisode)
        #expect(first.playEpisode == 9)
        #expect(first.artURL == "https://img.example/1-banner.jpg", "the banner before the poster")
        #expect(planned[1].artURL == "https://img.example/3.png")
    }

    @Test func behindSaysWhereYouAreAndPlaysThat() {
        let planned = NoticePlan.schedule(week([airing(1, 10, in: hour, next: 8)]), now: now)
        #expect(planned.first?.body == "Episode 10 is out. Next for you: episode 8.")
        #expect(planned.first?.category == .behind)
        #expect(planned.first?.playEpisode == 8)
    }

    @Test func whatNotifiesFollowsTheSettingAndEachShowsSwitch() {
        let rows = [
            airing(1, 2, in: hour, .watching),
            airing(2, 3, in: hour, .switchedOn),
            airing(3, 4, in: hour, .switchedOff),
        ]
        #expect(NoticePlan.schedule(week(rows, enabled: true), now: now).map(\.show) == [1, 2])
        // Off, only a show switched on for itself notifies.
        #expect(NoticePlan.schedule(week(rows, enabled: false), now: now).map(\.show) == [2])
    }

    @Test func noMoreThanSixtyAndEachEpisodeOnce() {
        let rows = (0..<80).map { airing(AnilistId($0 + 1), 1, in: hour + Int64($0)) } + [airing(1, 1, in: 3 * hour)]
        let planned = NoticePlan.schedule(week(rows), now: now)
        #expect(planned.count == NoticePlan.limit)
        #expect(Set(planned.map(\.identifier)).count == planned.count)
        #expect(planned.first?.fireAt == now.addingTimeInterval(TimeInterval(hour)), "the soonest kept")
    }

    @Test func theCheckPostsOnlyWhatNoScheduledNotificationSaid() {
        let said = [
            NewEpisode(id: 1, title: "Show 1", episode: 9, airedAt: 1, coverUrl: nil),
            NewEpisode(id: 2, title: "Show 2", episode: 5, airedAt: 2, coverUrl: "https://img.example/2.png"),
            NewEpisode(id: 2, title: "Show 2", episode: 5, airedAt: 2, coverUrl: nil),
        ]
        let standing = week([airing(2, 6, in: hour, next: 3)])
        let posts = NoticePlan.catchUp(said, scheduled: ["ep.1.9.sub"], week: standing)
        #expect(posts.map(\.identifier) == ["ep.2.5.sub"])
        #expect(posts.first?.fireAt == nil, "now")
        #expect(posts.first?.body == "Episode 5 is out. Next for you: episode 3.")
        #expect(posts.first?.category == .behind)
    }
}

@Suite struct NoticeGrouping {
    let notice = PlannedNotice(
        identifier: "ep.7.9.sub", show: 7, episode: 9, playEpisode: 9, title: "Natsukaze Relay",
        body: "Episode 9 is out.", category: .newEpisode, fireAt: now, artURL: nil)

    @Test func everyOneIsInTheNewEpisodesThreadNamedByItsShow() {
        let content = SystemNoticeCentre.content(notice)
        #expect(content.threadIdentifier == "new-episodes")
        #expect(content.summaryArgument == "Natsukaze Relay")
        #expect(content.title == "Natsukaze Relay")
        #expect(content.body == "Episode 9 is out.")
        #expect(content.categoryIdentifier == "new-episode")
        #expect(content.userInfo["title"] as? Int == 7)
    }

    @Test func aGroupSaysHowManyMoreAndHiddenPreviewsSayNewEpisode() throws {
        let categories = SystemNoticeCentre.categories()
        let caughtUp = try #require(categories.first { $0.identifier == "new-episode" })
        let behind = try #require(categories.first { $0.identifier == "new-episode-behind" })
        for category in [caughtUp, behind] {
            #expect(category.categorySummaryFormat == "%u more new episodes")
            #expect(category.hiddenPreviewsBodyPlaceholder == "New episode")
        }
        #expect(caughtUp.actions.map(\.identifier) == ["play", "mark-watched"])
        #expect(behind.actions.map(\.identifier) == ["play"], "no Mark watched when behind")
        #expect(caughtUp.actions[0].options.contains(.foreground), "Play opens the app")
        #expect(!caughtUp.actions[1].options.contains(.foreground), "Mark watched does not")
    }

    @Test func itFiresAtTheSecondTheEpisodeAirs() throws {
        let request = SystemNoticeCentre.request(notice, content: SystemNoticeCentre.content(notice))
        let trigger = try #require(request.trigger as? UNCalendarNotificationTrigger)
        #expect(!trigger.repeats)
        #expect(Calendar.current.date(from: trigger.dateComponents) == now)
        #expect(request.identifier == "ep.7.9.sub")
        var now = notice
        now.fireAt = nil
        #expect(SystemNoticeCentre.request(now, content: SystemNoticeCentre.content(now)).trigger == nil)
    }

    @Test func theThumbnailIsTheMiddleSquare() {
        let wide = NoticePictures.square(width: 680, height: 240)
        #expect(abs(wide.width - 240.0 / 680.0) < 1e-9 && wide.height == 1)
        #expect(abs(wide.midX - 0.5) < 1e-9)
        let tall = NoticePictures.square(width: 230, height: 345)
        #expect(tall.width == 1 && abs(tall.midY - 0.5) < 1e-9)
    }
}

@MainActor
@Suite struct NoticeRouting {
    let info: [AnyHashable: Any] = ["title": 7, "episode": 10, "play": 8]

    @Test func eachActionGoesWhereItSays() {
        #expect(NoticeRoute(action: UNNotificationDefaultActionIdentifier, userInfo: info) == .open(7))
        #expect(NoticeRoute(action: "play", userInfo: info) == .play(7, episode: 8), "the episode Play starts")
        #expect(NoticeRoute(action: "mark-watched", userInfo: info) == .markWatched(7, episode: 10))
        #expect(NoticeRoute(action: UNNotificationDismissActionIdentifier, userInfo: info) == nil)
        #expect(NoticeRoute(action: "play", userInfo: ["something": "else"]) == nil, "not a new episode's")
        #expect(NoticeRoute(action: "play", userInfo: ["title": "7", "episode": NSNumber(value: 3)]) == .play(7, episode: 3))
    }

    @Test func theRecordAPlannedNoticeKeepsRoutesBack() {
        let notice = NoticePlan.schedule(week([airing(4, 10, in: hour, next: 8)]), now: now)[0]
        let info = NoticeRoute.userInfo(notice)
        #expect(NoticeRoute(action: "play", userInfo: info) == .play(4, episode: 8))
        #expect(NoticeRoute(action: "mark-watched", userInfo: info) == .markWatched(4, episode: 10))
        #expect(!NoticeRoute.markWatched(4, episode: 10).needsForeground)
        #expect(NoticeRoute.play(4, episode: 8).needsForeground)
    }

    @Test func theStoreAsksTheEngineForWhatTheActionsDo() {
        let store = AppStore()
        store.perform(.play(4, episode: 8))
        store.perform(.markWatched(4, episode: 10))
        store.perform(.open(4))
        #expect(store.sentLog == ["play", "setWatched"])
    }
}

/// A quiet centre that counts what it was asked to schedule.
private final class CountingCentre: NoticeCentre, @unchecked Sendable {
    let quiet: QuietNoticeCentre
    private let lock = NSLock()
    private var count = 0
    var scheduledCount: Int { lock.withLock { count } }

    init(permission: NoticePermission = .allowed, answer: Bool = true) {
        quiet = QuietNoticeCentre(permission: permission, answer: answer)
    }

    func permission() async -> NoticePermission { await quiet.permission() }
    func askPermission() async -> Bool { await quiet.askPermission() }
    func pending() async -> [String] { await quiet.pending() }
    func schedule(_ notice: PlannedNotice, picture: URL?) async throws {
        lock.withLock { count += 1 }
        try await quiet.schedule(notice, picture: picture)
    }
    func cancel(_ identifiers: [String]) async { await quiet.cancel(identifiers) }
    func registerCategories() {}
}

private func ledger() -> NoticeLedger {
    let suite = "anistream.tests.\(UUID().uuidString)"
    return NoticeLedger(defaults: UserDefaults(suiteName: suite)!, key: "notices")
}

@Suite struct NoticeSchedulerTests {
    @Test func reschedulingTakesBackWhatNoLongerNotifiesAndLeavesTheRest() async {
        let centre = CountingCentre()
        let scheduler = NoticeScheduler(centre: centre, ledger: ledger()) { _ in nil }
        await scheduler.reschedule(week([airing(1, 2, in: hour), airing(2, 3, in: 2 * hour)]), now: now)
        #expect(await centre.pending() == ["ep.1.2.sub", "ep.2.3.sub"])
        #expect(centre.scheduledCount == 2)

        // Show 2 switched off, show 1 unchanged, show 3 new: one taken back, one added.
        await scheduler.reschedule(week([
            airing(1, 2, in: hour), airing(2, 3, in: 2 * hour, .switchedOff), airing(3, 1, in: 3 * hour),
        ]), now: now)
        #expect(await centre.pending() == ["ep.1.2.sub", "ep.3.1.sub"])
        #expect(centre.scheduledCount == 3, "the unchanged one is not scheduled again")

        // Behind now on show 1: the same notification, new words, scheduled again in its place.
        await scheduler.reschedule(week([airing(1, 2, in: hour, next: 1), airing(3, 1, in: 3 * hour)]), now: now)
        #expect(centre.scheduledCount == 4)
        #expect(centre.quiet.notices.first { $0.show == 1 }?.category == .behind)
    }

    @Test func withoutPermissionNothingIsScheduledAndWhatWaitedIsTakenBack() async {
        let centre = CountingCentre(permission: .denied)
        try? await centre.quiet.schedule(NoticePlan.schedule(week([airing(1, 2, in: hour)]), now: now)[0], picture: nil)
        let scheduler = NoticeScheduler(centre: centre, ledger: ledger()) { _ in nil }
        #expect(await scheduler.reschedule(week([airing(1, 2, in: hour)]), now: now).isEmpty)
        #expect(await centre.pending().isEmpty)
    }

    @Test func theCheckNeverSaysWhatWasScheduled() async {
        let centre = CountingCentre()
        let scheduler = NoticeScheduler(centre: centre, ledger: ledger()) { _ in nil }
        await scheduler.reschedule(week([airing(1, 2, in: hour)]), now: now)
        let said = [
            NewEpisode(id: 1, title: "Show 1", episode: 2, airedAt: 0, coverUrl: nil),
            NewEpisode(id: 5, title: "Show 5", episode: 1, airedAt: 0, coverUrl: nil),
        ]
        let later = now.addingTimeInterval(TimeInterval(2 * hour))
        #expect(await scheduler.catchUp(said, week: nil, now: later).map(\.identifier) == ["ep.5.1.sub"])
        #expect(await scheduler.catchUp(said, week: nil, now: later).isEmpty, "nor twice")
    }

    @Test func aPictureIsStagedForTheCentreToTake() async throws {
        let staging = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: staging) }
        let jpeg = Data([0xFF, 0xD8, 0xFF, 0xE0, 0, 0])
        let file = try #require(NoticePictures.stage(jpeg, in: staging))
        #expect(file.pathExtension == "jpg")
        #expect(NoticePictures.stage(Data("not a picture".utf8), in: staging) == nil)
    }
}

@MainActor
@Suite struct NoticeSwitches {
    @Test func turningOnAsksOnceAndADeclineLeavesItOff() async {
        let store = AppStore()
        let declining = CountingCentre(permission: .notAsked, answer: false)
        store.notifications.scheduler = NoticeScheduler(centre: declining, ledger: ledger()) { _ in nil }
        await store.setEpisodeNotifications(true)
        #expect(declining.quiet.timesAsked == 1)
        #expect(store.notifications.declined)
        #expect(!store.sentLog.contains("setSetting"), "declined, the switch stays off")

        let store2 = AppStore()
        let allowing = CountingCentre(permission: .notAsked, answer: true)
        store2.notifications.scheduler = NoticeScheduler(centre: allowing, ledger: ledger()) { _ in nil }
        await store2.setEpisodeNotifications(true)
        await store2.setShowNotices(9, to: .on)
        #expect(allowing.quiet.timesAsked == 1, "asked the first time only")
        #expect(!store2.notifications.declined)
        #expect(store2.sentLog.filter { $0 == "setSetting" || $0 == "setShowSetting" } == ["setSetting", "setShowSetting"])
        // Turning off asks nothing.
        await store2.setShowNotices(9, to: .off)
        #expect(allowing.quiet.timesAsked == 1)
    }

    @Test func aShowNotBeingWatchedThatFollowsSettingsSaysOff() {
        #expect(ShowNoticeChoice.followSettings.label(settingOn: true, watching: false, capitalised: false) == "Off · not watching")
        #expect(ShowNoticeChoice.followSettings.label(settingOn: true, watching: true, capitalised: true) == "Use Default · On")
        #expect(ShowNoticeChoice.followSettings.label(settingOn: false, watching: true, capitalised: false) == "Use default · Off")
        #expect(ShowNoticeChoice.on.label(settingOn: false, watching: false, capitalised: false) == "On")
    }
}

/// A background refresh on the testing engine's Watching scene: signed in to the fake AniList,
/// two of its four titles airing. Nothing reaches the system: the centre is quiet.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct NoticeRefreshTests {
    @Test func aRefreshSchedulesTheWeekAndWritesTheWidgetsRecord() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let launch = EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host)
        let engine = try BackgroundEngine(launch, host: quietHost())
        defer { engine.shutdown() }
        engine.send(.setSetting(key: .episodeNotifications, value: .bool(true)))

        let centre = CountingCentre()
        let scheduler = NoticeScheduler(centre: centre, ledger: ledger()) { _ in nil }
        // The widgets' record goes through the store's own writer, the widgets package's, which
        // reloads the widgets once it is written.
        let files = WidgetFiles(directory: root.appending(path: "group/widgets", directoryHint: .isDirectory))
        let reloads = Reloads()
        let feed = WidgetFeed(files: files, reload: { reloads.count += 1 })
        let outcome = await NoticeRefresh(
            questions: engine.questions, scheduler: scheduler, widgets: feed.writer(art: engine.questions)
        ).run()

        #expect(outcome.finished)
        #expect(outcome.posted.isEmpty, "a first check only notes where things stand")
        // Kumo no Ue no Toshokan's episode 6, with episode 3 the last watched.
        let library = try #require(outcome.scheduled.first { $0.identifier == "ep.900002.6.sub" }, "\(outcome.scheduled)")
        #expect(library.title == "The Library Above the Clouds")
        #expect(library.body == "Episode 6 is out. Next for you: episode 4.")
        #expect(library.category == .behind)
        #expect(library.fireAt.map { $0 > .now } == true)
        #expect(centre.quiet.notices.map(\.identifier) == outcome.scheduled.map(\.identifier))

        #expect(outcome.wroteWidgets)
        #expect(reloads.count == 1, "the widgets read the record again")
        let snapshot = try #require(files.snapshot())
        #expect(snapshot.version == 2)
        #expect(snapshot.signedIn == true)
        #expect(snapshot.continueWatching.count == 4)
        #expect(snapshot.continueWatching.first?.title == "Stardust Wayfarer")
        #expect(snapshot.continueWatching.first?.minutesLeft == 14)
    }
}
