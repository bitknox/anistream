import Foundation
import Testing
@testable import AnistreamWidgetData

@Suite("Stale")
struct StalenessTests {
    let snapshot = WidgetSnapshot(version: 2, generatedAt: at(day: 20, 19, 10))

    @Test("Continue turns stale a day after the snapshot was made")
    func continueStale() {
        #expect(Staleness.continueStale(snapshot) == at(day: 21, 19, 10))
        #expect(!Staleness.isContinueStale(snapshot, at: at(day: 21, 19, 10)))
        #expect(Staleness.isContinueStale(snapshot, at: at(day: 21, 19, 11)))
    }

    @Test("A version 2 schedule covers its day and the six after; a version 1 its day alone")
    func scheduleCoverage() {
        #expect(Staleness.coversDay(snapshot, at: at(day: 20, 0, 0), calendar: copenhagen))
        #expect(Staleness.coversDay(snapshot, at: at(day: 26, 23, 59), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 27, 0, 0), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 19, 23, 0), calendar: copenhagen))
        var one = snapshot
        one.version = 1
        #expect(Staleness.coversDay(one, at: at(day: 20, 23, 59), calendar: copenhagen))
        #expect(!Staleness.coversDay(one, at: at(day: 21, 0, 0), calendar: copenhagen))
    }

    @Test("A week across the clocks going back still ends at midnight")
    func acrossDaylightSaving() {
        // Copenhagen leaves summer time on Sunday 25 October 2026.
        #expect(Staleness.coversDay(snapshot, at: at(day: 26, 12), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 27, 0, 30), calendar: copenhagen))
    }
}

@Suite("Timelines")
struct TimelineTests {
    @Test("Continue: now, then the moment it turns stale, and it is never asked again")
    func continuePlan() {
        let snapshot = WidgetSnapshot(
            generatedAt: at(day: 20, 19, 10),
            continueWatching: [SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7)])
        let plan = WidgetTimeline.continuePlan(snapshot, now: at(day: 20, 19, 30))
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 21, 19, 10)])
        #expect(plan.reloadAt == nil)
        // Already stale: one entry.
        #expect(WidgetTimeline.continuePlan(snapshot, now: at(day: 22, 9)).dates == [at(day: 22, 9)])
        #expect(WidgetTimeline.continuePlan(nil, now: at(day: 20, 19, 30)).dates == [at(day: 20, 19, 30)])
    }

    @Test("Continue, caught up: an entry when the next episode airs")
    func caughtUpPlan() {
        let snapshot = WidgetSnapshot(
            generatedAt: at(day: 20, 19, 10), signedIn: true,
            nextAiring: SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: at(day: 20, 22)))
        let plan = WidgetTimeline.continuePlan(snapshot, now: at(day: 20, 19, 30))
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 20, 22), at(day: 21, 19, 10)])
    }

    @Test("Airing today: now, each broadcast still to come today, and midnight, when it is made again")
    func airingPlan() {
        let rows = [
            SnapshotAiring(id: 1, title: "A", episode: 1, airsAt: at(day: 20, 13)),
            SnapshotAiring(id: 2, title: "B", episode: 1, airsAt: at(day: 20, 21)),
            SnapshotAiring(id: 3, title: "C", episode: 1, airsAt: at(day: 20, 23, 45)),
            SnapshotAiring(id: 4, title: "D", episode: 1, airsAt: at(day: 21, 9)),
        ]
        let plan = WidgetTimeline.airingPlan(rows, now: at(day: 20, 19, 30), calendar: copenhagen)
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 20, 21), at(day: 20, 23, 45), at(day: 21, 0)])
        #expect(plan.reloadAt == at(day: 21, 0))
    }

    @Test("Airing today from the public schedule is asked again within three hours")
    func publicPlan() {
        let rows = [
            SnapshotAiring(id: 2, title: "B", episode: 1, airsAt: at(day: 20, 21)),
            SnapshotAiring(id: 3, title: "C", episode: 1, airsAt: at(day: 20, 23, 45)),
        ]
        let plan = WidgetTimeline.airingPlan(rows, now: at(day: 20, 19, 30), publicSchedule: true, calendar: copenhagen)
        #expect(plan.reloadAt == at(day: 20, 22, 30))
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 20, 21)])
    }
}
