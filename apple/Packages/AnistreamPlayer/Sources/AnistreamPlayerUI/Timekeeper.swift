// The player's time: what it reads as now, and how it waits for a while to pass. The apps run on
// the wall clock; a test runs the model on a clock of its own and moves it by hand, so what shows
// after a while is checked without waiting for it, however slow the machine running the test.

import Foundation

public struct Timekeeper: Sendable {
    /// Now, as the cards' timing reads it.
    public let now: @Sendable () -> ContinuousClock.Instant
    /// A wait of the given length, which throws once cancelled.
    public let sleep: @Sendable (Duration) async throws -> Void

    public init(
        now: @escaping @Sendable () -> ContinuousClock.Instant,
        sleep: @escaping @Sendable (Duration) async throws -> Void
    ) {
        self.now = now
        self.sleep = sleep
    }

    /// The wall clock.
    public static let wall = Timekeeper(now: { .now }, sleep: { try await Task.sleep(for: $0) })
}
