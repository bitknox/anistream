// Downloads that go on after the app leaves: iOS 26's continued-processing task, which the system
// runs with its own progress line while the person is elsewhere. The window is asked for when a
// download starts, or as the app leaves with some queued; while it is open the engine lets the
// downloads run in the background, with the connection check as on screen; and it closes when
// nothing is left, when it runs out, or when the person stops it, and the torrents wait again.
// The Mac has no such window and never asks.

import AnistreamKit
import Foundation
import Observation

/// The window the system opened, as the app reports to it.
@MainActor
public protocol DownloadWindow: AnyObject {
    /// How far the downloads are: `completed` of `total` units, and a line saying what is left.
    func report(completed: Int64, total: Int64, subtitle: String)
    /// Close the window: `success` when everything finished, not when it ran out or was stopped.
    func end(success: Bool)
}

/// Whether downloads go on in the background, and what that tells the engine.
@Observable @MainActor
public final class ContinuedDownloads {
    public enum Stage: Equatable, Sendable {
        /// No window.
        case idle
        /// Asked for, not opened yet.
        case asked
        /// Open: the downloads go on, and the system shows its own progress for them.
        case running
    }

    public private(set) var stage: Stage = .idle

    /// Whether the system's own progress for the downloads is showing, or about to: the app's
    /// own Live Activity stays out of the way meanwhile.
    public var isRunning: Bool { stage != .idle }

    /// Ask the system for a window, titled `title` with `subtitle`: false when it refused. `nil`
    /// where there is no such window, as on the Mac: nothing is asked.
    @ObservationIgnored public var ask: (@MainActor (_ title: String, _ subtitle: String) -> Bool)?
    /// Tell the engine where the app is.
    @ObservationIgnored var send: (@MainActor (Command) -> Void)?
    /// Whether the episode on is playing, for the phase the app goes to when it leaves.
    @ObservationIgnored public var playing: @MainActor () -> Bool = { false }

    @ObservationIgnored private var window: (any DownloadWindow)?
    @ObservationIgnored private(set) var inForeground = true
    /// What the window has counted since it opened, by download: how far each is, nought to one.
    @ObservationIgnored private var counted: [Int64: Double] = [:]
    /// How many downloads waited at the last count, to tell a download started from one ended.
    @ObservationIgnored private var lastPending = 0

    public static let title = "Downloading episodes"

    public init() {}

    /// What waits or runs in the queue.
    static func pending(_ rows: [DownloadRow]) -> [DownloadRow] {
        rows.filter { $0.state == .queued || $0.state == .active }
    }

    static func left(_ count: Int) -> String {
        count == 1 ? "1 episode left" : "\(count) episodes left"
    }

    /// Ask for a window when there are downloads to go on and none is asked or open.
    func want(_ rows: [DownloadRow]) {
        let pending = Self.pending(rows)
        guard stage == .idle, !pending.isEmpty, let ask else { return }
        if ask(Self.title, Self.left(pending.count)) {
            stage = .asked
        }
    }

    /// The queue as the engine said it: a download started asks for a window, and an open one
    /// reports how far they are and closes once nothing is left.
    func update(_ rows: [DownloadRow]) {
        let pending = Self.pending(rows)
        defer { lastPending = pending.count }
        if stage == .idle, inForeground, pending.count > lastPending {
            want(rows)
        }
        guard stage == .running, let window else { return }
        for row in rows where counted[row.id] != nil || pending.contains(where: { $0.id == row.id }) {
            counted[row.id] = row.state == .done ? 1 : min(max(row.fraction, 0), 1)
        }
        if pending.isEmpty {
            finish(success: true)
            return
        }
        let total = Int64(counted.count) * 1_000
        let completed = Int64((counted.values.reduce(0, +) * 1_000).rounded())
        window.report(completed: min(completed, total), total: total, subtitle: Self.left(pending.count))
    }

    /// The system opened the window it was asked for.
    public func began(_ window: any DownloadWindow, rows: [DownloadRow]) {
        guard !Self.pending(rows).isEmpty else {
            window.end(success: true)
            stage = .idle
            return
        }
        self.window = window
        stage = .running
        counted = [:]
        if !inForeground { send?(.appPhase(phase: .backgroundDownloads)) }
        update(rows)
    }

    /// The system ended the window: it ran out, or the person stopped it. The downloads wait
    /// where they got to.
    public func expired() {
        finish(success: false)
    }

    /// The system would not open the window it was asked for.
    public func refused() {
        if stage == .asked { stage = .idle }
    }

    /// The app came forward, or left: the phase the engine is told, and a window asked for as it
    /// leaves with downloads queued.
    public func phase(foreground: Bool, rows: [DownloadRow]) {
        inForeground = foreground
        if foreground {
            send?(.appPhase(phase: .foreground))
        } else {
            want(rows)
            send?(.appPhase(phase: stage == .running ? .backgroundDownloads : .background(playing: playing())))
        }
    }

    /// The app is about to leave: still in front, so the system can open a window for it.
    public func leaving(rows: [DownloadRow]) {
        want(rows)
    }

    private func finish(success: Bool) {
        window?.end(success: success)
        window = nil
        counted = [:]
        let was = stage
        stage = .idle
        if was == .running, !inForeground {
            send?(.appPhase(phase: .background(playing: playing())))
        }
    }
}
