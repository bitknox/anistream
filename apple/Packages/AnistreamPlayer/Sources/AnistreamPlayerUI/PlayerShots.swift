// The player's states on the Fixtures app's Playing scene, for screenshots set beside the
// boards (apple/scripts/shots.sh). Testing only: the release app has none of this.

#if FIXTURES
import AnistreamKit
import AnistreamStore
import Foundation

/// A state of the player a screenshot shows.
public enum PlayerShot: String, CaseIterable, Sendable {
    /// The banner under "Starting episode 7" (T2, M4).
    case starting
    /// Playing, with the controls up (MacPlayer, PhonePlayer).
    case playing
    /// The opening's skip, with the controls hidden (PhonePlayerSkip).
    case skip
    /// Up Next counting down from the ending (MacPlayer, PhonePlayerUpNext).
    case upNext = "up-next"
    /// "Still watching?" (PhonePlayerStillWatching).
    case stillWatching = "still-watching"
    /// The stall prompt, counting up.
    case stall

    /// The launch argument that names one: `-AnistreamPlayerShot up-next`.
    public static let argument = "-AnistreamPlayerShot"

    public static func named(in arguments: [String]) -> PlayerShot? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return PlayerShot(rawValue: arguments[index + 1])
    }
}

extension PlayerModel {
    /// Play the first Continue row, the Playing scene's episode 7, and bring the player to
    /// `shot`. False when it did not get there in time.
    public func stage(_ shot: PlayerShot, store: AppStore) async -> Bool {
        guard let item = store.continuing.queue(store.titles).all.first else { return false }
        holdsCurtain = shot == .starting
        store.send(.play(id: item.id, episode: String(item.episode)))
        guard await waited(30, { self.isPresented && self.hasPicture && self.duration != nil }) else { return false }
        var there = true
        switch shot {
        case .starting:
            // The banner's art.
            try? await Task.sleep(for: .seconds(1))
        case .playing:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            pinnedControls = true
        case .skip:
            seek(to: 7)
            there = await waited(10) { self.skip != nil && self.position >= 7.5 }
            hideControls()
        case .upNext:
            seek(to: 43.5)
            there = await waited(10) { self.upNextShown && self.countdown != nil && self.position >= 44 }
            // The Mac's board has the controls up with the card, the iPhone's has them hidden.
            #if os(macOS)
            pinnedControls = true
            #else
            hideControls()
            #endif
        case .stillWatching:
            seek(to: 33)
            there = await waited(10) { self.position >= 33.5 }
            askStillWatching()
            togglePause()
            there = await waited(10) { self.paused } && there
        case .stall:
            seek(to: 33)
            there = await waited(10) { self.position >= 33.5 }
            if let playback { apply(.playerUi(playback: playback, ui: .stall(seconds: 9))) }
            hideControls()
        }
        // The fades.
        try? await Task.sleep(for: .milliseconds(700))
        return there
    }

    private func waited(_ seconds: Int, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif
