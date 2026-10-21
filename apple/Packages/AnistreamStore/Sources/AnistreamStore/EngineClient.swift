// The engine as the app holds it: where commands go, and one task reading what happened.

import AnistreamKit
import Foundation
import Synchronization

/// Owns the `FfiEngine` and the one task that loops over `nextEvents()`, handing each batch to
/// the main actor in the order the engine applied them. An empty batch means the engine stopped,
/// and ends the loop.
public final class EngineClient: Sendable {
    public let engine: FfiEngine
    private let reader: Mutex<Task<Void, Never>?>

    /// Start reading `engine`'s events. `deliver` is handed each batch; `stopped` is called once,
    /// after the last.
    public init(
        engine: FfiEngine,
        deliver: @escaping @MainActor @Sendable ([Event]) -> Void,
        stopped: @escaping @MainActor @Sendable () -> Void
    ) {
        self.engine = engine
        self.reader = Mutex(nil)
        let task = Task.detached(priority: .userInitiated) {
            while !Task.isCancelled {
                let batch = await engine.nextEvents()
                guard !batch.isEmpty else { break }
                await deliver(batch)
            }
            await stopped()
        }
        reader.withLock { $0 = task }
    }

    /// Hand the engine a command, and say which job it starts when it starts one. Only puts it
    /// on a channel, so the main thread may call it. `nil` too once the engine has stopped.
    @discardableResult
    public func send(_ command: Command) -> JobId? {
        engine.send(command: command)
    }

    /// Stop the engine after everything sent before, and wait while it winds down, which with
    /// torrents on takes up to a second. The reader then sees the stream end.
    public func shutdown() {
        engine.shutdown()
    }

    deinit {
        reader.withLock { $0?.cancel() }
    }
}
