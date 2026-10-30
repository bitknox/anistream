// One visit of a title page's trailer: when the banner gives way, and when the view is let go.

import AnistreamKit
@testable import AnistreamUI
import Foundation
import Testing

@MainActor
private final class FakeSurface: TrailerSurface {
    var controls: [TrailerControl] = []
    var released = false
    var signal: (@MainActor (TrailerSignal) -> Void)?

    func control(_ control: TrailerControl) { controls.append(control) }
    func tearDown() { released = true }
}

@MainActor
private func visit(
    load: Duration = .seconds(5), away: Duration = .seconds(5), fade: Duration = .milliseconds(30)
) -> (TrailerVisit, FakeSurface) {
    let visit = TrailerVisit(loadTimeout: load, awayLimit: away, fade: fade)
    let surface = FakeSurface()
    visit.begin { signal in
        surface.signal = signal
        return surface
    }
    return (visit, surface)
}

/// Wait for `condition`, five seconds at most. The visit's timers run on the main actor with every
/// other test's, so a fixed sleep can end before a second timer, started by the first, has fired.
@MainActor
private func until(_ condition: () -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(5)
    while !condition() {
        guard ContinuousClock.now < deadline else {
            Issue.record("timed out")
            return
        }
        try await Task.sleep(for: .milliseconds(10))
    }
}

@MainActor
struct TrailerVisitTests {
    @Test func theBannerStaysUntilAFrameIsOnScreen() {
        let (visit, surface) = visit()
        #expect(visit.live)
        #expect(!visit.shown)
        surface.signal?(.quiet)
        #expect(!visit.shown, "ready is not a picture")
        surface.signal?(.playing)
        #expect(visit.shown)
        #expect(visit.muted, "sound is off at every start")
    }

    @Test func soundTogglesThroughThePage() {
        let (visit, surface) = visit()
        visit.toggleSound()
        visit.toggleSound()
        #expect(surface.controls == [.unmute, .mute])
    }

    @Test func nothingPlayingInTimeLeavesTheBannerAndLetsGo() async throws {
        let (visit, surface) = visit(load: .milliseconds(40))
        try await until { !visit.live }
        #expect(!visit.live)
        #expect(!visit.shown)
        #expect(surface.released)
    }

    @Test func anErrorLetsGoAtOnce() {
        let (visit, surface) = visit()
        surface.signal?(.failed(150))
        #expect(!visit.live)
        #expect(surface.released)
        #expect(visit.failure == 150)
    }

    @Test func theBannerReturnsBeforeTheEndAndTheViewIsReleasedUnderIt() async throws {
        let (visit, surface) = visit()
        surface.signal?(.playing)
        surface.signal?(.ending)
        #expect(!visit.shown)
        #expect(visit.live, "kept while the banner fades back")
        try await until { !visit.live }
        #expect(!visit.live)
        #expect(surface.released)
        #expect(surface.controls.isEmpty, "never paused: no end screen to show")
    }

    @Test func scrollingAwayFadesFirstThenPauses() async throws {
        let (visit, surface) = visit(fade: .milliseconds(60))
        surface.signal?(.playing)
        visit.scrolledAway(true)
        #expect(!visit.shown)
        #expect(surface.controls.isEmpty, "the banner is fading back over a playing video")
        try await until { !surface.controls.isEmpty }
        #expect(surface.controls == [.pause])
    }

    @Test func scrollingBackPlaysAgainAndFadesInOnlyOnAPicture() async throws {
        let (visit, surface) = visit(fade: .milliseconds(20))
        surface.signal?(.playing)
        visit.scrolledAway(true)
        try await until { surface.controls == [.pause] }
        visit.scrolledAway(false)
        #expect(surface.controls == [.pause, .play])
        #expect(!visit.shown)
        surface.signal?(.playing)
        #expect(visit.shown)
    }

    @Test func scrollingBackBeforeThePauseLandedKeepsTheVideoPlaying() async throws {
        let (visit, surface) = visit(fade: .milliseconds(150))
        surface.signal?(.playing)
        visit.scrolledAway(true)
        visit.scrolledAway(false)
        #expect(visit.shown)
        try await Task.sleep(for: .milliseconds(500))
        #expect(surface.controls.isEmpty)
    }

    @Test func awayForTooLongReleasesTheViewForTheRestOfTheVisit() async throws {
        let (visit, surface) = visit(away: .milliseconds(60), fade: .milliseconds(10))
        surface.signal?(.playing)
        visit.scrolledAway(true)
        try await until { !visit.live }
        #expect(!visit.live)
        #expect(surface.released)
        visit.begin { _ in FakeSurface() }
        #expect(!visit.live, "it does not restart on this visit")
    }

    @Test func leavingReleasesAtOnce() {
        let (visit, surface) = visit()
        surface.signal?(.playing)
        visit.release()
        #expect(!visit.live)
        #expect(!visit.shown)
        #expect(surface.released)
    }
}
