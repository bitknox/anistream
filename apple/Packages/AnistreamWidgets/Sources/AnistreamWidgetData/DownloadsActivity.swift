// The downloads' Live Activity on iPhone: what it shows, as the app works it out from the
// download queue, and the words it shows it in. One activity for all downloads, which changes
// state rather than stacking. Names and episode numbers only, never an episode's title.

import AnistreamStrings
import Foundation
#if os(iOS)
import ActivityKit
#endif

/// The Live Activity's content: the download it is about, and the queue around it.
public struct DownloadsActivityState: Codable, Hashable, Sendable {
    /// The AniList id of the title it is about.
    public var titleID: Int
    /// The shown title.
    public var title: String
    /// The episode, as the queue names it: "8".
    public var episode: String
    /// The finished download's id, for Play.
    public var downloadID: Int64?
    /// The cover, as a file name in the App Group's `widgets/live/`.
    public var coverFile: String?
    /// This download's bytes so far, and its size; the queue's, with several.
    public var bytesDone: Int64
    public var bytesTotal: Int64
    /// Bytes a second, over every download running.
    public var speed: Double?
    /// Seconds until this download, or the queue, is done, at that speed.
    public var secondsLeft: Int?
    /// How many episodes are downloading, waiting or paused.
    public var count: Int
    /// What the queue still has to fetch.
    public var bytesLeft: Int64
    /// Whether everything is paused.
    public var paused: Bool
    /// Whether the last download has finished, which the activity says for a few minutes.
    public var finished: Bool

    public init(
        titleID: Int, title: String, episode: String, downloadID: Int64? = nil, coverFile: String? = nil,
        bytesDone: Int64, bytesTotal: Int64, speed: Double? = nil, secondsLeft: Int? = nil, count: Int = 1,
        bytesLeft: Int64 = 0, paused: Bool = false, finished: Bool = false
    ) {
        self.titleID = titleID
        self.title = title
        self.episode = episode
        self.downloadID = downloadID
        self.coverFile = coverFile
        self.bytesDone = bytesDone
        self.bytesTotal = bytesTotal
        self.speed = speed
        self.secondsLeft = secondsLeft
        self.count = count
        self.bytesLeft = bytesLeft
        self.paused = paused
        self.finished = finished
    }

    /// From 0 to 1: this download's, or the queue's.
    public var fraction: Double {
        guard bytesTotal > 0 else { return 0 }
        return min(max(Double(bytesDone) / Double(bytesTotal), 0), 1)
    }

    /// Whether a change from `old` to this is one to show at once rather than at the next
    /// throttled update: another episode, count, pause or finish.
    public func differsInKind(from old: DownloadsActivityState) -> Bool {
        titleID != old.titleID || episode != old.episode || count != old.count || paused != old.paused
            || finished != old.finished || coverFile != old.coverFile
    }
}

#if os(iOS)
/// The Live Activity's type, the same in the app that starts it and the extension that draws it.
public struct DownloadsAttributes: ActivityAttributes {
    public typealias ContentState = DownloadsActivityState

    public init() {}
}
#endif

/// The activity's words, per the board (NativeLiveActivity).
public struct DownloadsActivityText: Equatable, Sendable {
    /// The first line: the show title, or "Downloading 3 episodes".
    public var title: String
    /// Whether `title` is a show's, set in the show-title face.
    public var titleIsShow: Bool
    public var line1: String
    public var line2: String
    /// The Dynamic Island's expanded trailing: "5 min", in water.
    public var timeLeft: String?
    /// The expanded bottom line: "412 MB of 1.1 GB · 2.4 MB/s".
    public var detail: String
    /// What VoiceOver reads.
    public var accessibilityLabel: String

    public init(_ state: DownloadsActivityState, stale: Bool = false, locale: Locale = .autoupdatingCurrent) {
        let bytes = Self.bytes(state.bytesDone, of: state.bytesTotal, locale: locale)
        let speed = state.speed.flatMap { $0 > 0 ? Self.speed($0, locale: locale) : nil }
        let left = state.secondsLeft.map { Self.minutes($0, locale: locale) }
        let percent = Int((state.fraction * 100).rounded())
        timeLeft = state.paused || state.finished || stale ? nil : left
        detail = [bytes, speed].compactMap { $0 }.joined(separator: " · ")

        if state.finished {
            title = state.title
            titleIsShow = true
            line1 = L.widgetDownloadedLine(episode: state.episode)
            line2 = L.widgetDownloadedReady
            accessibilityLabel = L.widgetA11yDownloaded(title: state.title, episode: state.episode)
            return
        }
        if state.count > 1 {
            title = L.widgetDownloadingTitle(count: state.count)
            titleIsShow = false
            line1 = state.paused
                ? L.widgetDownloadsPaused
                : L.widgetDownloadsThenMore(title: state.title, episode: state.episode, count: state.count - 1)
            let remaining = L.widgetSizeLeft(size: Self.size(state.bytesLeft, locale: locale))
            line2 = state.paused || stale ? remaining : [remaining, speed].compactMap { $0 }.joined(separator: " · ")
        } else {
            title = state.title
            titleIsShow = true
            line1 = state.paused ? L.widgetDownloadsPaused : L.widgetDownloadsEpisode(episode: state.episode)
            line2 = state.paused || stale ? bytes : [bytes, left.map { L.widgetTimeLeft(time: $0) }].compactMap { $0 }.joined(separator: " · ")
        }
        if stale && !state.paused {
            line1 = L.widgetContinuesWhenOpen
        }
        var spoken = L.widgetA11yDownloading(
            title: state.title, episode: state.episode,
            percent: state.fraction.formatted(.percent.precision(.fractionLength(0)).locale(locale)))
        if state.paused {
            spoken = L.widgetA11yThenPaused(label: spoken)
        } else if let seconds = state.secondsLeft, !stale {
            spoken = L.widgetA11yThenTimeLeft(label: spoken, time: Self.minutes(seconds, locale: locale))
        }
        if state.count > 1 { spoken = L.widgetA11yThenCount(label: spoken, count: state.count) }
        accessibilityLabel = spoken
    }

    /// "412 MB of 1.1 GB", or "412 MB" before the size is known.
    static func bytes(_ done: Int64, of total: Int64, locale: Locale) -> String {
        total > 0
            ? L.widgetBytesOf(done: size(done, locale: locale), total: size(total, locale: locale))
            : size(done, locale: locale)
    }

    /// "1.1 GB", in the decimal units Files uses.
    static func size(_ bytes: Int64, locale: Locale) -> String {
        bytes.formatted(.byteCount(style: .file, spellsOutZero: false).locale(locale))
    }

    /// "2.4 MB/s".
    static func speed(_ bytes: Double, locale: Locale) -> String {
        L.widgetSpeed(size: size(Int64(bytes), locale: locale))
    }

    /// "5 min", rounded up, at least a minute.
    static func minutes(_ seconds: Int, locale: Locale = .autoupdatingCurrent) -> String {
        let minutes = max(1, (seconds + 59) / 60)
        return Duration.seconds(minutes * 60)
            .formatted(.units(allowed: [.hours, .minutes], width: .abbreviated).locale(locale))
    }
}
