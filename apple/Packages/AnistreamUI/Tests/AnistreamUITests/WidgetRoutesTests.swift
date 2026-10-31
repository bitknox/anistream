// Where the widgets' taps land in the app.

@testable import AnistreamUI
import AnistreamStore
import AnistreamWidgetData
import Foundation
import Testing

@MainActor
struct WidgetRoutesTests {
    @Test func aTitleOpensItsPageOnceOverTheSectionOnScreen() {
        let store = AppStore()
        let navigator = Navigator(route: LaunchRoute())
        navigator.openWidgetLink(.title(900_001), store: store)
        navigator.openWidgetLink(.title(900_001), store: store)
        #expect(navigator.titleOnMac == 900_001)
        #expect(navigator.paths[.home] == [900_001], "not pushed twice")
        #expect(store.sentLog.isEmpty)
    }

    @Test func anEpisodePlaysFromItsTitlesPage() {
        let store = AppStore()
        let navigator = Navigator(route: LaunchRoute())
        navigator.openWidgetLink(.play(900_001, episode: 7), store: store)
        #expect(navigator.titleOnMac == 900_001)
        #expect(store.sentLog == ["play"])
    }

    @Test func theHeaderOpensTheCalendarOnThisWeek() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.calendarWeek = 2
        navigator.openWidgetLink(.calendar, store: AppStore())
        #expect(navigator.section == .calendar)
        #expect(navigator.calendarWeek == 0)
    }

    @Test func connectingOpensAccounts() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.openWidgetLink(.accounts, store: AppStore())
        #expect(navigator.section == .settings)
        #expect(navigator.settingsPane == .accounts)
    }

    @Test func aFinishedDownloadPlaysOrOpensDownloadsWhileTheQueueIsNotHere() {
        let store = AppStore()
        let navigator = Navigator(route: LaunchRoute())
        navigator.openWidgetLink(.download(7), store: store)
        #expect(navigator.section == .downloads)
        #expect(store.sentLog.isEmpty)
    }

    @Test func thePlayIntentsLinkWaitsForTheAppToTakeIt() {
        let router = WidgetLinkRouter()
        router.open(.play(900_001, episode: 7))
        var taken: [WidgetLink] = []
        router.register { taken.append($0) }
        #expect(taken == [.play(900_001, episode: 7)])
        router.open(.title(900_002))
        #expect(taken == [.play(900_001, episode: 7), .title(900_002)])
    }
}
