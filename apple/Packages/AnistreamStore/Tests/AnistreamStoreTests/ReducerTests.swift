// The store fed scripted events, as the engine would send them, and what each model then holds.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
struct ReducerTests {
    /// The four titles of the Watching scene, as Continue's list brings them.
    func watching() -> [Entry] {
        [
            entry(900_001, "Stardust Wayfarer") {
                $0.secondary = "Hoshikuzu no Tabibito"
                $0.episodes = 12
                $0.progress = TitleProgress(done: 6, next: 7)
                $0.resume = ResumePoint(position: 606, fraction: 606.0 / 1440)
            },
            entry(900_005, "The Steel Gardener") {
                $0.progress = TitleProgress(done: 1, next: 2)
                $0.resume = ResumePoint(position: 810, fraction: 0.75)
            },
            entry(900_002, "The Library Above the Clouds") {
                $0.progress = TitleProgress(done: 3, next: 4)
                $0.airingIn = 184_500
                $0.nextEpisode = 6
            },
            entry(900_006, "Summer Wind Relay") {
                $0.progress = TitleProgress(done: 7, next: 8)
                $0.airingIn = 536_400
                $0.nextEpisode = 9
            },
        ]
    }

    @Test func continueGroupsTheQueueAndSaysWhatEachRowPlays() {
        let store = AppStore()
        store.apply([
            .content(view: .continue, content: .entries(watching())),
            // The broadcast line follows the list: episode 8 out 19 hours ago, episode 5 of the
            // library title five days ago.
            .lastAired([
                TitleAired(id: 900_006, aired: Aired(episode: 8, ago: 19 * 3600)),
                TitleAired(id: 900_002, aired: Aired(episode: 5, ago: 5 * 86_400)),
            ]),
        ])

        let queue = store.continuing.queue(store.titles)
        #expect(queue.inProgress.map(\.id) == [900_001, 900_005, 900_002])
        #expect(queue.inProgress.map(\.action) == [
            "Resume E7 · 10:06", "Resume E2 · 13:30", "Start E4",
        ])
        #expect(queue.newThisWeek.map(\.action) == ["Start E8 · out 19h ago"])
        let first = try! #require(queue.inProgress.first)
        #expect(first.buttonTitle == "Resume Episode 7 · 10:06")
        #expect(first.minutesLeft == 14)
        #expect(queue.inProgress[2].buttonTitle == "Start Episode 4")
    }

    @Test func anEpisodeOutMoreThanAWeekAgoIsNotNew() {
        let store = AppStore()
        store.apply([
            .content(view: .continue, content: .entries(watching())),
            .lastAired([TitleAired(id: 900_006, aired: Aired(episode: 8, ago: 8 * 86_400))]),
        ])
        let queue = store.continuing.queue(store.titles)
        #expect(queue.newThisWeek.isEmpty)
        #expect(queue.inProgress.last?.action == "Start E8")
    }

    @Test func continueKeepsToFiveRows() {
        let store = AppStore()
        let many = (0..<8).map { n in
            entry(AnilistId(900_100 + n), "Title \(n)") { $0.progress = TitleProgress(done: 1, next: 2) }
        }
        store.apply(.content(view: .continue, content: .entries(many)))
        #expect(store.continuing.queue(store.titles).all.count == 5)
    }

    @Test func aTitleHiddenFromContinueGoesAtOnceAndComesBackWithTheNextList() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries(watching())))
        store.hideFromContinue(900_005)
        #expect(!store.continuing.queue(store.titles).all.contains { $0.id == 900_005 })
        store.apply(.content(view: .continue, content: .entries(watching())))
        #expect(store.continuing.queue(store.titles).all.contains { $0.id == 900_005 })
    }

    @Test func nothingWatchedYetIsEmptyRatherThanAnEmptyList() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .empty))
        #expect(store.continuing.list.state == .empty)
        store.apply(.content(view: .continue, content: .failed("no disk")))
        #expect(store.continuing.list.state == .failed("no disk"))
    }

    @Test func progressMovesATitleEverywhereItShows() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries(watching())))
        store.apply(.content(view: .seasonal, content: .entries([watching()[2]])))
        store.apply(.progress(
            id: 900_002,
            progress: TitleProgress(done: 4, next: 5),
            resume: ResumePoint(position: 300, fraction: 0.2),
            episodes: []
        ))
        #expect(store.titles[900_002]?.progress == TitleProgress(done: 4, next: 5))
        let row = store.continuing.queue(store.titles).inProgress.first { $0.id == 900_002 }
        #expect(row?.action == "Resume E5 · 5:00")
        #expect(store.seasonal.state == .loaded([900_002]))
    }

    @Test func aListKeepsWhatTheTitlesPageBrought() {
        let store = AppStore()
        store.apply(.detail(entry(900_001, "Stardust Wayfarer") {
            $0.availableOn = ["Lantern Stream"]
            $0.trailerUrl = "https://video.example/trailer"
        }))
        store.apply(.content(view: .seasonal, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        #expect(store.titles[900_001]?.availableOn == ["Lantern Stream"])
        #expect(store.titles[900_001]?.trailerUrl == "https://video.example/trailer")
    }

    @Test func aSlowAnswerToAnEarlierSearchIsNotShown() {
        let store = AppStore()
        store.search("hoshi")
        store.search("hoshikuzu")
        store.apply(.content(
            view: .search(query: "hoshi", filter: SearchFilters()),
            content: .entries([entry(900_003, "Moonlit Platform")])
        ))
        #expect(store.search.list.state == .loading)
        store.apply(.content(
            view: .search(query: "hoshikuzu", filter: SearchFilters()),
            content: .entries([entry(900_001, "Stardust Wayfarer")])
        ))
        #expect(store.search.list.state == .loaded([900_001]))
    }

    @Test func eachListTakesOnlyItsOwnAnswerAndCaption() {
        let store = AppStore()
        store.apply([
            .caption(view: .seasonal, text: "autumn 2026"),
            .content(view: .seasonal, content: .entries([entry(900_002, "The Library Above the Clouds")])),
            .content(view: .library(.planning), content: .entries([entry(900_003, "Moonlit Platform")])),
            .calendarWindow(anchor: 1_800_000_000, pages: 1, morePast: true),
            .content(view: .calendar, content: .entries([entry(900_006, "Summer Wind Relay")])),
            .calendarBackfill(page: 1, entries: [entry(900_002, "The Library Above the Clouds")], morePast: false),
        ])
        #expect(store.seasonal.caption == "autumn 2026")
        #expect(store.seasonal.state == .loaded([900_002]))
        #expect(store.library.segment(.planning).state == .loaded([900_003]))
        #expect(store.library.segment(.watching).state == .idle)
        #expect(store.calendar.rows.map(\.id) == [900_002, 900_006])
        #expect(store.calendar.pages == 2 && !store.calendar.morePast)
    }

    @Test func noticesAreKeptNewestFirstAndTheCapsuleShowsTheLatest() {
        let store = AppStore()
        store.apply([
            .toast(Notice(kind: .info, text: "plugin update ready: alpha", hint: .openStores)),
            .toast(Notice(kind: .alert, text: "AniList is not answering", hint: nil)),
            .status("syncing"),
        ])
        #expect(store.notices.log.map(\.notice.text) == [
            "AniList is not answering", "plugin update ready: alpha",
        ])
        let current = try! #require(store.notices.current)
        #expect(current.isAlert)
        store.notices.dismiss(current)
        #expect(store.notices.current == nil)
        #expect(store.notices.status == "syncing")
    }

    @Test func theSyncCapsuleSaysWhereSyncStands() {
        let store = AppStore()
        #expect(store.accounts.summary.text == "Not connected")
        var anilist = SyncState(
            tracker: "anilist", connected: true, user: "Mika", storage: "keychain",
            storageDegraded: false, outbox: 0, deferred: 0, needsReauth: false, last: nil,
            pullError: nil
        )
        store.apply(.sync(anilist))
        #expect(store.accounts.summary == SyncSummary(text: "AniList synced", state: .synced, tracker: "anilist"))
        store.apply([.progressQueued, .progressQueued])
        #expect(store.accounts.summary.text == "2 changes waiting to sync")
        anilist.needsReauth = true
        store.apply(.sync(anilist))
        #expect(store.accounts.summary.text == "Sign in to AniList again")
    }
}
