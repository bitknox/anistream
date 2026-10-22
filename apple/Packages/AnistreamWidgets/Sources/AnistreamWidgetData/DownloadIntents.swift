// The Live Activity's Pause and Resume: App Intents that run in the app, in the background, and
// act on the whole download queue.

import AppIntents
import Foundation

#if os(iOS)
/// Pauses every download that is running or waiting.
public struct PauseDownloadsIntent: LiveActivityIntent {
    public static let title: LocalizedStringResource = "Pause downloads"
    public static let isDiscoverable = false

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        DownloadControls.shared.pause?()
        return .result()
    }
}

/// Resumes every paused download.
public struct ResumeDownloadsIntent: LiveActivityIntent {
    public static let title: LocalizedStringResource = "Resume downloads"
    public static let isDiscoverable = false

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        DownloadControls.shared.resume?()
        return .result()
    }
}
#endif

/// What Pause and Resume do: the app fills these in when it starts.
@MainActor
public final class DownloadControls {
    public static let shared = DownloadControls()

    public var pause: (@MainActor () -> Void)?
    public var resume: (@MainActor () -> Void)?

    public init() {}
}
