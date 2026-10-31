// Home's state beside the engine's view: what a front end holds and decides.

import AnistreamKit
@testable import AnistreamStore
import Testing

@MainActor
struct HomeModelTests {
    func spotlight(_ id: AnilistId, _ kind: SpotlightCase = .popular) -> Spotlight {
        Spotlight(
            case: kind, label: .homeSpotlightPopular, entry: entry(id, "Paper Comets"), episode: 1, action: .play,
            reason: nil, resume: nil, starts: nil, playable: .unknown, line: nil, because: nil)
    }

    func view(spotlight: Spotlight? = nil, pending: [HomePart] = [], end: HomeEnd? = .everything) -> HomeView {
        HomeView(
            spotlight: spotlight, sections: [], pending: pending, end: end, offline: false, offlineSince: nil,
            failed: false, season: nil)
    }

    @Test func settlesOnceAskedAndEnded() {
        let model = HomeModel()
        #expect(model.isSettled)
        model.asked()
        #expect(!model.isSettled)
        model.apply(view(pending: [.week], end: nil), titles: TitleCache())
        #expect(!model.isSettled)
        model.apply(view(), titles: TitleCache())
        #expect(model.isSettled)
    }

    @Test func aCardLeavesAtOnceAndReturnsOnUndo() {
        let model = HomeModel()
        model.leaves(900_001)
        #expect(model.leaving == [900_001])
        model.returns(900_001)
        #expect(model.leaving.isEmpty)
    }

    @Test func aLeavingTitleIsForgottenOnceTheEngineDropsIt() {
        let model = HomeModel()
        model.apply(view(spotlight: spotlight(900_001)), titles: TitleCache())
        model.leaves(900_001)
        model.leaves(900_002)
        model.apply(view(spotlight: spotlight(900_001)), titles: TitleCache())
        #expect(model.leaving == [900_001])
    }

    @Test func theSpotlightMovesOnATitleOrACaseChange() {
        let model = HomeModel()
        model.apply(view(spotlight: spotlight(900_001)), titles: TitleCache())
        #expect(model.spotlightChanges == 0)
        model.apply(view(spotlight: spotlight(900_001)), titles: TitleCache())
        #expect(model.spotlightChanges == 0)
        model.apply(view(spotlight: spotlight(900_001, .suggested)), titles: TitleCache())
        #expect(model.spotlightChanges == 1)
        model.apply(view(spotlight: spotlight(900_002, .suggested)), titles: TitleCache())
        #expect(model.spotlightChanges == 2)
    }

    @Test func aMissIsKeptOverWhatHomeWasSentWith() {
        let model = HomeModel()
        #expect(model.playable(900_001, sent: .found) == .found)
        model.playable(900_001, .notFound)
        #expect(model.playable(900_001, sent: .found) == .notFound)
        model.playable(900_001, .unknown)
        #expect(model.playable(900_001, sent: .found) == .found)
    }

    @Test func restoringDropsTheTitleFromTheHiddenList() {
        let model = HomeModel()
        model.hidden([HiddenTitle(entry: entry(900_001, "Paper Comets"), untilAirs: true)])
        model.unhide(900_001)
        #expect(model.hidden?.isEmpty == true)
    }
}
