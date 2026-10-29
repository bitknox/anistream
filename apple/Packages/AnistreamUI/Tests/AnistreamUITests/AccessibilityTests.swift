// What VoiceOver reads: screen shorthand in words, a broadcast, an episode, a queue row.

import AnistreamKit
@testable import AnistreamStore
@testable import AnistreamUI
import Foundation
import Testing

struct AccessibilityTests {
    @Test func aSpanIsReadInFullWords() {
        #expect(Spoken.span(26 * 3600) == "1 day, 2 hours")
        #expect(Spoken.span(90 * 60) == "1 hour, 30 minutes")
        #expect(Spoken.span(14 * 60) == "14 minutes")
        #expect(Spoken.span(10 * 86_400) == "10 days")
        #expect(Spoken.out(30) == "just now")
    }

    @Test func aBroadcastIsReadWithItsEpisodeFirst() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        var e = entry { $0.progress = TitleProgress(done: 7, next: 8) }
        let aired = Broadcast(entry: e, episode: 8, airs: now.addingTimeInterval(-19 * 3600))
        #expect(aired.spoken(now: now) == "Episode 8, out 19 hours ago")
        let coming = Broadcast(entry: e, episode: 9, airs: now.addingTimeInterval(90 * 60))
        #expect(coming.spoken(now: now) == "Episode 9 at \(CalendarWords.time(coming.airs)), in 1 hour, 30 minutes")
        e.progress = TitleProgress(done: 8, next: 9)
        #expect(Broadcast(entry: e, episode: 8, airs: aired.airs).spoken(now: now) == "Episode 8, watched")
    }

    @Test func aBroadcastSaysAnEstimatedTimeAndItsMark() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let airs = now.addingTimeInterval(90 * 60)
        let plain = Broadcast(entry: entry { _ in }, episode: 9, airs: airs)
        #expect(plain.clock == CalendarWords.time(airs))
        #expect(!plain.dubbed)
        let estimated = Broadcast(entry: entry { $0.airingApproximate = true; $0.airingDubbed = true }, episode: 9, airs: airs)
        #expect(estimated.clock == "~" + CalendarWords.time(airs))
        #expect(estimated.dubbed)
        #expect(estimated.spoken(now: now) == "Episode 9, Dubbed at about \(CalendarWords.time(airs)), in 1 hour, 30 minutes")
        #expect(Broadcast(entry: entry { $0.airingTranslation = .sub }, episode: 9, airs: airs).dubbed == false, "only dubs are marked")
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
        #expect(dubbed.spokenProgress == "Episode 7, Dubbed, 14 minutes left")
        #expect(ContinueItem(entry: dub, episode: 4, resume: nil, airedAgo: nil).spokenProgress == "Episode 4, Dubbed")
        let sub = entry { $0.watchedIn = .sub }
        #expect(ContinueItem(entry: sub, episode: 4, resume: nil, airedAgo: nil).spokenProgress == "Episode 4")
    }
}
