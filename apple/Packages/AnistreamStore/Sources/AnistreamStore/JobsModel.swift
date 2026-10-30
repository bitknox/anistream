// Jobs: work the user started and is waiting on.

import AnistreamKit
import Foundation
import Observation

/// One job, from the `JobStarted` that names it to the `JobDone` that ends it.
public struct Job: Identifiable, Equatable, Sendable {
    public let id: JobId
    /// What it is doing, in the engine's words: "loading continue watching".
    public let activity: String
    public let started: ContinuousClock.Instant
    /// What it is held up on, when that is not the work itself.
    public internal(set) var note: String?
    /// Until when, when that is known, for the note to count down.
    public internal(set) var waitUntil: ContinuousClock.Instant?
}

/// The jobs running, and the ones that have run long enough to show.
///
/// The 600 ms rule, the terminal's: work earns a place on screen only once it has run that
/// long. Most loads answer well inside it, and a row that blinked in and out on every click
/// would shift the screen for nothing; past it, the wait is long enough to wonder about.
@Observable @MainActor
public final class JobsModel {
    /// How long work runs before it shows.
    public static let delay: Duration = .milliseconds(600)

    /// Every job running, oldest first.
    public private(set) var running: [Job] = []
    /// The jobs to show now, newest first.
    public private(set) var shown: [Job] = []

    @ObservationIgnored private let now: @Sendable () -> ContinuousClock.Instant
    @ObservationIgnored private let schedule: Bool
    /// The rechecks `started` scheduled that have not run yet, each under a number of its own.
    @ObservationIgnored private var rechecks: [Int: Task<Void, Never>] = [:]
    @ObservationIgnored private var lastRecheck = 0

    /// - Parameters:
    ///   - now: the clock, which a test replaces.
    ///   - schedule: whether to recheck when a job crosses the delay; a test that steps its own
    ///     clock calls `refresh()` instead.
    public init(
        now: @escaping @Sendable () -> ContinuousClock.Instant = { ContinuousClock.now },
        schedule: Bool = true
    ) {
        self.now = now
        self.schedule = schedule
    }

    /// The jobs that have run at least `delay` by `instant`, newest first.
    public func visible(at instant: ContinuousClock.Instant) -> [Job] {
        running.filter { instant - $0.started >= Self.delay }.reversed()
    }

    /// Recompute what shows, at the clock's now.
    public func refresh() {
        let visible = visible(at: now())
        if visible != shown { shown = visible }
    }

    func started(_ job: JobId, activity: String) {
        running.append(Job(id: job, activity: activity, started: now()))
        guard schedule else { return }
        lastRecheck += 1
        let recheck = lastRecheck
        rechecks[recheck] = Task { [weak self] in
            try? await Task.sleep(for: Self.delay)
            self?.refresh()
            self?.rechecks[recheck] = nil
        }
    }

    /// Wait until every recheck scheduled so far has run. What a test waits on rather than a
    /// clock: a busy main actor can hold a recheck up well past the delay, but once it has run,
    /// the job it was for has been looked at on the clock's own now.
    func rechecked() async {
        for recheck in rechecks.values {
            await recheck.value
        }
    }

    func note(_ job: JobId, note: String, wait: TimeInterval?) {
        guard let index = running.firstIndex(where: { $0.id == job }) else { return }
        running[index].note = note
        running[index].waitUntil = wait.map { now() + .milliseconds(Int64($0 * 1000)) }
        refresh()
    }

    /// The engine is gone: no job will be done.
    func clear() {
        running = []
        rechecks.values.forEach { $0.cancel() }
        rechecks = [:]
        refresh()
    }

    func done(_ job: JobId) {
        running.removeAll { $0.id == job }
        refresh()
    }
}
