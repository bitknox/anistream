// One visit of a hover preview's trailer: when the art gives way, when the view is let go, the
// sound carried from one trailer to the next, and when YouTube is no longer asked.

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

/// A sound preference of the test's own, in defaults no other test or the app reads.
@MainActor
private func sound() -> TrailerSound {
    let suite = "trailer-sound-\(UUID().uuidString)" // i18n-skip: a test's defaults suite
    let defaults = UserDefaults(suiteName: suite)!
    defaults.removePersistentDomain(forName: suite)
    return TrailerSound(defaults: defaults)
}

@MainActor
private func visit(
    load: Duration = .seconds(5), fade: Duration = .milliseconds(30), sound: TrailerSound = sound()
) -> (TrailerVisit, FakeSurface) {
    let visit = TrailerVisit(loadTimeout: load, fade: fade, sound: sound)
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
    @Test func theArtStaysUntilAFrameIsOnScreen() {
        let (visit, surface) = visit()
        #expect(visit.live)
        #expect(!visit.shown)
        surface.signal?(.quiet)
        #expect(!visit.shown, "ready is not a picture")
        surface.signal?(.playing)
        #expect(visit.shown)
        #expect(visit.muted, "sound is off until it is turned on")
        #expect(surface.controls.isEmpty)
    }

    @Test func soundTogglesThroughThePage() {
        let (visit, surface) = visit()
        visit.toggleSound()
        visit.toggleSound()
        #expect(surface.controls == [.unmute, .mute])
    }

    @Test func soundTurnedOnCarriesToTheNextTrailer() {
        let preference = sound()
        let (first, _) = visit(sound: preference)
        first.toggleSound()
        first.release()
        #expect(preference.on)

        let (next, surface) = visit(sound: preference)
        #expect(!next.muted, "it starts as the last one was left")
        #expect(surface.controls == [.unmute], "the page comes up with its sound")

        next.toggleSound()
        let (third, quiet) = visit(sound: preference)
        #expect(third.muted)
        #expect(quiet.controls.isEmpty, "muted is how the page starts")
    }

    @Test func nothingPlayingInTimeLeavesTheArtAndLetsGo() async throws {
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

    @Test func theArtReturnsBeforeTheEndAndTheViewIsReleasedUnderIt() async throws {
        let (visit, surface) = visit()
        surface.signal?(.playing)
        surface.signal?(.ending)
        #expect(!visit.shown)
        #expect(visit.live, "kept while the art fades back")
        try await until { !visit.live }
        #expect(!visit.live)
        #expect(surface.released)
        #expect(surface.controls.isEmpty, "never paused: no end screen to show")
    }

    @Test func captionsShowWhileThePageSaysSoAndNotAfterLetGo() {
        let (visit, surface) = visit()
        #expect(!visit.captions)
        surface.signal?(.captions(true))
        #expect(visit.captions)
        surface.signal?(.captions(false))
        #expect(!visit.captions)
        surface.signal?(.captions(true))
        visit.release()
        #expect(!visit.captions)
    }

    @Test func leavingReleasesAtOnceAndTheVisitDoesNotRestart() {
        let (visit, surface) = visit()
        surface.signal?(.playing)
        visit.release()
        #expect(!visit.live)
        #expect(!visit.shown)
        #expect(surface.released)
        visit.begin { _ in FakeSurface() }
        #expect(!visit.live, "it does not restart on this visit")
    }
}

@MainActor
struct TrailerGateTests {
    @Test func aRefusedVideoIsNeverAskedForAgain() {
        let gate = TrailerGate()
        #expect(gate.mayAsk("abc"))
        #expect(!gate.failed("abc", code: 150))
        #expect(!gate.mayAsk("abc"))
        #expect(gate.mayAsk("def"), "one owner's refusal stops only that video")
        #expect(!gate.closed)
    }

    @Test func twoRefusalsOfDifferentVideosInARowStopTrailersForTheRun() {
        let gate = TrailerGate()
        gate.failed("abc", code: 150)
        #expect(gate.failed("def", code: 101))
        #expect(gate.closed)
        #expect(!gate.mayAsk("ghi"))
        gate.played()
        #expect(!gate.mayAsk("ghi"), "nothing opens it again this run")
    }

    @Test func aTrailerPlayedBetweenRefusalsKeepsTrailersOn() {
        let gate = TrailerGate()
        gate.failed("abc", code: 150)
        gate.played()
        #expect(!gate.failed("def", code: 150))
        #expect(!gate.closed)
        #expect(gate.mayAsk("ghi"))
    }

    @Test func theSameVideoRefusedTwiceCountsOnce() {
        let gate = TrailerGate()
        gate.failed("abc", code: 150)
        #expect(!gate.failed("abc", code: 150))
        #expect(!gate.closed)
    }

    @Test func errorsThatAreNotRefusalsAskAgain() {
        let gate = TrailerGate()
        for (id, code) in [("abc", 0), ("def", 2), ("ghi", 5), ("jkl", 100)] {
            #expect(!gate.failed(id, code: code))
            #expect(gate.mayAsk(id), "\(code)")
        }
        #expect(!gate.closed)
        gate.failed("mno", code: 153)
        #expect(gate.failed("pqr", code: 152), "the app's identity refused")
    }
}
