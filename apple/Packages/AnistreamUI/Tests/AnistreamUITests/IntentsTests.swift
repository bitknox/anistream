// What the App Intents do: the titles query Shortcuts and Siri name titles by (by id, by the
// words of their names, and as suggestions), Continue's hero, and today's broadcasts. The
// intents' own types are the app's, and call these.

@testable import AnistreamUI
import AnistreamKit
import AnistreamStore
import Foundation
import Testing

@MainActor
struct IntentsTests {
    // MARK: The titles query

    @Test func titlesAreFoundByTheirIds() {
        let query = bridge(SpotlightTests().watchingStore())
        let found = query.titles(for: [900_005, 900_001, 1])
        #expect(found.map(\.id) == [900_005, 900_001], "in the order asked, and none for a title nobody knows")
        #expect(found.last?.title == "Hoshikuzu no Tabibito")
        #expect(found.last?.line == "Continue · Episode 7 of 24", "what Shortcuts shows beneath it")
    }

    @Test func aTitleThatLeftTheShelfIsStillFoundByItsId() {
        let store = SpotlightTests().watchingStore()
        store.apply(.detail(shelved(900_009, "The Clockmaker's Daughter")))
        let query = bridge(store)
        #expect(query.titles(for: [900_009]).map(\.title) == ["The Clockmaker's Daughter"])
        #expect(query.titles(matching: "clockmaker").isEmpty, "found by its words only while it is on your lists")
    }

    @Test func titlesAreFoundByTheWordsOfAnyOfTheirNames() {
        let query = bridge(SpotlightTests().watchingStore())
        func found(_ words: String) -> [AnilistId] { query.titles(matching: words).map(\.id) }
        #expect(found("hoshikuzu") == [900_001])
        #expect(found("WAYFARER") == [900_001], "the other-language title")
        #expect(found("星屑") == [900_001], "the native title")
        #expect(found("tabibito stardust") == [900_001], "words from two of its names")
        #expect(found("the") == [900_005, 900_002], "in the shelf's order")
        #expect(found("adventure") == [900_001], "a genre")
        #expect(found("   ").isEmpty)
        #expect(found("hoshikuzu gardener").isEmpty, "every word, in one title")
    }

    @Test func accentsCaseAndWidthDoNotCount() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([shelved(900_010, "Café Lumière")])))
        let query = bridge(store)
        #expect(query.titles(matching: "cafe lumiere").map(\.id) == [900_010])
        #expect(query.titles(matching: "ＣＡＦＥ").map(\.id) == [900_010])
    }

    @Test func theSuggestionsAreTheShelfContinuesFirst() {
        let query = bridge(SpotlightTests().watchingStore())
        #expect(query.suggestedTitles().map(\.id) == [900_001, 900_005, 900_002, 900_003])
    }

    @Test func withSpotlightOffNothingIsSuggestedButATitleIsStillFound() throws {
        let suite = "anistream.tests.intents.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set(false, forKey: SpotlightIndexer.preference)
        let off = IntentsBridge(defaults: defaults)
        off.store = SpotlightTests().watchingStore()
        #expect(off.suggestedTitles().isEmpty)
        #expect(off.titles(matching: "hoshikuzu").map(\.id) == [900_001])
    }

    @Test func beforeTheEngineHasAnsweredTheSavedShelfIsUsed() throws {
        let file = SpotlightTests().temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        try Shelf.current(SpotlightTests().watchingStore(), over: Shelf()).save(file)

        // A launch to answer the query: the engine has said nothing yet, or is not there.
        for store in [AppStore(), nil] {
            let early = bridge(store)
            early.shelfFile = file
            #expect(early.suggestedTitles().map(\.id) == [900_001, 900_005, 900_002, 900_003])
            #expect(early.titles(matching: "moonlit").map(\.id) == [900_003])
            #expect(early.titles(for: [900_002]).map(\.title) == ["The Library Above the Clouds"])
        }
    }

    // MARK: Opening

    @Test func openingATitleOpensItsPage() {
        let opening = bridge(AppStore())
        opening.open(900_003)
        #expect(opening.navigator?.titleOnScreenForTests == 900_003)
    }

    @Test func continueWatchingPlaysTheHerosEpisode() async throws {
        let store = AppStore()
        // A new episode out, and a title under way, which comes first.
        store.apply([
            .content(view: .continue, content: .entries([
                shelved(900_006, "Summer Wind Relay") { $0.progress = TitleProgress(done: 7, next: 8) },
                shelved(900_005, "The Steel Gardener") {
                    $0.progress = TitleProgress(done: 1, next: 2)
                    $0.resume = ResumePoint(position: 810, fraction: 0.75)
                },
            ])),
            .lastAired([TitleAired(id: 900_006, aired: Aired(episode: 8, ago: 3600))]),
        ])
        let continuing = bridge(store)
        let answer = await continuing.continueWatching()
        #expect(answer == .play(900_005, episode: 2, title: "The Steel Gardener"))
        #expect(answer.spoken == "Starting episode 2 of The Steel Gardener.")

        continuing.navigator?.open(.library)
        let mark = store.sentLog.count
        continuing.play(900_005, episode: 2)
        #expect(Array(store.sentLog.dropFirst(mark)) == ["play"], "from where it was left, as the engine resumes it")
        #expect(continuing.navigator?.section == .continue)
    }

    @Test func caughtUpItSaysWhenTheNextEpisodeAirs() async throws {
        let now = Date.now
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([
            shelved(900_006, "Summer Wind Relay") {
                $0.progress = TitleProgress(done: 8, next: 9)
                $0.nextEpisode = 9
                $0.nextAiringIn = 2 * 86_400
                $0.airingIn = 2 * 86_400
            },
        ])))
        let answer = await bridge(store).continueWatching(now: now)
        let airs = EpisodeAvailability.upcoming(inSeconds: 2 * 86_400).spoken(now: now)
        #expect(answer == .caughtUp(title: "Summer Wind Relay", episode: 9, airs: airs))
        #expect(answer.spoken == "You're caught up. Summer Wind Relay episode 9 airs \(airs!).")
        #expect(ContinueAnswer.caughtUp(title: "Summer Wind Relay", episode: 9, airs: nil).spoken
            == "You're caught up. Summer Wind Relay episode 9 is not out yet.")
    }

    @Test func withNothingToContinueItSaysSo() async {
        let empty = AppStore()
        empty.apply(.content(view: .continue, content: .empty))
        let answer = await bridge(empty).continueWatching()
        #expect(answer == .nothing)
        #expect(answer.spoken == "Nothing to continue yet.")
    }

    @Test func beforeTheEngineHasAnsweredContinuePlaysTheSavedHero() async throws {
        let file = SpotlightTests().temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        try Shelf.current(SpotlightTests().watchingStore(), over: Shelf()).save(file)
        let early = bridge(AppStore())
        early.shelfFile = file
        #expect(await early.continueWatching() == .play(900_001, episode: 7, title: "Hoshikuzu no Tabibito"))
    }

    @Test func whenAnEpisodeAirsIsSaidWithAt() throws {
        let calendar = Calendar.current
        let noon = try #require(calendar.date(bySettingHour: 12, minute: 0, second: 0, of: .now))
        func said(_ seconds: Int64) -> String? { EpisodeAvailability.upcoming(inSeconds: seconds).spoken(now: noon) }
        let later = noon.addingTimeInterval(4.5 * 3600)
        #expect(said(Int64(4.5 * 3600)) == "today at \(AirTime.clock(later))")
        let inThreeDays = noon.addingTimeInterval(3 * 86_400)
        let weekday = inThreeDays.formatted(Date.FormatStyle(locale: AirTime.locale).weekday(.wide))
        #expect(said(3 * 86_400) == "\(weekday) at \(AirTime.clock(inThreeDays))")
        let far = noon.addingTimeInterval(20 * 86_400)
        let day = far.formatted(Date.FormatStyle(locale: AirTime.locale).day().month(.abbreviated))
        #expect(said(20 * 86_400) == "\(day) at \(AirTime.clock(far))")
        #expect(EpisodeAvailability.upcoming(inSeconds: nil).spoken(now: noon) == nil)
        #expect(EpisodeAvailability.aired.spoken(now: noon) == nil)
    }

    // MARK: Airing today

    @Test func todaysBroadcastsAreListedEarliestFirstWithTheirTimes() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "Europe/Copenhagen"))
        let now = try #require(calendar.date(from: DateComponents(year: 2026, month: 10, day: 21, hour: 19, minute: 30)))
        func at(_ hour: Int, _ minute: Int, day: Int = 21) -> Date {
            calendar.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour, minute: minute))!
        }
        let wayfarer = SpotlightTests().wayfarer()
        let relay = shelved(900_006, "Summer Wind Relay")
        let rows = TodaysBroadcasts.rows([
            Broadcast(entry: relay, episode: 9, airs: at(22, 0)),
            Broadcast(entry: wayfarer, episode: 8, airs: at(16, 30)),
            Broadcast(entry: wayfarer, episode: 9, airs: at(16, 30, day: 22)),
            Broadcast(entry: relay, episode: 8, airs: at(23, 0, day: 20)),
        ], on: now, calendar: calendar)
        #expect(rows.map(\.title) == ["Hoshikuzu no Tabibito", "Summer Wind Relay"], "every one airing")
        #expect(rows.map(\.episode) == [8, 9])
        #expect(rows.map(\.aired) == [true, false])
        #expect(rows.map(\.marked) == [true, false], "under way here, so yours; the other is not on your lists")
        #expect(rows.first?.time == AirTime.clock(at(16, 30)))
    }

    @Test func siriReadsYourShowsTimes() {
        func row(_ title: String, _ episode: UInt32, _ time: String, marked: Bool) -> TodaysBroadcasts.Row {
            TodaysBroadcasts.Row(time: time, title: title, episode: episode, aired: false, marked: marked)
        }
        let wayfarer = row("Hoshikuzu no Tabibito", 8, "16:30", marked: true)
        let relay = row("Natsukaze Relay", 9, "22:00", marked: true)
        let other = row("Kanade Ropeway", 3, "19:00", marked: false)
        #expect(TodaysBroadcasts.summary([wayfarer]) == "Today: Hoshikuzu no Tabibito episode 8 at 16:30.")
        #expect(TodaysBroadcasts.summary([wayfarer, other, relay])
            == "Today: Hoshikuzu no Tabibito episode 8 at 16:30 and Natsukaze Relay episode 9 at 22:00.")
        #expect(TodaysBroadcasts.summary([other]) == "None of your shows airs today. 1 other title does.")
        #expect(TodaysBroadcasts.summary([other, other]) == "None of your shows airs today. 2 other titles do.")
        #expect(TodaysBroadcasts.summary([]) == "Nothing airing today.")
    }

    @Test func withNoEngineTheScheduleCannotBeHad() async {
        let airing = bridge(AppStore())
        #expect(await airing.airingToday() == nil)
    }

    // MARK: Support

    func bridge(_ store: AppStore?) -> IntentsBridge {
        let bridge = IntentsBridge(defaults: SpotlightTests.defaults)
        bridge.store = store
        bridge.navigator = Navigator(route: LaunchRoute())
        bridge.patience = .zero
        return bridge
    }
}

/// The intents over the testing engine, which the app starts for an intent that runs first.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct IntentsEngineTests {
    @Test func anIntentStartsTheEngineAndReadsTodaysSchedule() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "anistream-intents-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        defer { store.shutdown() }
        let bridge = IntentsBridge(defaults: SpotlightTests.defaults)
        bridge.store = store
        bridge.navigator = Navigator(route: LaunchRoute())
        let started = Counter()
        bridge.start = {
            started.count += 1
            let launch = EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host)
            Task { await store.start(launch, host: AppHost(secrets: MemorySecrets())) }
        }

        // One moment for both lists: read twice, a run across midnight compared two days.
        let now = Date.now
        let rows = try #require(await bridge.airingToday(now: now))
        #expect(started.count == 1)
        #expect(store.calendar.list.state.isAnswered)
        let today = TodaysBroadcasts.rows(store.broadcasts(mine: false), on: now)
        #expect(rows == today)
        _ = try #require(await bridge.airingToday())
        #expect(started.count == 1, "started once")
    }
}

@MainActor
final class Counter {
    var count = 0
}

extension Navigator {
    /// The title page on screen, on the Mac or the top of the tab's stack.
    var titleOnScreenForTests: AnilistId? {
        #if os(macOS)
        titleOnMac
        #else
        paths[section]?.last
        #endif
    }
}
