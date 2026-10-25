// The playback Live Activity's Play and Pause: App Intents that run in the app, in the
// background, and act on the episode playing.

import AppIntents
import Foundation

#if os(iOS)
/// Pauses the episode playing.
public struct PausePlaybackIntent: LiveActivityIntent {
    public static let title: LocalizedStringResource = "Pause"
    public static let isDiscoverable = false

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        PlaybackControls.shared.pause?()
        return .result()
    }
}

/// Plays the episode paused.
public struct ResumePlaybackIntent: LiveActivityIntent {
    public static let title: LocalizedStringResource = "Play"
    public static let isDiscoverable = false

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        PlaybackControls.shared.play?()
        return .result()
    }
}
#endif

/// What Play and Pause do: the app fills these in when it starts.
@MainActor
public final class PlaybackControls {
    public static let shared = PlaybackControls()

    public var play: (@MainActor () -> Void)?
    public var pause: (@MainActor () -> Void)?

    public init() {}
}
