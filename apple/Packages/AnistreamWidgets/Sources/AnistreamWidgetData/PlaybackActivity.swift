// The playback's Live Activity on iPhone: the episode playing, how far in, and Play or Pause.
// The app updates it on play, pause, seek, speed and episode changes only, never on a timer: the
// progress and the time left run on the dates it carries. Names and episode numbers only, never
// an episode's title.

import AnistreamStrings
import Foundation
#if os(iOS)
import ActivityKit
#endif

/// The Live Activity's content: the episode, and the playhead at a moment.
public struct PlaybackActivityState: Codable, Hashable, Sendable {
    /// The AniList id of the title playing.
    public var titleID: Int
    /// The shown title.
    public var title: String
    /// The episode, as the player names it: "7".
    public var episode: String
    /// The cover, as a file name in the App Group's `widgets/live/`.
    public var coverFile: String?
    /// Where the playhead was at `at`, and the episode's length, in seconds.
    public var position: Double
    public var duration: Double
    /// When `position` was read.
    public var at: Date
    /// The speed it plays at: 1 is normal.
    public var rate: Double
    public var paused: Bool

    public init(
        titleID: Int, title: String, episode: String, coverFile: String? = nil, position: Double,
        duration: Double, at: Date, rate: Double = 1, paused: Bool = false
    ) {
        self.titleID = titleID
        self.title = title
        self.episode = episode
        self.coverFile = coverFile
        self.position = position
        self.duration = duration
        self.at = at
        self.rate = rate
        self.paused = paused
    }

    /// From 0 to 1, at `at`.
    public var fraction: Double {
        guard duration > 0 else { return 0 }
        return min(max(position / duration, 0), 1)
    }

    /// Seconds of the episode still to play.
    public var remaining: Double { max(0, duration - min(max(position, 0), duration)) }

    /// When the episode started and when it ends at this speed, on the clock: what the
    /// progress and the time left run on while it plays, with no update.
    public var timeline: ClosedRange<Date> {
        let rate = rate > 0 ? rate : 1
        let start = at.addingTimeInterval(-min(max(position, 0), duration) / rate)
        let end = at.addingTimeInterval(remaining / rate)
        return start...max(start, end)
    }

    /// Whether a change from `old` to this is one to show: another episode, cover, pause or
    /// speed, or a playhead more than a second from where the old one's dates put it.
    public func differs(from old: PlaybackActivityState) -> Bool {
        if titleID != old.titleID || episode != old.episode || coverFile != old.coverFile || paused != old.paused
            || rate != old.rate || title != old.title || abs(duration - old.duration) >= 1
        {
            return true
        }
        if paused { return abs(position - old.position) >= 1 }
        return abs(timeline.upperBound.timeIntervalSince(old.timeline.upperBound)) >= 1
    }
}

#if os(iOS)
/// The Live Activity's type, the same in the app that starts it and the extension that draws it.
public struct PlaybackAttributes: ActivityAttributes {
    public typealias ContentState = PlaybackActivityState

    public init() {}
}
#endif

/// The activity's words.
public struct PlaybackActivityText: Equatable, Sendable {
    public var title: String
    /// "Episode 7".
    public var episode: String
    /// While paused: "Paused · 18:20 left". While playing the time left runs in the view.
    public var pausedLine: String
    /// What VoiceOver reads.
    public var accessibilityLabel: String

    public init(_ state: PlaybackActivityState) {
        title = state.title
        episode = L.episodeNumberText(number: state.episode)
        let left = Self.clock(state.remaining / (state.rate > 0 ? state.rate : 1))
        pausedLine = L.widgetPausedLine(left: left)
        let minutes = max(1, Int((state.remaining / (state.rate > 0 ? state.rate : 1) / 60).rounded(.up)))
        accessibilityLabel = state.paused
            ? L.widgetA11yPlaybackPaused(title: state.title, episode: state.episode, minutes: minutes)
            : L.widgetA11yPlaybackPlaying(title: state.title, episode: state.episode, minutes: minutes)
    }

    /// "18:20", or "1:02:03" past an hour, as a timer shows it.
    public static func clock(_ seconds: Double) -> String {
        let total = max(0, Int(seconds.rounded(.down)))
        let hours = total / 3600, minutes = (total % 3600) / 60, rest = total % 60
        return hours > 0
            ? String(format: "%d:%02d:%02d", hours, minutes, rest)
            : String(format: "%d:%02d", minutes, rest)
    }
}
