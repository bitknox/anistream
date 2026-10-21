// The model's helpers as the Swift side ports them, held to the Rust originals' own cases.

import AnistreamKit
import Foundation
import Testing

/// An entry with nothing but its id and title, as `Entry::new` makes one.
func entry(_ id: AnilistId = 1, _ title: String = "Hoshikuzu no Tabibito") -> Entry {
    Entry(
        id: id, title: title, secondary: nil, format: nil, episodes: nil, year: nil, score: nil,
        studio: nil, synopsis: "", coverUrl: nil, coverColor: nil, bannerUrl: nil, genres: [], availableOn: [],
        progress: nil, airingIn: nil, airingEpisode: nil, nextEpisode: nil, nextAiringIn: nil,
        lastAired: nil, resume: nil, related: [], recommended: [], trailerUrl: nil, listStatus: nil,
        titleRomaji: nil, titleEnglish: nil, titleNative: nil, season: nil, startDate: nil, endDate: nil,
        source: nil, adult: false, detail: nil
    )
}

struct ModelTests {
    @Test func aResumePointReadsAsAClock() {
        #expect(ResumePoint(position: 606, fraction: 0.42).clock == "10:06")
        #expect(ResumePoint(position: 3723.9, fraction: nil).clock == "1:02:03")
        #expect(ResumePoint(position: -4, fraction: nil).clock == "0:00")
    }

    @Test func theNextEpisodeProvesTheOneBeforeItIsOut() {
        var e = entry()
        #expect(e.latestAired == nil)
        e.nextEpisode = 1
        #expect(e.latestAired == nil, "nothing before episode 1")
        e.nextEpisode = 6
        #expect(e.latestAired?.episode == 5 && e.latestAired?.ago == nil)
        // A schedule row that lags loses to the larger claim, and its "ago" goes with it.
        e.lastAired = Aired(episode: 4, ago: 90_000)
        #expect(e.latestAired?.episode == 5 && e.latestAired?.ago == nil)
        // One that agrees keeps its "ago".
        e.lastAired = Aired(episode: 5, ago: 7_200)
        #expect(e.latestAired?.episode == 5 && e.latestAired?.ago == 7_200)
    }

    @Test func anEpisodeHasAiredByTheTitlesBroadcasts() {
        var e = entry()
        #expect(e.airing(of: 4) == .aired, "nothing scheduled: a finished title has aired all it has")
        e.nextEpisode = 4
        e.nextAiringIn = 2 * 86_400
        #expect(e.airing(of: 3) == .aired)
        #expect(e.airing(of: 4) == .upcoming(inSeconds: 2 * 86_400), "the next broadcast is dated")
        #expect(e.airing(of: 5) == .upcoming(inSeconds: nil), "the one after is not")
        // A countdown that ran out since the schedule came, or a broadcast row that says it aired.
        e.nextAiringIn = 0
        #expect(e.airing(of: 4) == .aired)
        e.nextAiringIn = 3600
        e.lastAired = Aired(episode: 4, ago: 60)
        #expect(e.airing(of: 4) == .aired)
    }

    @Test func anAirTimeIsTheDayAndTheTimeWithinTheWeek() {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        // Wednesday 21 October 2026, at noon where the test runs.
        let now = calendar.date(from: DateComponents(year: 2026, month: 10, day: 21, hour: 12))!
        let saturday = calendar.date(from: DateComponents(year: 2026, month: 10, day: 24, hour: 18, minute: 30))!
        #expect(AirTime.words(saturday, now: now) == "Saturday 18:30")
        let later = calendar.date(from: DateComponents(year: 2026, month: 11, day: 2, hour: 9, minute: 5))!
        #expect(AirTime.words(later, now: now) == "2 November 09:05")
        let seconds = Int64(saturday.timeIntervalSince(now))
        #expect(EpisodeAvailability.upcoming(inSeconds: seconds).airs(now: now) == "Saturday 18:30")
        #expect(EpisodeAvailability.upcoming(inSeconds: nil).airs(now: now) == nil)
        #expect(EpisodeAvailability.aired.airs(now: now) == nil)
    }

    @Test func progressCountsAgainstTheLengthOrWhatHasAired() {
        var e = entry()
        e.progress = TitleProgress(done: 3, next: 4)
        #expect(e.watchedFraction == 0)
        e.nextEpisode = 7
        #expect(e.progressOf?.total == 6 && e.progressOf?.isLength == false)
        #expect(e.watchedFraction == 0.5)
        e.episodes = 12
        #expect(e.progressOf?.total == 12 && e.progressOf?.isLength == true)
        #expect(e.watchedFraction == 0.25)
    }
}

struct WordingTests {
    @Test func spansReadAsTheTerminalReadsThem() {
        #expect(Wording.countdown(0) == "now")
        #expect(Wording.countdown(59) == "0m")
        #expect(Wording.countdown(4 * 60) == "4m")
        #expect(Wording.countdown(2 * 3600) == "2h")
        #expect(Wording.countdown(2 * 3600 + 5 * 60) == "2h 5m")
        #expect(Wording.countdown(6 * 86_400 + 17 * 3600) == "6d 17h")
        #expect(Wording.countdown(9 * 86_400 + 17 * 3600) == "9d")
        #expect(Wording.ago(30) == "just now")
        #expect(Wording.ago(2 * 86_400) == "2d ago")
        #expect(Wording.plural(1, "episode") == "1 episode")
        #expect(Wording.plural(3, "episode") == "3 episodes")
    }
}
