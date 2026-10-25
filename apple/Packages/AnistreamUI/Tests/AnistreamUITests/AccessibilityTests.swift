// What VoiceOver reads: screen shorthand in words, a broadcast, an episode, a queue row.

import AnistreamKit
@testable import AnistreamStore
@testable import AnistreamUI
import Foundation
import Testing

struct AccessibilityTests {
    @Test func shorthandIsReadInWords() {
        #expect(Spoken.words("Ep 8 out 1d 2h ago") == "Episode 8 out 1 day 2 hours ago")
        #expect(Spoken.words("Ep 9 in 1h 30m") == "Episode 9 in 1 hour 30 minutes")
        #expect(Spoken.words("14 min left") == "14 minutes left")
        #expect(Spoken.words("1 min left") == "1 minute left")
        #expect(Spoken.words("Resume E7 · 10:06") == "Resume E7 · 10:06", "a clock stays a clock")
    }

    @Test func aBroadcastIsReadWithItsEpisodeFirst() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        var e = entry { $0.progress = TitleProgress(done: 7, next: 8) }
        let aired = Broadcast(entry: e, episode: 8, airs: now.addingTimeInterval(-19 * 3600))
        #expect(aired.spoken(now: now) == "Episode 8, out 19 hours ago")
        let coming = Broadcast(entry: e, episode: 9, airs: now.addingTimeInterval(90 * 60))
        #expect(coming.spoken(now: now) == "Episode 9 at \(CalendarWords.time(coming.airs)), in 1 hour 30 minutes")
        e.progress = TitleProgress(done: 8, next: 9)
        #expect(Broadcast(entry: e, episode: 8, airs: aired.airs).spoken(now: now) == "Episode 8, watched")
    }

    @Test func aBroadcastSaysAnEstimatedTimeAndItsMark() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let airs = now.addingTimeInterval(90 * 60)
        let plain = Broadcast(entry: entry { _ in }, episode: 9, airs: airs)
        #expect(plain.clock == CalendarWords.time(airs))
        #expect(plain.mark == nil)
        let estimated = Broadcast(entry: entry { $0.airingApproximate = true; $0.airingDubbed = true }, episode: 9, airs: airs)
        #expect(estimated.clock == "~" + CalendarWords.time(airs))
        #expect(estimated.mark == "Dub")
        #expect(estimated.spoken(now: now) == "Episode 9, Dub at about \(CalendarWords.time(airs)), in 1 hour 30 minutes")
        #expect(Broadcast(entry: entry { $0.airingTranslation = .sub }, episode: 9, airs: airs).mark == nil, "only dubs are marked")
    }

    @Test func anEpisodeIsNamedThenSaysWhereItStands() {
        let e = entry {
            $0.progress = TitleProgress(done: 6, next: 7)
            $0.resume = ResumePoint(position: 606, fraction: 0.42)
        }
        func row(_ number: String, title: String? = nil, completed: Bool = false, kind: EpisodeKind? = nil) -> EpisodeRow {
            EpisodeRow(number: number, title: title, thumbnail: nil, description: nil, durationSecs: 1440,
                       watched: 0, completed: completed, kind: kind, skippable: false, airsAt: nil)
        }
        #expect(row("7", title: "The Lantern Road").spokenName == "Episode 7, The Lantern Road")
        #expect(row("7").spokenName == "Episode 7")
        #expect(row("6", completed: true).spokenValue(e, current: false, upcoming: false) == ["watched", "24 minutes"])
        #expect(row("7").spokenValue(e, current: true, upcoming: false) == ["14 minutes left"])
        #expect(row("8", kind: .filler).spokenValue(e, current: false, upcoming: false) == ["24 minutes", "filler"])
        #expect(row("9").spokenValue(e, current: false, upcoming: true) == ["not out yet"])
    }

    @Test func aQueueRowSaysTheEpisodeAndWhatIsLeft() {
        let e = entry { _ in }
        let underway = ContinueItem(entry: e, episode: 7, resume: ResumePoint(position: 606, fraction: 0.42), airedAgo: nil)
        #expect(underway.spokenProgress == "Episode 7, 14 minutes left")
        let fresh = ContinueItem(entry: e, episode: 9, resume: nil, airedAgo: 2 * 3600)
        #expect(fresh.spokenProgress == "Episode 9, out 2 hours ago")
        #expect(ContinueItem(entry: e, episode: 4, resume: nil, airedAgo: nil).spokenProgress == "Episode 4")
        // A dub is read as the row marks it, a sub as before.
        let dub = entry { $0.watchedIn = .dub }
        let dubbed = ContinueItem(entry: dub, episode: 7, resume: ResumePoint(position: 606, fraction: 0.42), airedAgo: nil)
        #expect(dubbed.spokenProgress == "Episode 7, Dub, 14 minutes left")
        #expect(ContinueItem(entry: dub, episode: 4, resume: nil, airedAgo: nil).spokenProgress == "Episode 4, Dub")
        let sub = entry { $0.watchedIn = .sub }
        #expect(ContinueItem(entry: sub, episode: 4, resume: nil, airedAgo: nil).spokenProgress == "Episode 4")
    }
}
