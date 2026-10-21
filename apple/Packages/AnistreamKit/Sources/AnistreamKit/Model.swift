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
