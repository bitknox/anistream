// The menu bar's Playback menu: its speed ladder and how a speed is named.

@testable import AnistreamPlayerUI
import Testing

@MainActor
struct PlaybackMenuTests {
    @Test func slowerAndFasterStepTheLadderAndStopAtItsEnds() {
        #expect(PlaybackMenu.step(1, by: 1) == 1.25)
        #expect(PlaybackMenu.step(1, by: -1) == 0.75)
        #expect(PlaybackMenu.step(2, by: 1) == 2, "the top")
        #expect(PlaybackMenu.step(0.5, by: -1) == 0.5, "the bottom")
        // A speed off the ladder steps to the next rung that way.
        #expect(PlaybackMenu.step(1.1, by: 1) == 1.25)
        #expect(PlaybackMenu.step(1.1, by: -1) == 1)
    }

    @Test func aSpeedIsNamedAsTheMenuBarNamesIt() {
        #expect(PlaybackMenu.name(1) == "Normal")
        #expect(PlaybackMenu.name(0.75) == "0.75×")
        #expect(PlaybackMenu.name(2) == "2×")
    }
}
