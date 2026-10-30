// The discussion's bookkeeping: which episodes have a thread, and when the player offers it.

@testable import AnistreamStore
import Testing

@MainActor
struct DiscussionTests {
    @Test func theCountOfAFoundThreadIsKeptPerEpisode() {
        let model = DiscussionModel()
        model.found(DiscussionKey(media: 900_001, episode: 7), comments: 214)
        #expect(model.count(900_001, episode: 7) == 214)
        #expect(model.count(900_001, episode: 8) == nil)
        #expect(model.count(900_002, episode: 7) == nil)
    }

    @Test func closingAnEarlierDiscussionLeavesTheLaterOneOpen() {
        let model = DiscussionModel()
        let seven = DiscussionKey(media: 900_001, episode: 7)
        let eight = DiscussionKey(media: 900_001, episode: 8)
        model.requested(seven)
        model.requested(eight)
        #expect(!model.owns(seven), "the window that showed 7 closing must not close 8")
        #expect(model.owns(eight))
        model.closed()
        #expect(!model.owns(eight))
    }

    @Test func showingADiscussionSetsItsTargetUntilItIsDismissed() {
        let model = DiscussionModel()
        model.show(DiscussionKey(media: 900_001, episode: 3))
        #expect(model.target == DiscussionKey(media: 900_001, episode: 3))
        model.target = nil
        #expect(model.target == nil)
    }

    @Test func thePlayerOffersTheDiscussionFrom85PercentAndKeepsOfferingIt() {
        var watch = PlayerDiscussion()
        let unknown = watch.see(position: 1_000, duration: nil)
        let early = watch.see(position: 1_000, duration: 1_440)
        let reached = watch.see(position: 1_224, duration: 1_440)
        // A seek back does not take it away.
        let after = watch.see(position: 10, duration: 1_440)
        #expect(!unknown && !early && reached && after)
    }

    @Test func onlyWholeEpisodeNumbersHaveADiscussion() {
        #expect(PlayerDiscussion.episode("7") == 7)
        #expect(PlayerDiscussion.episode("7.5") == nil)
        #expect(PlayerDiscussion.episode("SP1") == nil)
        #expect(PlayerDiscussion.episode(nil) == nil)
    }
}
