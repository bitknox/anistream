// The 600 ms rule, on a clock the test steps.

@testable import AnistreamStore
import AnistreamKit
import Synchronization
import Testing

/// A clock that moves only when told.
final class SteppedClock: Sendable {
    private let instant = Mutex(ContinuousClock.now)

    var now: ContinuousClock.Instant { instant.withLock { $0 } }

    func advance(_ duration: Duration) {
        instant.withLock { $0 += duration }
    }
}

@MainActor
struct JobsTests {
    @Test func aJobShowsOnlyOnceItHasRunSixHundredMilliseconds() {
        let clock = SteppedClock()
        let store = AppStore(jobs: JobsModel(now: { clock.now }, schedule: false))
        store.apply(.jobStarted(job: 1, activity: "loading continue watching"))
        #expect(store.jobs.running.count == 1)
        clock.advance(.milliseconds(599))
        store.jobs.refresh()
        #expect(store.jobs.shown.isEmpty)
        clock.advance(.milliseconds(1))
        store.jobs.refresh()
        #expect(store.jobs.shown.map(\.activity) == ["loading continue watching"])
    }

    @Test func aJobThatEndsInsideTheDelayNeverShows() {
        let clock = SteppedClock()
        let store = AppStore(jobs: JobsModel(now: { clock.now }, schedule: false))
        store.apply(.jobStarted(job: 1, activity: "searching “hoshikuzu”"))
        clock.advance(.milliseconds(300))
        store.apply(.jobDone(1))
        clock.advance(.milliseconds(400))
        store.jobs.refresh()
        #expect(store.jobs.running.isEmpty && store.jobs.shown.isEmpty)
    }

    @Test func theNewestJobShowsFirstWithWhatHoldsItUp() {
        let clock = SteppedClock()
        let store = AppStore(jobs: JobsModel(now: { clock.now }, schedule: false))
        store.apply(.jobStarted(job: 1, activity: "loading this season"))
        clock.advance(.milliseconds(100))
        store.apply(.jobStarted(job: 2, activity: "loading the calendar"))
        clock.advance(.milliseconds(700))
        store.apply(.jobNote(job: 1, note: "waiting on AniList", wait: 1.5))
        #expect(store.jobs.shown.map(\.id) == [2, 1])
        let held = try! #require(store.jobs.shown.last)
        #expect(held.note == "waiting on AniList")
        #expect(held.waitUntil == clock.now + .milliseconds(1500))
    }

    @Test func aScheduledJobShowsWithoutBeingAsked() async throws {
        let store = AppStore()
        store.apply(.jobStarted(job: 1, activity: "loading continue watching"))
        #expect(store.jobs.shown.isEmpty)
        try await Task.sleep(for: .milliseconds(900))
        #expect(store.jobs.shown.map(\.id) == [1])
    }
}
