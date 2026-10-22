// The downloads window's life: asked for when a download starts or the app leaves with some
// queued, reported to while open, closed when nothing is left, when it runs out or is stopped,
// and what the engine is told at each step. The system's task is a fake throughout.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

private func row(_ id: Int64, _ state: DownloadState, _ fraction: Double = 0) -> DownloadRow {
    DownloadRow(
        id: id, anilistId: 900_001, title: "Stardust Wayfarer", episode: String(id), state: state,
        fraction: fraction, downloaded: UInt64(fraction * 1_000), total: 1_000, error: nil, path: nil)
}

@MainActor
private final class FakeWindow: DownloadWindow {
    var reports: [(completed: Int64, total: Int64, subtitle: String)] = []
    var ended: Bool?

    func report(completed: Int64, total: Int64, subtitle: String) {
        reports.append((completed, total, subtitle))
    }

    func end(success: Bool) {
        ended = success
    }
}

@MainActor
private final class Harness {
    let continued = ContinuedDownloads()
    var asked: [String] = []
    var answer = true
    var phases: [AppState] = []
    var playing = false

    init() {
        continued.ask = { [unowned self] _, subtitle in
            asked.append(subtitle)
            return answer
        }
        continued.send = { [unowned self] command in
            if case .appPhase(let phase) = command { phases.append(phase) }
        }
        continued.playing = { [unowned self] in playing }
    }
}

@MainActor
@Suite struct ContinuedDownloadsTests {
    @Test func aDownloadStartedAsksForAWindowOnce() {
        let h = Harness()
        h.continued.update([row(1, .done)])
        #expect(h.asked.isEmpty, "nothing to go on")
        h.continued.update([row(1, .done), row(2, .queued)])
        #expect(h.asked == ["1 episode left"])
        #expect(h.continued.stage == .asked)
        #expect(h.continued.isRunning, "the Live Activity stays out of the way from the ask")
        h.continued.update([row(1, .done), row(2, .active, 0.1), row(3, .queued)])
        #expect(h.asked.count == 1, "not again while one is asked for")
    }

    @Test func aRefusalLetsTheNextDownloadAskAgain() {
        let h = Harness()
        h.answer = false
        h.continued.update([row(1, .queued)])
        #expect(h.continued.stage == .idle)
        h.continued.update([row(1, .active, 0.2)])
        #expect(h.asked.count == 1, "the same downloads ask nothing new")
        h.answer = true
        h.continued.update([row(1, .active, 0.2), row(2, .queued)])
        #expect(h.asked.count == 2)
        #expect(h.continued.stage == .asked)
    }

    @Test func anOpenWindowReportsAndClosesWhenEverythingIsDone() {
        let h = Harness()
        h.continued.update([row(1, .active, 0.5), row(2, .queued)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .active, 0.5), row(2, .queued)])
        #expect(h.continued.stage == .running)
        #expect(h.phases.isEmpty, "in front, the engine is told nothing yet")
        #expect(window.reports.last?.completed == 500)
        #expect(window.reports.last?.total == 2_000)
        #expect(window.reports.last?.subtitle == "2 episodes left")

        // The app leaves: the downloads go on in the background.
        h.continued.phase(foreground: false, rows: [row(1, .active, 0.5), row(2, .queued)])
        #expect(h.phases == [.backgroundDownloads])

        h.continued.update([row(1, .done, 1), row(2, .active, 0.25)])
        #expect(window.reports.last?.completed == 1_250)
        #expect(window.reports.last?.subtitle == "1 episode left")

        h.continued.update([row(1, .done, 1), row(2, .done, 1)])
        #expect(window.ended == true)
        #expect(h.continued.stage == .idle)
        #expect(h.phases == [.backgroundDownloads, .background(playing: false)], "everything waits again")
    }

    @Test func runningOutOrBeingStoppedPausesWhereTheyGotTo() {
        let h = Harness()
        h.playing = true
        h.continued.update([row(1, .active, 0.3)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .active, 0.3)])
        h.continued.phase(foreground: false, rows: [row(1, .active, 0.3)])
        h.continued.expired()
        #expect(window.ended == false)
        #expect(h.continued.stage == .idle)
        #expect(h.phases.last == .background(playing: true), "what plays goes on; the downloads wait")
        // Nothing more is reported to a closed window.
        h.continued.update([row(1, .active, 0.4)])
        #expect(window.reports.count == 1)
    }

    @Test func leavingWithDownloadsQueuedAsksAndTheOpenedWindowLetsThemGoOn() {
        let h = Harness()
        let rows = [row(1, .queued)]
        // Queued at launch, before any window: the app leaves.
        h.continued.leaving(rows: rows)
        #expect(h.asked == ["1 episode left"])
        h.continued.phase(foreground: false, rows: rows)
        #expect(h.phases == [.background(playing: false)], "held until the window opens")
        h.continued.began(FakeWindow(), rows: rows)
        #expect(h.phases == [.background(playing: false), .backgroundDownloads])
        // Back in front: everything runs.
        h.continued.phase(foreground: true, rows: rows)
        #expect(h.phases.last == .foreground)
        #expect(h.continued.stage == .running, "the window stays open until the downloads finish")
    }

    @Test func aWindowWithNothingLeftClosesAtOnce() {
        let h = Harness()
        h.continued.update([row(1, .queued)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .done, 1)])
        #expect(window.ended == true)
        #expect(h.continued.stage == .idle)
    }

    @Test func withNoWindowToAskForNothingIsAsked() {
        let continued = ContinuedDownloads()
        continued.update([row(1, .queued)])
        continued.leaving(rows: [row(1, .queued)])
        #expect(continued.stage == .idle)
        #expect(!continued.isRunning)
    }
}
