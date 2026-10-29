// The airing Live Activity on iPhone: an episode of a show you follow that airs within the hour,
// counting down to it, then "Out now". The countdown runs on the air time it carries, and the
// activity's stale date is that time, so it turns to "Out now" with no update from the app.

import AnistreamStrings
import Foundation
#if os(iOS)
import ActivityKit
#endif

/// The Live Activity's content: the episode, and when it airs.
public struct AiringActivityState: Codable, Hashable, Sendable {
    /// The AniList id of the show.
    public var titleID: Int
    /// The shown title.
    public var title: String
    public var episode: Int
    /// When it airs, by AniList's schedule.
    public var airsAt: Date
    /// When the activity started: where the countdown runs from. Never after `airsAt`.
    public var since: Date
    /// The cover, as a file name in the App Group's `widgets/live/`.
    public var coverFile: String?
    /// Whether the app has seen it air. The activity also says so once its stale date passes.
    public var out: Bool

    public init(
        titleID: Int, title: String, episode: Int, airsAt: Date, since: Date, coverFile: String? = nil,
        out: Bool = false
    ) {
        self.titleID = titleID
        self.title = title
        self.episode = episode
        self.airsAt = airsAt
        self.since = min(since, airsAt)
        self.coverFile = coverFile
        self.out = out
    }

    /// What the countdown runs over.
    public var countdown: ClosedRange<Date> { min(since, airsAt)...airsAt }

    /// Whether it is the same episode as `other`.
    public func isEpisode(of other: AiringActivityState) -> Bool {
        titleID == other.titleID && episode == other.episode
    }
}

#if os(iOS)
/// The Live Activity's type, the same in the app that starts it and the extension that draws it.
public struct AiringAttributes: ActivityAttributes {
    public typealias ContentState = AiringActivityState

    public init() {}
}
#endif

/// The activity's words.
public struct AiringActivityText: Equatable, Sendable {
    public var title: String
    /// "Episode 9".
    public var episode: String
    /// "Airs at 19:45", or once out "Aired at 19:45".
    public var time: String
    /// What VoiceOver reads.
    public var accessibilityLabel: String

    /// `out`: it has aired, as the app saw or the stale date says.
    public init(_ state: AiringActivityState, out: Bool, locale: Locale = .autoupdatingCurrent, timeZone: TimeZone = .autoupdatingCurrent) {
        var style = Date.FormatStyle(date: .omitted, time: .shortened).locale(locale)
        style.timeZone = timeZone
        let at = state.airsAt.formatted(style)
        title = state.title
        episode = L.widgetEpisodeLong(episode: state.episode)
        time = out ? L.widgetAiredAt(time: at) : L.widgetAirsAt(time: at)
        accessibilityLabel = out
            ? L.widgetA11yOutNow(title: state.title, episode: state.episode)
            : L.widgetA11yAirsAt(title: state.title, episode: state.episode, time: at)
    }

    /// What the activity says once the episode is out.
    public static var outNow: String { L.widgetOutNow }
}
