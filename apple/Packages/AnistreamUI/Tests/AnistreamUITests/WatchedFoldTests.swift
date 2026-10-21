// The watched episodes an episode list folds away, and what its row says.

import AnistreamKit
@testable import AnistreamUI
import Testing

struct WatchedFoldTests {
    /// Episodes numbered from `first`, the first `watched` of them watched.
    func rows(_ count: Int, watched: Int, from first: Int = 1) -> [EpisodeRow] {
        (0..<count).map { index in
            EpisodeRow(number: String(first + index), title: nil, thumbnail: nil, description: nil,
                       durationSecs: 1440, watched: 0, completed: index < watched, kind: nil,
                       skippable: false, airsAt: nil)
        }
    }

    @Test func theListStartsAtTheLastEpisodeWatched() throws {
        let fold = try #require(WatchedFold(rows(12, watched: 6)))
        #expect(fold.count == 5)
        #expect(fold.label == "Episodes 1–5 watched")
        #expect(WatchedFold.hide == "Hide watched episodes")
    }

    @Test func aSecondCourSaysItsOwnNumbers() {
        #expect(WatchedFold(rows(12, watched: 8, from: 13))?.label == "Episodes 13–19 watched")
    }

    @Test func nothingFoldsWithNothingWatchedOrOnlyTheFirstEpisodeToHide() {
        #expect(WatchedFold(rows(12, watched: 0)) == nil, "nothing watched")
        #expect(WatchedFold(rows(12, watched: 1)) == nil, "the last watched is the first")
        #expect(WatchedFold(rows(12, watched: 2)) == nil, "only episode 1 would go")
        #expect(WatchedFold(rows(12, watched: 3))?.label == "Episodes 1–2 watched")
        #expect(WatchedFold([]) == nil)
    }

    @Test func onlyTheWatchedRunAtTheTopFolds() {
        // 1 to 4 watched, 5 not, 6 watched: only the run before 5 folds, so the row says no more
        // than is true.
        var table = rows(8, watched: 4)
        table[5].completed = true
        #expect(WatchedFold(table)?.label == "Episodes 1–3 watched")
    }

    @Test func aFinishedShowStartsAtItsLastEpisode() {
        #expect(WatchedFold(rows(12, watched: 12))?.label == "Episodes 1–11 watched")
    }
}
