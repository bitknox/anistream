// What VoiceOver reads of the player: the seek line's times in words.

import Testing

@testable import AnistreamPlayerUI

struct PlayerAccessibilityTests {
    @Test func theSeekLineReadsItsTimesInWords() {
        #expect(PlayerClock.spoken(724) == "12 minutes 4 seconds")
        #expect(PlayerClock.spoken(1420) == "23 minutes 40 seconds")
        #expect(PlayerClock.spoken(3729) == "1 hour 2 minutes 9 seconds")
        #expect(PlayerClock.spoken(60) == "1 minute")
        #expect(PlayerClock.spoken(0) == "0 seconds")
        #expect(PlayerClock.spoken(.nan) == "0 seconds")
    }
}
