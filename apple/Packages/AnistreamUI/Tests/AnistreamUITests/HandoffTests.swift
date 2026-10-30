// Handoff on screen: what another device hands over opens, and an episode handed over is offered
// on its title's page, never started by itself.

@testable import AnistreamUI
import AnistreamKit
import AnistreamStore
import Foundation
import Testing

@MainActor
struct HandoffTests {
    @Test func aTitleHandedOverOpensItsPageOnce() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.continueHandoff(.title(900_001))
        navigator.continueHandoff(.title(900_001))
        #expect(navigator.titleOnScreen == 900_001)
        #expect(navigator.paths[.continue] == [900_001])
        #expect(navigator.handedOff == nil)
    }

    @Test func anEpisodeHandedOverOpensItsTitleWithTheOfferToResume() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.continueHandoff(.playing(900_001, episode: "7", position: 760))
        #expect(navigator.titleOnScreen == 900_001)
        #expect(navigator.handedOff == HandedOff(id: 900_001, episode: "7", position: 760))
    }

    @Test func anActivityAsTheOtherDeviceSaysItOpensHere() {
        let types = HandoffTypes(bundleIdentifier: "tv.anistream.app")
        // As the player there describes the episode it plays.
        let playing = NSUserActivity(activityType: types.playing)
        Handoff.playing(900_001, episode: "7", position: 760)
            .describe(playing, title: "Hoshikuzu no Tabibito · Episode 7")
        let navigator = Navigator(route: LaunchRoute())
        #expect(HandoffRoutes.take(playing, types: types, navigator: navigator))
        #expect(navigator.titleOnScreen == 900_001)
        #expect(TitleAction(entry { _ in }, handedOff: navigator.handedOff).title == "Resume episode 7 · 12:40")

        // As the title page there describes itself.
        let page = NSUserActivity(activityType: types.title)
        Handoff.title(900_005).describe(page, title: "Hagane no Niwashi")
        #expect(HandoffRoutes.take(page, types: types, navigator: navigator))
        #expect(navigator.titleOnScreen == 900_005)

        // The Fixtures app's, from the same team, is not taken.
        let fixtures = NSUserActivity(activityType: "tv.anistream.fixtures.title")
        Handoff.title(900_002).describe(fixtures, title: "Kumo no Ue no Toshokan")
        #expect(!HandoffRoutes.take(fixtures, types: types, navigator: navigator))
        #expect(navigator.titleOnScreen == 900_005)
    }

    @Test func thePageOffersTheHandedOverEpisodeAtItsPlace() {
        // Here the title was left at episode 6, 10:06 in.
        let here = entry {
            $0.progress = TitleProgress(done: 5, next: 6)
            $0.resume = ResumePoint(position: 606, fraction: 0.4)
        }
        let handedOff = HandedOff(id: 900_001, episode: "7", position: 760)
        let action = TitleAction(here, handedOff: handedOff)
        #expect(action.title == "Resume episode 7 · 12:40")
        #expect(action.sentence == "Resume episode 7 · 12:40")
        #expect(action.playable)

        #expect(TitleAction(here).title == "Resume episode 6 · 10:06")
        let otherTitle = HandedOff(id: 900_005, episode: "7", position: 760)
        #expect(TitleAction(here, handedOff: otherTitle).title == "Resume episode 6 · 10:06")
        let notWhole = HandedOff(id: 900_001, episode: "7.5", position: 760)
        #expect(TitleAction(here, handedOff: notWhole).title == "Resume episode 6 · 10:06")
    }

    @Test func playingTheOfferUsesItUp() {
        let store = AppStore()
        let navigator = Navigator(route: LaunchRoute())
        navigator.continueHandoff(.playing(900_001, episode: "7", position: 760))
        let mark = store.sentLog.count
        TitleAction(entry { _ in }, handedOff: navigator.handedOff).play(900_001, store: store, navigator: navigator)
        #expect(Array(store.sentLog.dropFirst(mark)) == ["play"])
        #expect(navigator.handedOff == nil)
        #expect(TitleAction(entry { _ in }, handedOff: navigator.handedOff).handedOff == nil)
    }

    @Test func theTitlePageOfferDoesNotFollowThePlayer() {
        let page = entry { _ in }
        #expect(TitleHandoff.offers(page, onScreen: page.id))
        #expect(!TitleHandoff.offers(page, onScreen: 900_002))
        #expect(!TitleHandoff.offers(entry { $0.title = "" }, onScreen: page.id))
    }
}
