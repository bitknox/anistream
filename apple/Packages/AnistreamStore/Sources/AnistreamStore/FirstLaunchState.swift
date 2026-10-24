// Whether first launch is for this install: a new one sees it; one already in use (after an
// upgrade, say) goes straight to the app.

import AnistreamKit
import Foundation

extension AppStore {
    /// Whether this install has been used: a source to play from (a plugin, a torrent source
    /// that can play, or a joined plugin store), a signed-in tracker, or something watched.
    public var hasBeenUsed: Bool {
        let sources = providers.rows.contains { $0.kind != "torrent" || $0.usable } || !stores.stores.isEmpty
        let signedIn = accounts.trackers.contains(where: \.connected)
        let watched = !continuing.list.state.ids.isEmpty
        return sources || signedIn || watched
    }

    /// Whether the answers `hasBeenUsed` reads are in: the sources and what to continue.
    public var knowsWhetherUsed: Bool {
        switch continuing.list.state {
        case .idle, .loading: false
        case .loaded, .empty, .failed: providers.loaded
        }
    }

    /// Whether to show first launch: not once it has been seen, and not to an install already in
    /// use. Waits for the engine's answers at most `limit`, and decides on what came by then.
    public func firstLaunchDue(seen: Bool, limit: Duration = .seconds(4)) async -> Bool {
        if seen { return false }
        if continuing.list.state == .idle { loadContinue() }
        if !stores.loaded { loadStores() }
        let deadline = ContinuousClock.now + limit
        while !knowsWhetherUsed, !hasBeenUsed, ContinuousClock.now < deadline, !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(100))
        }
        return !hasBeenUsed
    }
}
