import Foundation
import Testing
@testable import AnistreamWidgetData

let en = Locale(identifier: "en_GB")

@Suite("The downloads' Live Activity")
struct DownloadsActivityTests {
    let one = DownloadsActivityState(
        titleID: 900001, title: "Hoshikuzu no Tabibito", episode: "8", bytesDone: 412_000_000,
        bytesTotal: 1_100_000_000, speed: 2_400_000, secondsLeft: 290)

    @Test("One download: the episode, its bytes and the time left")
    func single() {
        let text = DownloadsActivityText(one, locale: en)
        #expect(text.title == "Hoshikuzu no Tabibito" && text.titleIsShow)
        #expect(text.line1 == "Episode 8")
        #expect(text.line2 == "412 MB of 1.1 GB · 5 min left")
        #expect(text.timeLeft == "5 min")
        #expect(text.detail == "412 MB of 1.1 GB · 2.4 MB/s")
        #expect(text.accessibilityLabel == "Hoshikuzu no Tabibito, episode 8, 37 percent, 5 minutes left")
        #expect(abs(one.fraction - 0.3745) < 0.001)
    }

    @Test("Several: how many, which first, and what is left at what speed")
    func several() {
        var state = one
        state.count = 3
        state.bytesLeft = 2_700_000_000
        let text = DownloadsActivityText(state, locale: en)
        #expect(text.title == "Downloading 3 episodes" && !text.titleIsShow)
        #expect(text.line1 == "Hoshikuzu no Tabibito E8, then 2 more")
        #expect(text.line2 == "2.7 GB left · 2.4 MB/s")
    }

    @Test("Paused: no time left, and the bytes")
    func paused() {
        var state = one
        state.paused = true
        state.secondsLeft = nil
        let text = DownloadsActivityText(state, locale: en)
        #expect(text.line1 == "Paused")
        #expect(text.line2 == "412 MB of 1.1 GB")
        #expect(text.timeLeft == nil)
        #expect(text.accessibilityLabel == "Hoshikuzu no Tabibito, episode 8, 37 percent, paused")
    }

    @Test("Finished: ready to watch offline")
    func finished() {
        var state = one
        state.finished = true
        state.bytesDone = state.bytesTotal
        let text = DownloadsActivityText(state, locale: en)
        #expect(text.line1 == "Episode 8 downloaded")
        #expect(text.line2 == "Ready to watch offline")
        #expect(text.timeLeft == nil)
    }

    @Test("Stale, the app suspended: no time left, and when it carries on")
    func stale() {
        let text = DownloadsActivityText(one, stale: true, locale: en)
        #expect(text.line1 == "Continues when you open anistream")
        #expect(text.line2 == "412 MB of 1.1 GB")
        #expect(text.timeLeft == nil)
    }

    @Test("Times left round up, in hours past one")
    func minutes() {
        #expect(DownloadsActivityText.minutes(1) == "1 min")
        #expect(DownloadsActivityText.minutes(61) == "2 min")
        #expect(DownloadsActivityText.minutes(3_600) == "1 h")
        #expect(DownloadsActivityText.minutes(5_000) == "1 h 24 min")
    }

    @Test("A change of episode, count, pause or finish shows at once; progress waits")
    func kinds() {
        var moved = one
        moved.bytesDone += 1_000_000
        #expect(!moved.differsInKind(from: one))
        var paused = one
        paused.paused = true
        #expect(paused.differsInKind(from: one))
        var next = one
        next.episode = "9"
        #expect(next.differsInKind(from: one))
    }

    @Test("Play on a finished download links into the app")
    func link() {
        #expect(WidgetLink.download(42).url.absoluteString == "anistream://download/42")
        #expect(WidgetLink(url: URL(string: "anistream://download/42")!) == .download(42))
        #expect(WidgetLink(url: URL(string: "anistream://download/x")!) == nil)
    }
}
