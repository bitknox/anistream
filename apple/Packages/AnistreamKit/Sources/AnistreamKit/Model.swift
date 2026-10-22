// What the model's types do in Rust beside their fields, which the bindings carry no further than
// the fields: each a port of the method it is named after, kept to the same rule, so that a
// native screen says what the terminal says.

import Foundation

extension FfiDirs {
    /// The engine's directories under one root, `config`, `data` and `cache`, as a test or the
    /// home override lays them out.
    public init(root: URL) {
        self.init(
            config: root.appending(path: "config", directoryHint: .isDirectory).path,
            data: root.appending(path: "data", directoryHint: .isDirectory).path,
            cache: root.appending(path: "cache", directoryHint: .isDirectory).path
        )
    }
}

extension ResumePoint {
    /// `12:30`, or `1:02:03` past the hour: the position as a viewer thinks of it.
    /// `ResumePoint::clock`.
    public var clock: String {
        let total = Int(max(position, 0))
        let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60)
        return hours > 0
            ? String(format: "%d:%02d:%02d", hours, minutes, seconds)
            : String(format: "%d:%02d", minutes, seconds)
    }
}

extension Entry {
    /// The newest episode known to be out, with how many seconds ago when that is known too.
    /// `Entry::latest_aired`.
    ///
    /// Holding episode N + 1 as next proves N is out, so the larger claim wins over a schedule row
    /// that lags; the "ago" is carried only when it belongs to the episode named.
    public var latestAired: (episode: UInt32, ago: Int64?)? {
        let proven = nextEpisode.flatMap { $0 >= 2 ? $0 - 1 : nil }
        switch (lastAired, proven) {
        case (.some(let aired), .some(let proven)) where proven > aired.episode:
            return (proven, nil)
        case (.some(let aired), _):
            return (aired.episode, aired.ago)
        case (nil, let proven):
            return proven.map { ($0, nil) }
        }
    }

    /// Whether `episode` has aired, by the title's broadcasts: out when it is no later than the
    /// newest one known to be out; not out when it is the next broadcast or after, and that
    /// broadcast is still ahead; out otherwise, as every episode of a finished title is.
    public func airing(of episode: UInt32, now: Date = .now) -> EpisodeAvailability {
        // The page's schedule dates the episode when it has it.
        if let at = detail?.schedule.first(where: { $0.episode == episode })?.airsAt {
            let left = at - Int64(now.timeIntervalSince1970)
            return left > 0 ? .upcoming(inSeconds: left) : .aired
        }
        if let aired = latestAired, episode <= aired.episode { return .aired }
        guard let next = nextEpisode, episode >= next else { return .aired }
        if episode == next {
            // The countdown ran out since the schedule came: it is out.
            if let left = nextAiringIn, left <= 0 { return .aired }
            return .upcoming(inSeconds: nextAiringIn)
        }
        return .upcoming(inSeconds: nil)
    }

    /// What progress is counted against, and whether that is the season's length. `Entry::progress_of`.
    public var progressOf: (total: UInt32, isLength: Bool)? {
        if let episodes, episodes > 0 { return (episodes, true) }
        return latestAired.map { ($0.episode, false) }
    }

    /// Fraction of the title watched, for a meter. `Entry::watched_fraction`.
    public var watchedFraction: Double {
        guard let progress, let of = progressOf else { return 0 }
        return min(max(Double(progress.done) / Double(of.total), 0), 1)
    }
}

/// Whether an episode can play yet, by its title's broadcasts.
public enum EpisodeAvailability: Equatable, Sendable {
    /// It is out, and plays.
    case aired
    /// Not out yet: it airs in `inSeconds` when the schedule dates it, which it does for the next
    /// broadcast only.
    case upcoming(inSeconds: Int64?)

    public var hasAired: Bool { self == .aired }

    /// When it airs, as the screens say it: "Saturday 18:30", or `nil` when it is out or undated.
    public func airs(now: Date = .now) -> String? {
        guard case .upcoming(let seconds?) = self else { return nil }
        return AirTime.words(now.addingTimeInterval(TimeInterval(seconds)), now: now)
    }
}

/// A broadcast's time as the screens say it (native-spec/metadata.md): "today 16:30", "tomorrow
/// 16:30", "Thursday 16:30" within six days, "29 Oct, 16:30" later. English words, day before
/// month, on the device's own clock.
public enum AirTime {
    public static let locale = Locale(identifier: "en_GB")

    public static func words(_ date: Date, now: Date = .now, calendar: Calendar = .current) -> String {
        let time = clock(date)
        if calendar.isDate(date, inSameDayAs: now) { return "today \(time)" }
        if let tomorrow = calendar.date(byAdding: .day, value: 1, to: now), calendar.isDate(date, inSameDayAs: tomorrow) {
            return "tomorrow \(time)"
        }
        let days = calendar.dateComponents([.day], from: calendar.startOfDay(for: now), to: calendar.startOfDay(for: date)).day ?? 0
        let style = Date.FormatStyle(locale: locale)
        if (0...6).contains(days) { return "\(date.formatted(style.weekday(.wide))) \(time)" }
        return "\(date.formatted(style.day().month(.abbreviated))), \(time)"
    }

    /// The time on the device's own clock: "16:30", or "4:30 PM" where it keeps twelve hours.
    public static func clock(_ date: Date) -> String {
        let twelve = DateFormatter.dateFormat(fromTemplate: "j", options: 0, locale: .current)?.contains("a") ?? false
        let style = Date.FormatStyle(locale: Locale(identifier: twelve ? "en_US" : "en_GB"))
        return date.formatted(style.hour(.defaultDigits(amPM: .abbreviated)).minute(.twoDigits))
    }

    /// "Thursday 15 October 2026", a day in full.
    public static func day(_ date: Date) -> String {
        let style = Date.FormatStyle(locale: locale)
        return "\(date.formatted(style.weekday(.wide))) \(date.formatted(style.day().month(.wide).year()))"
    }
}

/// The words for counts and spans of time: `anistream_model::wording`, so a native screen says a
/// wait or a count as the terminal does.
public enum Wording {
    /// A countdown in the largest unit that still reads at a glance: `6d 17h`, `2h 5m`, `4m`,
    /// `now` at zero. `wording::countdown`.
    public static func countdown(_ seconds: Int64) -> String {
        guard seconds > 0 else { return "now" }
        let minutes = seconds / 60
        let hours = minutes / 60
        let days = hours / 24
        if days >= 1 {
            let hoursLeft = hours % 24
            return hoursLeft > 0 && days < 7 ? "\(days)d \(hoursLeft)h" : "\(days)d"
        }
        if hours >= 1 {
            let minutesLeft = minutes % 60
            return minutesLeft > 0 ? "\(hours)h \(minutesLeft)m" : "\(hours)h"
        }
        return "\(minutes)m"
    }

    /// How long ago: `2d ago`, or `just now` under a minute. `wording::ago`.
    public static func ago(_ seconds: Int64) -> String {
        seconds < 60 ? "just now" : "\(countdown(seconds)) ago"
    }

    /// A count and its noun, singular at one: `1 episode`, `3 episodes`. `wording::plural`.
    public static func plural(_ count: Int, _ noun: String) -> String {
        count == 1 ? "1 \(noun)" : "\(count) \(noun)s"
    }
}

extension LibrarySegment {
    /// The list's name in lower case: "watching". `LibrarySegment::label`.
    public var label: String {
        switch self {
        case .watching: "watching"
        case .planning: "planning"
        case .completed: "completed"
        case .paused: "paused"
        case .dropped: "dropped"
        }
    }
}
