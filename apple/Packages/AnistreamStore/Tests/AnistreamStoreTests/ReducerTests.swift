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
            "Resume E7 · 10:06", "Resume E2 · 13:30", "Play E4",
        ])
        #expect(queue.newThisWeek.map(\.action) == ["Play E8 · out 19 h ago"])
        let first = try! #require(queue.inProgress.first)
        #expect(first.buttonTitle == "Resume episode 7 · 10:06")
        #expect(first.minutesLeft == 14)
        #expect(queue.inProgress[2].buttonTitle == "Play episode 4")
    }

    @Test func aShowWatchedDubbedIsMarkedAndASubIsNot() {
        let rows = watching().map { row in
            var row = row
            if row.id == 900_005 || row.id == 900_006 { row.watchedIn = .dub }
            if row.id == 900_002 { row.watchedIn = .sub }
            return row
        }
        let store = AppStore()
        store.apply([
            .content(view: .continue, content: .entries(rows)),
            .lastAired([TitleAired(id: 900_006, aired: Aired(episode: 8, ago: 19 * 3600))]),
        ])
        let queue = store.continuing.queue(store.titles)
        #expect(queue.inProgress.map(\.action) == [
            "Resume E7 · 10:06", "Resume E2 dubbed · 13:30", "Play E4",
        ])
        #expect(queue.inProgress.map(\.buttonTitle) == [
            "Resume episode 7 · 10:06", "Resume episode 2 · Dubbed · 13:30", "Play episode 4",
        ])
        #expect(queue.newThisWeek.map(\.action) == ["Play E8 dubbed · out 19 h ago"])
        #expect(queue.newThisWeek.map(\.buttonTitle) == ["Play episode 8 · Dubbed"])
        #expect(queue.all.map(\.dubbed) == [false, true, false, true])
    }

    @Test func aDubCaughtUpIsNotMarkedAsTheEpisodeIsStillToAir() {
        var row = watching()[3]
        row.watchedIn = .dub
        row.progress = TitleProgress(done: 8, next: 9)
        let item = ContinueQueue.item(row)
        #expect(!item.playable)
        #expect(item.action.hasPrefix("Caught up"), "\(item.action)")
        #expect(!item.action.contains("Dub"))
        #expect(!item.buttonTitle.contains("Dub"))
    }

    @Test func anEpisodeOutMoreThanAWeekAgoIsNotNew() {
        let store = AppStore()
        store.apply([
            .content(view: .continue, content: .entries(watching())),
            .lastAired([TitleAired(id: 900_006, aired: Aired(episode: 8, ago: 8 * 86_400))]),
        ])
        let queue = store.continuing.queue(store.titles)
        #expect(queue.newThisWeek.isEmpty)
        #expect(queue.inProgress.last?.action == "Play E8")
    }

    @Test func stillsAreAskedForOnceAndOnlyForATitleWithNoBanner() {
        let store = AppStore()
        store.apply(.detail(entry(900_005, "The Steel Gardener")))
        store.apply(.detail(entry(900_001, "Stardust Wayfarer") { $0.bannerUrl = "banner/900001.jpg" }))
        #expect(store.loadStills(900_005))
        #expect(!store.loadStills(900_005), "once a session")
        #expect(!store.loadStills(900_001), "a banner needs no stills")
        store.apply(.detail(entry(900_006, "Summer Wind Relay")))
        store.apply(.episodes(id: 900_006, rows: []))
        #expect(!store.loadStills(900_006), "the episodes are here already")
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

    @Test func aFailedRefreshKeepsTheRowsOnScreen() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries(watching())))
        let before = store.continuing.list.state
        store.apply(.content(view: .continue, content: .failed("no network")))
        #expect(store.continuing.list.state == before, "the loaded list stays")
        #expect(store.continuing.list.state.ids.count == 4)
    }

    @Test func aTrackersLibraryRowDoesNotMoveContinueBackToEpisodeOne() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries(watching())))
        // The tracker says the title is completed, 12 of 12; this device stopped at episode 7.
        var tracked = watching()[0]
        tracked.progress = TitleProgress(done: 12, next: 1)
        tracked.resume = nil
        store.apply(.content(view: .library(.completed), content: .entries([tracked])))
        #expect(store.titles[900_001]?.progress == TitleProgress(done: 6, next: 7))
        #expect(store.titles[900_001]?.resume?.position == 606)
        let row = store.continuing.queue(store.titles).inProgress.first { $0.id == 900_001 }
        #expect(row?.action == "Resume E7 · 10:06")
        // Continue's own list still updates it.
        var moved = watching()[0]
        moved.progress = TitleProgress(done: 7, next: 8)
        store.apply(.content(view: .continue, content: .entries([moved])))
        #expect(store.titles[900_001]?.progress == TitleProgress(done: 7, next: 8))
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
        store.search(SearchQuery(text: "hoshi"))
        store.search(SearchQuery(text: "hoshikuzu"))
        store.apply(.content(
            view: .search(query: SearchQuery(text: "hoshi")),
            content: .entries([entry(900_003, "Moonlit Platform")])
        ))
        #expect(store.search.list.state == .loading)
        store.apply(.content(
            view: .search(query: SearchQuery(text: "hoshikuzu")),
            content: .entries([entry(900_001, "Stardust Wayfarer")])
        ))
        #expect(store.search.list.state == .loaded([900_001]))
    }

    @Test func searchTakesTheAnswerToItsTextWithoutTheSpacesAround() {
        let store = AppStore()
        store.searchLive(SearchQuery(text: " hoshi "))
        store.apply(.content(
            view: .search(query: SearchQuery(text: "hoshi")),
            content: .entries([entry(900_001, "Stardust Wayfarer")])
        ))
        #expect(store.search.list.state == .loaded([900_001]))
        #expect(store.search.query.text == " hoshi ")
    }

    @Test func aBlankSearchIsTheStartAndAsksNothing() {
        let store = AppStore()
        store.searchLive(SearchQuery(text: "  "))
        #expect(store.search.atStart)
        #expect(store.search.asked == nil)
        #expect(store.search.list.state == .idle)
        // A sort alone is a browse.
        store.searchLive(SearchQuery(sort: .score))
        #expect(!store.search.atStart)
        #expect(store.search.list.state == .loading)
    }

    @Test func oneTypedLetterWaitsForTheNextAsTheEngineDoes() {
        let store = AppStore()
        let hoshi = SearchQuery(text: "hoshi")
        store.searchLive(hoshi)
        store.apply(.content(view: .search(query: hoshi), content: .entries([entry(900_001, "Stardust Wayfarer")])))
        // Back to one letter: the start again, nothing asked, not the answer to "hoshi".
        store.searchLive(SearchQuery(text: "h"))
        #expect(store.search.atStart)
        #expect(store.search.asked == nil)
        #expect(!store.search.pending)
        // One kana is a word.
        store.searchLive(SearchQuery(text: "ほ"))
        #expect(!store.search.atStart)
        #expect(store.search.asked == SearchQuery(text: "ほ"))
        // Return searches for one letter all the same.
        store.search(SearchQuery(text: "h"))
        #expect(store.search.asked == SearchQuery(text: "h"))
        #expect(!store.search.atStart)
    }

    @Test func oneLetterWithFiltersKeepsTheFiltersAnswer() {
        let store = AppStore()
        let mecha = SearchQuery(genres: ["Mecha"])
        store.searchLive(mecha)
        store.apply(.content(view: .search(query: mecha), content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.searchLive(SearchQuery(text: "s", genres: ["Mecha"]))
        #expect(!store.search.atStart)
        #expect(store.search.asked == mecha)
        #expect(store.search.answered == mecha)
        #expect(!store.search.pending)
    }

    @Test func theEarlierAnswerIsKnownForWhatItAnswers() {
        let store = AppStore()
        let hoshi = SearchQuery(text: "hoshi")
        let moon = SearchQuery(text: "moon")
        store.searchLive(hoshi)
        #expect(store.search.pending)
        #expect(store.search.answered == nil)
        store.apply(.content(view: .search(query: hoshi), content: .entries([entry(900_001, "Stardust Wayfarer")])))
        #expect(!store.search.pending)
        #expect(store.search.answered == hoshi)
        store.searchLive(moon)
        // The earlier results stay on screen, known to answer "hoshi".
        #expect(store.search.pending)
        #expect(store.search.answered == hoshi)
        #expect(store.search.list.state == .loaded([900_001]))
        #expect(!store.search.overdue)
        store.apply(.content(view: .search(query: moon), content: .entries([entry(900_003, "Moonlit Platform")])))
        #expect(store.search.answered == moon)
        #expect(store.search.list.state == .loaded([900_003]))
    }

    @Test func anAnswerWaitedForPastTheQuietIsOverdue() async throws {
        let store = AppStore()
        store.searchLive(SearchQuery(text: "hoshi"))
        #expect(!store.search.overdue)
        try await Task.sleep(for: SearchModel.quiet)
        // The timer's task can run late while other tests share the main actor.
        let deadline = ContinuousClock.now + .seconds(5)
        while !store.search.overdue, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(store.search.overdue)
        // A further search keeps the placeholders up, not the results before them.
        store.searchLive(SearchQuery(text: "hoshiku"))
        #expect(store.search.overdue)
        store.apply(.content(
            view: .search(query: SearchQuery(text: "hoshiku")),
            content: .entries([entry(900_001, "Stardust Wayfarer")])
        ))
        #expect(!store.search.overdue)
    }

    @Test func aSearchThatChangesNothingIsNotSentAgain() {
        let store = AppStore()
        #expect(store.search.edit(SearchQuery(text: "hoshi")))
        #expect(!store.search.edit(SearchQuery(text: " hoshi ")))
        // Blank and one letter are told, so the engine drops the search it waits to make.
        #expect(store.search.edit(SearchQuery(text: "h")))
        #expect(!store.search.edit(SearchQuery(text: "h ")))
        #expect(store.search.edit(SearchQuery(text: "")))
        // Return always is.
        #expect(store.search.edit(SearchQuery(text: ""), submitted: true))
    }

    @Test func furtherPagesGoAfterWhatIsThereAndAFailedOneIsAskedAgain() {
        let store = AppStore()
        let mecha = SearchQuery(genres: ["Mecha"])
        store.search(mecha)
        store.apply([
            .searchPaging(query: mecha, hasNext: true),
            .content(view: .search(query: mecha), content: .entries([entry(900_001, "Stardust Wayfarer")])),
        ])
        #expect(store.search.hasNext)
        store.searchMore()
        #expect(store.search.loadingMore)
        // Asked once, however often the end comes near while it is on its way.
        store.searchMore()
        store.apply(.searchMore(query: mecha, page: 2, entries: [], hasNext: true))
        #expect(!store.search.loadingMore)
        #expect(store.search.pages == 1)
        store.searchMore()
        store.apply(.searchMore(
            query: mecha, page: 2,
            entries: [entry(900_003, "Moonlit Platform"), entry(900_001, "Stardust Wayfarer")],
            hasNext: false
        ))
        #expect(store.search.list.state == .loaded([900_001, 900_003]))
        #expect(store.search.pages == 2)
        #expect(!store.search.hasNext)
        // A page of an earlier search is not added.
        store.apply(.searchMore(query: SearchQuery(text: "hoshi"), page: 3, entries: [entry(900_006, "Summer Wind Relay")], hasNext: false))
        #expect(store.search.list.state == .loaded([900_001, 900_003]))
    }

    @Test func theRailsBringTheirTitlesAndTheStartItsRecents() {
        let store = AppStore()
        let rail = BrowseRail(title: "Trending now", detail: nil, query: SearchQuery(sort: .trending),
                              entries: [entry(900_002, "The Library Above the Clouds")])
        store.apply([
            .browse([rail]),
            .recentSearches([SearchQuery(text: "hoshi", genres: ["Mecha"])]),
            .liveSearch(paused: true),
        ])
        #expect(store.search.rails?.first?.title == "Trending now")
        #expect(store.titles[900_002]?.title == "The Library Above the Clouds")
        #expect(store.search.recents.first?.genres == ["Mecha"])
        #expect(store.search.livePaused)
    }

    @Test func seasonalIsTheSeasonPresetOfSearch() {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .gmt
        let october = calendar.date(from: DateComponents(year: 2026, month: 10, day: 24))!
        let query = SearchQuery.season(at: october)
        #expect(query.season == .fall)
        #expect(query.years == Bounds(min: 2026, max: 2026))
        #expect(query.sort == .popularity)
        #expect(query.headline() == "2026 · Fall")
    }

    @Test func eachListTakesOnlyItsOwnAnswerAndCaption() {
        let store = AppStore()
        store.apply([
            .caption(view: .seasonal, text: "autumn 2026"),
            .content(view: .seasonal, content: .entries([entry(900_002, "The Library Above the Clouds")])),
            .content(view: .library(.planning), content: .entries([entry(900_003, "Moonlit Platform")])),
            .calendarWindow(anchor: 1_800_000_000, pages: 1, morePast: true, floor: 1_792_224_000, ceiling: 1_802_419_200, aheadPages: 1, moreAhead: false),
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
        ])
        #expect(store.notices.log.map(\.notice.text) == [
            "AniList is not answering", "plugin update ready: alpha",
        ])
        let current = try! #require(store.notices.current)
        #expect(current.isAlert)
        store.notices.dismiss(current)
        #expect(store.notices.current == nil)
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

    @Test func offlineSaysWhatWaitsQuietly() {
        let store = AppStore()
        store.apply(.sync(SyncState(
            tracker: "anilist", connected: true, user: "Mika", storage: "keychain",
            storageDegraded: false, outbox: 0, deferred: 0, needsReauth: false, last: nil,
            pullError: nil
        )))
        let down = MetaSourceRow(
            source: "anilist", state: "unreachable", healthy: false, active: false, detail: nil, retryAt: nil
        )
        store.apply(.metaStatus([down]))
        #expect(store.accounts.summary == SyncSummary(text: "Offline", state: .waiting, tracker: "anilist"))
        store.apply(.progressQueued)
        #expect(store.accounts.summary.text == "One change waiting to sync")
        var up = down
        up.healthy = true
        store.apply(.metaStatus([up]))
        #expect(store.accounts.summary.text == "One change waiting to sync")
        #expect(!store.accounts.offline)
    }
}
