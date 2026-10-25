import Foundation
import Testing
@testable import AnistreamWidgetData

@Suite("The playback and airing Live Activities")
struct PlaybackActivityTests {
    let at = Date(timeIntervalSince1970: 1_792_516_200)

    var playing: PlaybackActivityState {
        PlaybackActivityState(
            titleID: 900001, title: "Hoshikuzu no Tabibito", episode: "7", position: 300, duration: 1_400, at: at)
    }

    @Test("The timeline runs from the episode's start to its end on the clock")
    func timeline() {
        #expect(playing.timeline == at.addingTimeInterval(-300)...at.addingTimeInterval(1_100))
        var fast = playing
        fast.rate = 2
        #expect(fast.timeline == at.addingTimeInterval(-150)...at.addingTimeInterval(550))
        #expect(abs(playing.fraction - 300.0 / 1_400) < 0.0001)
    }

    @Test("Playing, the time left runs in the view; paused, it is said")
    func words() {
        var paused = playing
        paused.paused = true
        let text = PlaybackActivityText(paused)
        #expect(text.title == "Hoshikuzu no Tabibito")
        #expect(text.episode == "Episode 7")
        #expect(text.pausedLine == "Paused · 18:20 left")
        #expect(text.accessibilityLabel == "Hoshikuzu no Tabibito, episode 7, paused, 19 minutes left")
        #expect(PlaybackActivityText(playing).accessibilityLabel.contains("playing"))
        #expect(PlaybackActivityText.clock(3_723) == "1:02:03")
    }

    @Test("A playhead where the old dates put it is no change; a pause, a seek or another episode is")
    func differs() {
        var later = playing
        later.at = at.addingTimeInterval(60)
        later.position = 360.4
        #expect(!later.differs(from: playing))
        var sought = later
        sought.position = 600
        #expect(sought.differs(from: playing))
        var paused = later
        paused.paused = true
        #expect(paused.differs(from: playing))
        var next = playing
        next.episode = "8"
        #expect(next.differs(from: playing))
    }

    @Test("Airing: the time it airs at, then aired and out now")
    func airing() {
        let zone = TimeZone(identifier: "Europe/Copenhagen")!
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone
        let airs = calendar.date(from: DateComponents(year: 2026, month: 10, day: 20, hour: 19, minute: 45))!
        let state = AiringActivityState(
            titleID: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: airs, since: airs.addingTimeInterval(-1_200))
        let before = AiringActivityText(state, out: false, locale: Locale(identifier: "en_GB"), timeZone: zone)
        #expect(before.episode == "Episode 6")
        #expect(before.time == "Airs at 19:45")
        #expect(before.accessibilityLabel == "Kumo no Ue no Toshokan, episode 6, airs at 19:45")
        let after = AiringActivityText(state, out: true, locale: Locale(identifier: "en_GB"), timeZone: zone)
        #expect(after.time == "Aired at 19:45")
        #expect(after.accessibilityLabel == "Kumo no Ue no Toshokan, episode 6, out now")
        #expect(state.countdown == airs.addingTimeInterval(-1_200)...airs)
    }

    @Test("The countdown never starts after the air time")
    func countdownStart() {
        let state = AiringActivityState(titleID: 1, title: "A", episode: 1, airsAt: at, since: at.addingTimeInterval(30))
        #expect(state.since == at)
        #expect(state.countdown == at...at)
    }
}
