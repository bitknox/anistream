// What Home's cards say and do from what the engine sent and what the sources check found.

import AnistreamKit
@testable import AnistreamUI
import Testing

struct HomeTests {
    @Test func aMissTurnsPlayIntoAddToPlanning() {
        #expect(HomeFacts(playable: .notFound, action: .play).action == .addToPlanning)
        #expect(HomeFacts(playable: .notFound, action: .resume).action == .addToPlanning)
        #expect(HomeFacts(playable: .found, action: .play).action == .play)
        // A check that could not tell changes nothing.
        #expect(HomeFacts(playable: .unknown, action: .resume).action == .resume)
    }

    @Test func nothingToPressHasNoMainAction() {
        #expect(HomeMain(action: .nothing, episode: 1) == nil)
        #expect(HomeMain(action: .resume, episode: 3)?.symbol == "play.fill")
        #expect(HomeMain(action: .addToPlanning, episode: 1)?.symbol == "plus")
    }

    @Test func aStartDateIsWordedByItsPrecision() {
        #expect(HomeWords.starts(nil) == nil)
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: nil, day: nil)) == "2027")
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: 4, day: nil))?.contains("2027") == true)
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: 4, day: 9))?.contains("2027") == false)
    }
}
