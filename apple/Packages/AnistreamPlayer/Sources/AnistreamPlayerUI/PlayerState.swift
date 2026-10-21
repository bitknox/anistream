// What the player shows, as values: the engine's PlayerUi kept until it says otherwise, and
// what mpv says of the file.

import AnistreamKit
import Foundation

/// The rules the player's cards keep time by.
public enum PlayerTiming {
    /// Up Next's countdown, from where the ending starts. A setting once the engine has one (E8).
    public static let upNextCountdown: Double = 10
    /// Without ending data, Up Next shows this long before the end of anything over two minutes.
    public static let upNextWithoutEnding: Double = 30
    /// Episodes rolled on in a row with nobody touching anything before "Still watching?".
    public static let stillWatchingAfter = 3
    /// The skip button dims after this long.
    public static let skipDims: Duration = .seconds(8)
    /// The undo note stays for this much playback, and at most `undoPaused` of wall time.
    public static let undoPlayback: Double = 6
    public static let undoPaused: Duration = .seconds(15)
    /// The resume note stays for this much playback.
    public static let resumePlayback: Double = 6
    /// Buffering shows a spinner from here.
    public static let spinnerAfter: Duration = .seconds(1)
    /// The starting screen admits the wait from here.
    public static let startingWaits: Duration = .seconds(3)
    /// Controls hide after this long without input: touch, and the Mac.
    public static let controlsHideTouch: Duration = .seconds(3)
    public static let controlsHideMac: Duration = .seconds(2)
    /// An alert on the video stays this long.
    public static let alertStays: Duration = .seconds(4)
}

/// The playback Now Playing shows, as the engine staged it.
public struct StagedPlayback: Equatable, Sendable {
    public var id: AnilistId
    public var title: String
    public var episode: String
    public var episodeTitle: String?
}

/// The starting screen: what is being found or opened, since when.
public struct StartingScreen: Equatable, Sendable {
    public var playback: PlaybackId
    public var title: String
    public var episode: String
    public var episodeTitle: String?
    public var since: ContinuousClock.Instant
}

/// The episode on screen, for the header and Now Playing.
public struct EpisodeOnScreen: Equatable, Sendable {
    public var number: String
    public var label: String
    public var title: String?
    public var show: String
    public var translation: Translation

    /// "Episode 7 · The Lantern Road", or the label alone.
    public var line: String {
        guard let title, !title.isEmpty else { return label }
        return "\(label) · \(title)"
    }
}

/// What the menus list, whole.
public struct PlayerMenus: Equatable, Sendable {
    public var episodes: [MenuEpisode]
    public var sources: [MenuSource]
    public var translation: Translation
    public var upscaling: MenuChoices
    public var toneMapping: MenuChoices?
    public var nightMode: Bool
}

public struct SkipOffer: Equatable, Sendable {
    public var kind: SkipKind
    public var to: Double
    public var since: ContinuousClock.Instant

    /// "Skip opening".
    public var label: String { kind == .opening ? "Skip opening" : "Skip ending" }
}

public struct UndoOffer: Equatable, Sendable {
    public var kind: SkipKind
    public var undoTo: Double
    /// Where the playhead was when the note appeared, and when.
    public var from: Double?
    public var since: ContinuousClock.Instant

    /// "Skipped opening · Undo" reads this, then Undo.
    public var label: String { kind == .opening ? "Skipped opening" : "Skipped ending" }
}

public struct UpNextCard: Equatable, Sendable {
    public var next: NextEpisode
    public var auto: Bool
    public var endingAt: Double?
    /// The viewer cancelled it, for the rest of the episode.
    public var cancelled = false
    /// PlayNext has been sent, once.
    public var played = false

    /// Where the card shows and its countdown starts: the ending, or near the end of the file.
    public func showsAt(duration: Double?) -> Double? {
        if let endingAt { return endingAt }
        guard let duration, duration > 0 else { return nil }
        return duration > 120
            ? duration - PlayerTiming.upNextWithoutEnding
            : max(0, duration - PlayerTiming.upNextCountdown)
    }

    /// Seconds left on the countdown at `position`, while it runs.
    public func remaining(at position: Double, duration: Double?) -> Double? {
        guard auto, !cancelled, let start = showsAt(duration: duration), position >= start else { return nil }
        return max(0, PlayerTiming.upNextCountdown - (position - start))
    }
}

public struct StallCard: Equatable, Sendable {
    /// Seconds held when the engine said so, and when that was: the card counts up from there.
    public var seconds: UInt64
    public var since: ContinuousClock.Instant

    public func held(at now: ContinuousClock.Instant) -> Int {
        Int(seconds) + Int((now - since).components.seconds)
    }
}

public struct ResumeNote: Equatable, Sendable {
    public var position: Double

    /// "Resumed at 12:04".
    public var label: String { "Resumed at \(PlayerClock.label(position))" }
}

public struct PlayerAlert: Equatable, Sendable {
    public var text: String
    public var kind: NoticeKind
    public var since: ContinuousClock.Instant
}

/// A chapter of the file, as mpv lists it.
public struct Chapter: Equatable, Sendable {
    public var title: String
    public var time: Double
}

/// A track of the file, as mpv lists it.
public struct Track: Equatable, Sendable, Identifiable {
    public enum Kind: String, Sendable { case video, audio, sub }
    public var kind: Kind
    public var id: Int64
    public var language: String?
    public var title: String?
    public var selected: Bool
    public var external: Bool

    /// "English · Signs & Songs": the language by name, its title below.
    public var name: String {
        let language = language.flatMap { Locale.current.localizedString(forLanguageCode: $0) }
        switch (language, title) {
        case (let language?, let title?): return "\(language) · \(title)"
        case (let language?, nil): return language
        case (nil, let title?): return title
        case (nil, nil): return "Track \(id)"
        }
    }
}

/// A stretch of the seek line: opening and ending marks are drawn in water.
public struct SeekSegment: Equatable, Sendable {
    public enum Kind: Sendable { case plain, opening, ending }
    public var kind: Kind
    public var start: Double
    public var end: Double
}

/// Times as the player writes them.
public enum PlayerClock {
    /// "12:04", "1:02:09".
    public static func label(_ seconds: Double) -> String {
        let total = Int(max(0, seconds.isFinite ? seconds : 0).rounded(.down))
        let (hours, minutes, secs) = (total / 3600, total / 60 % 60, total % 60)
        return hours > 0
            ? String(format: "%d:%02d:%02d", hours, minutes, secs)
            : String(format: "%d:%02d", minutes, secs)
    }

    /// "−2:29".
    public static func remaining(_ seconds: Double) -> String { "−" + label(seconds) }
}

extension Chapter {
    /// The chapter names the engine reads as an opening or an ending, as its own rule does.
    var segment: SeekSegment.Kind {
        let name = title.trimmingCharacters(in: .whitespaces).lowercased()
        if ["op", "opening", "intro"].contains(name) || name.hasPrefix("ncop") { return .opening }
        if ["ed", "ending", "outro"].contains(name) || name.hasPrefix("nced") { return .ending }
        return .plain
    }
}

/// The seek line's stretches: openings and endings from the chapters, everything else plain.
public func seekSegments(chapters: [Chapter], duration: Double?) -> [SeekSegment] {
    guard let duration, duration > 0 else { return [] }
    let sorted = chapters.filter { $0.time < duration }.sorted { $0.time < $1.time }
    var segments: [SeekSegment] = []
    var at = 0.0
    for (index, chapter) in sorted.enumerated() {
        let kind = chapter.segment
        guard kind != .plain else { continue }
        let end = index + 1 < sorted.count ? sorted[index + 1].time : duration
        if chapter.time > at { segments.append(SeekSegment(kind: .plain, start: at, end: chapter.time)) }
        segments.append(SeekSegment(kind: kind, start: chapter.time, end: end))
        at = end
    }
    if at < duration { segments.append(SeekSegment(kind: .plain, start: at, end: duration)) }
    return segments
}
