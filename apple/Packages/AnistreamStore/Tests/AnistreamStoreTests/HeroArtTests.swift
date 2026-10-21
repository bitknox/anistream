// The one rule for a title's wide art.

import AnistreamKit
@testable import AnistreamStore
import Testing

struct HeroArtTests {
    /// Episodes 1 to `count`, each with a still, the first `completed` of them watched.
    func rows(_ count: Int, completed: Int = 0, stills: Bool = true) -> [EpisodeRow] {
        (1...count).map { n in
            EpisodeRow(
                number: String(n), title: nil, thumbnail: stills ? "ep/900005/\(n).jpg" : nil,
                description: nil, durationSecs: 1440, watched: n <= completed ? 1 : 0,
                completed: n <= completed, kind: nil, skippable: false
            )
        }
    }

    let bannerless = entry(900_005, "Hagane no Niwashi") { $0.bannerUrl = nil }

    @Test func theBannerWins() {
        let titled = entry(900_005, "Hagane no Niwashi") { $0.bannerUrl = "banner/900001.jpg" }
        #expect(HeroArt.resolve(titled, episodes: rows(12, completed: 6)) == .banner("banner/900001.jpg"))
        #expect(HeroArt.resolve(titled, episodes: nil) == .banner("banner/900001.jpg"))
    }

    @Test func theLastWatchedEpisodesStill() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12, completed: 6)) == .still("ep/900005/6.jpg"))
    }

    @Test func whatTheTrackerCountsAsDoneIsWatchedToo() {
        let tracked = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.progress = TitleProgress(done: 4, next: 5)
        }
        #expect(HeroArt.resolve(tracked, episodes: rows(12)) == .still("ep/900005/4.jpg"))
    }

    @Test func episodeOneWhenNothingIsWatched() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12)) == .still("ep/900005/1.jpg"))
    }

    @Test func neverAStillFromAnEpisodeNotYetSeen() {
        // Episodes 2 to 12 have stills; 1, the only one watched, has none.
        var table = rows(12, completed: 1)
        table[0].thumbnail = nil
        #expect(HeroArt.resolve(bannerless, episodes: table) == .compact)
        // A watched episode with no still gives way to an earlier one that has one, never a later.
        var later = rows(12, completed: 5)
        later[4].thumbnail = nil
        #expect(HeroArt.resolve(bannerless, episodes: later) == .still("ep/900005/4.jpg"))
    }

    @Test func compactWhenThereIsNothingWide() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12, completed: 3, stills: false)) == .compact)
        #expect(HeroArt.resolve(bannerless, episodes: []) == .compact)
    }

    @Test func thePlateWhileTheEpisodesAreOnTheirWay() {
        #expect(HeroArt.resolve(bannerless, episodes: nil) == .waiting)
    }
}
