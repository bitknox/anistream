// What the player model's tests share: an mpv that only records, a scripted engine, and waits.

import AnistreamKit
import AnistreamPlayer
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

    func perform(_ call: MpvCall) async throws -> PlayerReport? {
        let answer = state.withLock { state -> AnistreamPlayer.MpvValue? in
            guard !state.ended else { return nil }
            state.calls.append(call)
            if case .restate(let property) = call { return state.answers[property] ?? .array([]) }
            return nil
        }
        if state.withLock({ $0.ended }) { throw MpvError.terminated }
        if case .restate(let property) = call, let answer {
            return .property(name: property, json: answer.json)
        }
        return nil
    }

    func display(_ display: DisplayInfo) async {
        state.withLock { $0.displays.append(display) }
    }

    func resized() async {}

    func end() async {
        let continuation = state.withLock { state -> AsyncStream<MpvEvent>.Continuation? in
            state.ended = true
            return state.continuation
        }
        continuation?.finish()
    }
}

/// A model on a scripted mpv, and what it sends the engine.
@MainActor
final class Rig {
    var sent: [Command] = []
    var made: [ScriptedMpv] = []
    var previews: [ScriptedMpv] = []
    var lowPower = false
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
            lowPower: { [unowned self] in lowPower })
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

    /// Open a player for `playback` and load a file into it, which mpv starts.
    func opened(
        _ playback: PlaybackId = 1, url: String = "/media/episode.mkv", options: [MpvOption] = []
    ) async throws {
        ops(playback, [.open(fullscreen: false, speed: nil, volume: nil), .load(url: url, options: options)])
        try await until("the file loaded") { self.mpv.calls.contains { if case .load = $0 { true } else { false } } }
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

/// Wait, a few milliseconds at a time, until `condition` holds; fail after two seconds.
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
