// When a widget's content changes on its own: Continue turning stale, and Airing today's rows
// moving from upcoming to out, then the day turning.

import Foundation

/// The spec's thresholds.
public enum Staleness {
    /// Continue shows "Updated …" in place of progress once its snapshot is older than this.
    public static let continueAfter: TimeInterval = 24 * 3_600

    /// The moment `snapshot`'s Continue turns stale.
    public static func continueStale(_ snapshot: WidgetSnapshot) -> Date {
        snapshot.generatedAt.addingTimeInterval(continueAfter)
    }

    /// Whether `snapshot`'s Continue is stale at `now`.
    public static func isContinueStale(_ snapshot: WidgetSnapshot, at now: Date) -> Bool {
        now > continueStale(snapshot)
    }

    /// How many days `snapshot`'s schedule covers, counting the day it was made: a week from
    /// version 2, which carries the six days after; that day alone in version 1.
    public static func scheduleDays(_ snapshot: WidgetSnapshot) -> Int {
        snapshot.version >= 2 ? 7 : 1
    }

    /// Whether `snapshot` holds the schedule of the day `now` is in. Past it, Airing today says
    /// "No schedule for today yet".
    public static func coversDay(
        _ snapshot: WidgetSnapshot, at now: Date, calendar: Calendar = .current
    ) -> Bool {
        let made = calendar.startOfDay(for: snapshot.generatedAt)
        let today = calendar.startOfDay(for: now)
        guard today >= made,
              let end = calendar.date(byAdding: .day, value: scheduleDays(snapshot), to: made)
        else { return false }
        return today < end
    }
}

/// The dates a widget's timeline has entries at, and when it asks for the next timeline.
public struct TimelinePlan: Sendable, Equatable {
    public var dates: [Date]
    /// When WidgetKit should ask again: nil for never, until the app reloads the widgets.
    public var reloadAt: Date?

    public init(dates: [Date], reloadAt: Date?) {
        self.dates = dates
        self.reloadAt = reloadAt
    }
}

public enum WidgetTimeline {
    /// Continue's: now, the moment the snapshot turns stale while that is to come, and, caught
    /// up, the moment the next episode airs. Never asked again: the app reloads the widgets
    /// whenever it writes a snapshot.
    public static func continuePlan(_ snapshot: WidgetSnapshot?, now: Date) -> TimelinePlan {
        var dates = [now]
        if let snapshot {
            let stale = Staleness.continueStale(snapshot)
            if stale > now { dates.append(stale) }
            if snapshot.continueWatching.isEmpty, let next = snapshot.nextAiring?.airsAt, next > now {
                dates.append(next)
            }
        }
        return TimelinePlan(dates: Array(Set(dates)).sorted(), reloadAt: nil)
    }

    /// How long AniList's public schedule, fetched by the extension itself, is shown before it
    /// is fetched again.
    public static let publicScheduleLifetime: TimeInterval = 3 * 3_600

    /// Airing today's: now, each broadcast of the day still to come, so its row turns from
    /// upcoming to out without the app, and the next midnight, when the day turns and the
    /// timeline is made again from the snapshot's next day. From the public schedule it is
    /// asked again sooner, so a delay shows.
    public static func airingPlan(
        _ rows: [SnapshotAiring], now: Date, publicSchedule: Bool = false, calendar: Calendar = .current
    ) -> TimelinePlan {
        let today = calendar.startOfDay(for: now)
        let midnight = calendar.date(byAdding: .day, value: 1, to: today) ?? now.addingTimeInterval(86_400)
        var dates = Set([now, midnight])
        for row in rows where row.airsAt > now && row.airsAt < midnight {
            dates.insert(row.airsAt)
        }
        var reload = midnight
        if publicSchedule { reload = min(reload, now.addingTimeInterval(publicScheduleLifetime)) }
        return TimelinePlan(dates: dates.filter { $0 <= reload }.sorted(), reloadAt: reload)
    }
}
