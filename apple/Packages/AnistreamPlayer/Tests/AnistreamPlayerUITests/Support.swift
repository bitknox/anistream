// What the player model's tests share: an mpv that only records, a scripted engine, and waits.

import AnistreamKit
import AnistreamPlayer
import CoreGraphics
import Foundation
import Synchronization
import Testing

@testable import AnistreamPlayerUI

/// An mpv that records each call and says what the test tells it to.
final class ScriptedMpv: PlayerBackend {
    struct State {
        var begun = false
        var ended = false
        var calls: [MpvCall] = []
        var displays: [DisplayInfo] = []
        var continuation: AsyncStream<MpvEvent>.Continuation?
        var answers: [String: AnistreamPlayer.MpvValue] = [:]
        /// Commands mpv says no to, by name: `sub-add` for a file it can't read.
        var refuses: Set<String> = []
    }

    let state = Mutex(State())

    var calls: [MpvCall] { state.withLock { $0.calls } }
    var begun: Bool { state.withLock { $0.begun } }
    var ended: Bool { state.withLock { $0.ended } }

    nonisolated func events() -> AsyncStream<MpvEvent> {
        let (stream, continuation) = AsyncStream.makeStream(of: MpvEvent.self)
        state.withLock { $0.continuation = continuation }
        return stream
    }

    /// mpv says `event`.
    func emit(_ event: MpvEvent) {
        _ = state.withLock { $0.continuation }?.yield(event)
    }

    func answer(_ property: String, _ value: AnistreamPlayer.MpvValue) {
        state.withLock { $0.answers[property] = value }
    }

    func begin(on layer: MpvLayer) async throws {
        state.withLock { $0.begun = true }
    }

    func perform(_ call: MpvCall) async throws -> [PlayerReport] {
        let answers = state.withLock { state -> [(String, AnistreamPlayer.MpvValue)] in
            guard !state.ended else { return [] }
            state.calls.append(call)
            switch call {
            case .restate(let property): return [(property, state.answers[property] ?? .array([]))]
            // Only what it was told to answer, as mpv has no value for the rest.
            case .sample(let properties): return properties.compactMap { p in state.answers[p].map { (p, $0) } }
            default: return []
            }
        }
        if state.withLock({ $0.ended }) { throw MpvError.terminated }
        if case .command(let words) = call, let name = words.first, state.withLock({ $0.refuses.contains(name) }) {
            throw MpvError.mpv(code: -13, call: words.joined(separator: " "))
        }
        return answers.map { .property(name: $0.0, json: $0.1.json) }
    }

    func display(_ display: DisplayInfo) async {
        state.withLock { $0.displays.append(display) }
    }

    func resized() async {}

    /// A 16 by 9 grey picture, as mpv's screenshot of the window would be.
    func snapshot() async -> CGImage? {
        let context = CGContext(
            data: nil, width: 16, height: 9, bitsPerComponent: 8, bytesPerRow: 64,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        context?.setFillColor(gray: 0.5, alpha: 1)
        context?.fill(CGRect(x: 0, y: 0, width: 16, height: 9))
        return context?.makeImage()
    }

    func end() async {
        let continuation = state.withLock { state -> AsyncStream<MpvEvent>.Continuation? in
            state.ended = true
            return state.continuation
        }
        continuation?.finish()
    }
}

/// A clock that stands still until a test moves it. The model's ticker and its waits sleep on
/// it, and wake only as `advance` passes their deadlines, so a slow or stalled machine changes
/// nothing a test sees.
final class SteppedClock: Sendable {
    private struct Sleeper {
        let id: Int
        let length: Duration
        let due: ContinuousClock.Instant
        let wake: CheckedContinuation<Void, any Error>
    }

    private struct State {
        var now = ContinuousClock.now
        var sleepers: [Sleeper] = []
        var next = 0
    }

    private let state = Mutex(State())

    var now: ContinuousClock.Instant { state.withLock { $0.now } }

    var keeper: Timekeeper {
        Timekeeper(now: { [self] in now }, sleep: { [self] in try await sleep($0) })
    }

    /// Whether something sleeps for `length`: a wait the model started, in a task of its own,
    /// which moving the clock on before it sleeps would miss.
    func sleeping(_ length: Duration) -> Bool {
        state.withLock { $0.sleepers.contains { $0.length == length } }
    }

    /// Move on by `duration`, waking every sleep it ends.
    func advance(by duration: Duration) {
        let due = state.withLock { state -> [Sleeper] in
            state.now += duration
            let now = state.now
            let due = state.sleepers.filter { $0.due <= now }
            state.sleepers.removeAll { $0.due <= now }
            return due
        }
        for sleeper in due { sleeper.wake.resume() }
    }

    private func sleep(_ duration: Duration) async throws {
        let id = state.withLock { state -> Int in
            state.next += 1
            return state.next
        }
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (wake: CheckedContinuation<Void, any Error>) in
                let cancelled = state.withLock { state -> Bool in
                    // Cancelled before it slept: the handler below found nothing to wake.
                    guard !Task.isCancelled else { return true }
                    state.sleepers.append(Sleeper(id: id, length: duration, due: state.now + duration, wake: wake))
                    return false
                }
                if cancelled { wake.resume(throwing: CancellationError()) }
            }
        } onCancel: {
            let sleeper = state.withLock { state -> Sleeper? in
                guard let index = state.sleepers.firstIndex(where: { $0.id == id }) else { return nil }
                return state.sleepers.remove(at: index)
            }
            sleeper?.wake.resume(throwing: CancellationError())
        }
    }
}

/// A model on a scripted mpv and a stepped clock, and what it sends the engine.
@MainActor
final class Rig {
    let time = SteppedClock()
    var sent: [Command] = []
    var made: [ScriptedMpv] = []
    var previews: [ScriptedMpv] = []
    var lowPower = false
    /// What Copy stats put on the clipboard, which is never the system's in a test.
    var copied: [String] = []
    private(set) var model: PlayerModel!

    init() {
        model = PlayerModel(
            send: { [unowned self] command in sent.append(command) },
            backend: { [unowned self] in
                let mpv = ScriptedMpv()
                made.append(mpv)
                return mpv
            },
            preview: { [unowned self] in
                let mpv = ScriptedMpv()
                previews.append(mpv)
                return mpv
            },
            lowPower: { [unowned self] in lowPower },
            copy: { [unowned self] text in copied.append(text) },
            time: time.keeper)
    }

    /// Let `duration` pass, and have the model look at its clock as its ticker would.
    func step(_ duration: Duration) {
        time.advance(by: duration)
        model.tick()
    }

    var mpv: ScriptedMpv { made.last! }

    /// The engine says these for `playback`.
    func ops(_ playback: PlaybackId, _ ops: [PlayerOp]) {
        model.apply(.playerOps(playback: playback, ops: ops))
    }

    func ui(_ playback: PlaybackId, _ ui: PlayerUi) {
        model.apply(.playerUi(playback: playback, ui: ui))
    }

    /// The reports sent, with their playbacks.
    var reports: [(PlaybackId, PlayerReport)] {
        sent.compactMap {
            if case .playerReport(let playback, let report) = $0 { (playback, report) } else { nil }
        }
    }

    /// The commands sent to steer playback, aimed at a playback or at whatever plays.
    var controls: [PlayerCommand] {
        sent.compactMap {
            switch $0 {
            case .player(let command), .control(_, let command): command
            default: nil
            }
        }
    }

    /// The playbacks the commands were aimed at, `nil` for whatever plays.
    var aims: [PlaybackId?] {
        sent.compactMap {
            switch $0 {
            case .player: .some(nil)
            case .control(let playback, _): .some(playback)
            default: nil
            }
        }
    }

    /// Wait until mpv has had every call the model has queued for it so far. Not with `until`:
    /// each call takes a turn on the main actor that every test running shares, which CI's
    /// cold start can hold for a second at a time.
    func handedOver() async {
        await model.handedOver()
    }

    /// Open a player for `playback` and load a file into it, which mpv starts.
    func opened(
        _ playback: PlaybackId = 1, url: String = "/media/episode.mkv", options: [MpvOption] = []
    ) async throws {
        ops(playback, [.open(fullscreen: false, speed: nil, volume: nil), .load(url: url, options: options)])
        await handedOver()
        try #require(mpv.calls.contains { if case .load = $0 { true } else { false } }, "the file loaded")
        mpv.emit(.startFile)
        mpv.emit(.fileLoaded)
        try await until("the load reported") { self.reports.contains { $0 == (playback, .fileLoaded) } }
    }

    /// mpv's playhead at `seconds`, as an observed property, once the model has taken it: its
    /// own report, not just any, since mpv's events before it may still be on their way.
    func playhead(_ seconds: Double) async throws {
        let report = PlayerReport.property(name: "time-pos", json: AnistreamPlayer.MpvValue.double(seconds).json)
        let before = reports.filter { $0.1 == report }.count
        mpv.emit(.property(name: "time-pos", value: .double(seconds)))
        try await until("time-pos \(seconds) reported") { self.reports.filter { $0.1 == report }.count > before }
    }
}

/// Give way on the main actor until `condition` holds, for work already queued there, such as a
/// sleep the stepped clock ended: turns, not a timeout, so a stalled machine cannot run it out.
@MainActor
func afterTurns(
    _ what: String, turns: Int = 100, _ condition: @MainActor () -> Bool,
    sourceLocation: SourceLocation = #_sourceLocation
) async throws {
    for _ in 0..<turns where !condition() {
        await Task.yield()
    }
    guard condition() else {
        Issue.record("\(what) never came", sourceLocation: sourceLocation)
        throw CancellationError()
    }
}

/// Wait, a few milliseconds at a time, until `condition` holds; fail after two seconds. For what
/// stays on the main actor, such as mpv's events taken: they wait there ahead of the next look,
/// so a busy main actor delays both alike. Calls the model queues for mpv go off it and back
/// once each, which a busy main actor can stretch past any timeout: `Rig.handedOver()` waits
/// for those.
@MainActor
func until(
    _ what: String, timeout: Duration = .seconds(2), _ condition: @MainActor () -> Bool,
    sourceLocation: SourceLocation = #_sourceLocation
) async throws {
    let deadline = ContinuousClock.now + timeout
    while !condition() {
        guard ContinuousClock.now < deadline else {
            Issue.record("timed out waiting for \(what)", sourceLocation: sourceLocation)
            throw CancellationError()
        }
        try await Task.sleep(for: .milliseconds(5))
    }
}
