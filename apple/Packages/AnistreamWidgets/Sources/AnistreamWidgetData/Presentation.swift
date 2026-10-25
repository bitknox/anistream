// What each widget shows at a moment, worked out from the snapshot: its state, its rows and its
// copy. The views only lay these out.

import Foundation

// MARK: - Continue watching

/// What Continue watching shows.
public enum ContinueContent: Sendable, Equatable {
    /// No tracker and nothing played on this device, or no snapshot yet.
    case signedOut
    /// Signed in and caught up, with the next episode due when the snapshot knows it, and that
    /// show's art from its row in the schedule.
    case caughtUp(SnapshotNextAiring?, art: Art)
    /// What to continue. `updated` is when the snapshot was made, once it is stale: the content
    /// stays, and says when it is from in place of progress.
    case shows([SnapshotContinue], updated: Date?)
}

public enum ContinuePresentation {
    public static func content(_ snapshot: WidgetSnapshot?, at now: Date) -> ContinueContent {
        guard let snapshot else { return .signedOut }
        if !snapshot.continueWatching.isEmpty {
            let stale = Staleness.isContinueStale(snapshot, at: now)
            return .shows(snapshot.continueWatching, updated: stale ? snapshot.generatedAt : nil)
        }
        // A version 1 file does not say who is signed in: nothing to continue reads as caught up.
        if snapshot.signedIn == false { return .signedOut }
        let next = snapshot.nextAiring.flatMap { $0.airsAt > now ? $0 : nil }
        let art = next.flatMap { next in snapshot.schedule.first { $0.id == next.id }?.art } ?? Art()
        return .caughtUp(next, art: art)
    }

    /// What plays, as Continue says it: "E7 · 14 min left", "Resume E7" or "Start E4".
    public static func line(_ item: SnapshotContinue) -> String {
        guard item.resumes else { return "Start E\(item.episode)" }
        if let minutes = item.minutesLeft { return "E\(item.episode) · \(minutes) min left" }
        return "Resume E\(item.episode)"
    }

    /// The large widget's one action: "Resume E7" or "Start E4".
    public static func action(_ item: SnapshotContinue) -> String {
        item.resumes ? "Resume E\(item.episode)" : "Start E\(item.episode)"
    }

    /// The large widget's episode line: "Episode 7 · The Lantern Road".
    public static func episodeLine(_ item: SnapshotContinue) -> String {
        if let name = item.episodeTitle, !name.isEmpty { return "Episode \(item.episode) · \(name)" }
        return "Episode \(item.episode)"
    }

    /// "14 min left" under the large widget's progress.
    public static func timeLeft(_ item: SnapshotContinue) -> String? {
        guard item.resumes, let minutes = item.minutesLeft else { return nil }
        return "\(minutes) min left"
    }

    /// What VoiceOver reads for a show.
    public static func accessibilityLabel(_ item: SnapshotContinue) -> String {
        let what: String
        if item.resumes {
            what = item.minutesLeft.map { "episode \(item.episode), \($0) minutes left" } ?? "episode \(item.episode)"
        } else {
            what = "start episode \(item.episode)"
        }
        return "\(item.title), \(what)"
    }
}

// MARK: - Airing today

/// Airing today's edit-sheet choice.
public enum AiringFilter: String, Sendable, CaseIterable {
    /// Everything airing, with your list's shows marked: the default.
    case all
    /// Only your list's shows.
    case myList
}

/// One of the day's broadcasts as the widget shows it.
public struct AiringRow: Sendable, Equatable, Identifiable {
    public enum Status: Sendable, Equatable {
        /// Still to come.
        case upcoming
        /// Out, of a show in your list you have not watched it of: the water dot.
        case out
        /// Out, and watched.
        case watched
        /// Out, of a show not in your list, or with nothing marked.
        case aired
    }

    public var item: SnapshotAiring
    /// Whether it carries the bookmark and a silk title.
    public var marked: Bool
    public var status: Status

    public var id: String { item.rowID }
    public var isUpcoming: Bool { status == .upcoming }
}

/// What Airing today shows.
public enum AiringContent: Sendable, Equatable {
    /// The day's broadcasts. `marks` is whether your list's shows are marked; `tomorrow` is the
    /// next day's, when the snapshot has it.
    case rows(day: Date, rows: [AiringRow], marks: Bool, tomorrow: [SnapshotAiring]?)
    /// No broadcasts today, with the next one due when it is known.
    case nothingToday(next: SnapshotNextAiring?)
    /// No schedule for today: the snapshot's week has run out, or there is none and AniList
    /// could not be asked.
    case noSchedule
}

public enum AiringPresentation {
    /// What Airing today shows at `now`: from the snapshot while it covers today, otherwise from
    /// AniList's public schedule the extension fetched, with nothing marked.
    public static func content(
        snapshot: WidgetSnapshot?,
        publicSchedule: [SnapshotAiring]?,
        filter: AiringFilter,
        at now: Date,
        calendar: Calendar = .current
    ) -> AiringContent {
        let today = calendar.startOfDay(for: now)
        guard let tomorrowStart = calendar.date(byAdding: .day, value: 1, to: today),
              let dayAfter = calendar.date(byAdding: .day, value: 2, to: today)
        else { return .noSchedule }

        let schedule: [SnapshotAiring]
        let marks: Bool
        var tomorrow: [SnapshotAiring]?
        var next: SnapshotNextAiring?
        if let snapshot {
            guard Staleness.coversDay(snapshot, at: now, calendar: calendar) else { return .noSchedule }
            schedule = snapshot.schedule
            // Signed out, nothing is marked: the list the marks go by is the tracker's.
            marks = snapshot.signedIn != false
            let covered = calendar.date(
                byAdding: .day, value: Staleness.scheduleDays(snapshot),
                to: calendar.startOfDay(for: snapshot.generatedAt))
            if let covered, dayAfter <= covered {
                tomorrow = schedule.filter { $0.airsAt >= tomorrowStart && $0.airsAt < dayAfter }
            }
            if let due = snapshot.nextAiring, due.airsAt > now, marks { next = due }
        } else if let publicSchedule {
            schedule = publicSchedule.sorted { ($0.airsAt, $0.id) < ($1.airsAt, $1.id) }
            marks = false
        } else {
            return .noSchedule
        }

        let mine: (SnapshotAiring) -> Bool = { marks && $0.onList }
        var rows = schedule
            .filter { $0.airsAt >= today && $0.airsAt < tomorrowStart }
            .map { row(for: $0, marks: marks, at: now) }
        if filter == .myList, marks {
            rows = rows.filter(\.marked)
            tomorrow = tomorrow?.filter(mine)
        }
        if rows.isEmpty {
            let later = schedule.first { $0.airsAt >= tomorrowStart && (!marks || $0.onList) }
            let fallback = later.map { SnapshotNextAiring(id: $0.id, title: $0.title, episode: $0.episode, airsAt: $0.airsAt) }
            return .nothingToday(next: next.flatMap { $0.airsAt >= tomorrowStart ? $0 : nil } ?? fallback)
        }
        return .rows(day: today, rows: rows, marks: marks, tomorrow: tomorrow)
    }

    static func row(for item: SnapshotAiring, marks: Bool, at now: Date) -> AiringRow {
        let marked = marks && item.onList
        let status: AiringRow.Status
        if item.airsAt > now {
            status = .upcoming
        } else if marked {
            status = item.watched ? .watched : .out
        } else {
            status = .aired
        }
        return AiringRow(item: item, marked: marked, status: status)
    }

    /// The rows a size has room for, `count` of them: everything still to come, then what came
    /// out, a list show you have not watched first, the most recent first. In the order they air.
    public static func window(_ rows: [AiringRow], count: Int) -> [AiringRow] {
        guard rows.count > count else { return rows }
        let upcoming = rows.filter(\.isUpcoming)
        if upcoming.count >= count { return Array(upcoming.prefix(count)) }
        let past = rows.filter { !$0.isUpcoming }.reversed()
        let ranked = past.filter { $0.status == .out } + past.filter { $0.status != .out }
        let kept = Set(ranked.prefix(count - upcoming.count).map(\.id))
        return rows.filter { $0.isUpcoming || kept.contains($0.id) }
    }

    /// The small size's and the Lock Screen's: what is still to come, or, when nothing is, the
    /// last of the day.
    public static func comingUp(_ rows: [AiringRow], count: Int) -> [AiringRow] {
        let upcoming = rows.filter(\.isUpcoming)
        if !upcoming.isEmpty { return Array(upcoming.prefix(count)) }
        return Array(rows.suffix(count))
    }

    /// The next broadcast still to come, a marked one first: the inline accessory's.
    public static func nextUp(_ rows: [AiringRow]) -> AiringRow? {
        rows.first { $0.isUpcoming && $0.marked } ?? rows.first(where: \.isUpcoming)
    }

    /// "E9", "E9 · Dub". Only dubs are marked: a subbed episode is the usual case.
    public static func episodeShort(_ item: SnapshotAiring) -> String {
        "E\(item.episode)" + dubMark(item)
    }

    /// "Episode 9", "Episode 9 · Dub".
    public static func episodeLong(_ item: SnapshotAiring) -> String {
        "Episode \(item.episode)" + dubMark(item)
    }

    /// " · Dub" for a dub's broadcast, or nothing.
    static func dubMark(_ item: SnapshotAiring) -> String {
        item.translation == .dub ? " · Dub" : ""
    }

    /// What VoiceOver reads for a row.
    public static func accessibilityLabel(_ row: AiringRow, format: WidgetFormat) -> String {
        var label = "\(row.item.title), episode \(row.item.episode), \(format.time(row.item.airsAt))"
        switch row.status {
        case .watched: label += ", watched"
        case .out: label += ", out"
        case .aired, .upcoming: break
        }
        if row.marked { label += ", in your list" }
        return label
    }
}

// MARK: - Dates as the widgets say them

/// Times and days in the device's calendar and locale, as the widgets print them.
public struct WidgetFormat: Sendable {
    public var calendar: Calendar
    public var locale: Locale

    public init(calendar: Calendar = .autoupdatingCurrent, locale: Locale = .autoupdatingCurrent) {
        self.calendar = calendar
        self.locale = locale
    }

    private func style() -> Date.FormatStyle {
        var style = Date.FormatStyle(locale: locale, calendar: calendar, timeZone: calendar.timeZone)
        style.capitalizationContext = .standalone
        return style
    }

    /// "21:00", or "9:00 PM" where the locale says so.
    public func time(_ date: Date) -> String {
        date.formatted(style().hour().minute())
    }

    /// "Tuesday".
    public func weekday(_ date: Date) -> String {
        date.formatted(style().weekday(.wide))
    }

    /// "Tuesday 20 October".
    public func longDay(_ date: Date) -> String {
        date.formatted(style().weekday(.wide).day().month(.wide))
    }

    /// The day `date` is in, from `now`: "today", "tomorrow", "Thursday" within the week, or
    /// "12 November".
    public func day(_ date: Date, from now: Date) -> String {
        let today = calendar.startOfDay(for: now)
        let day = calendar.startOfDay(for: date)
        let days = calendar.dateComponents([.day], from: today, to: day).day ?? 0
        switch days {
        case 0: return "today"
        case 1: return "tomorrow"
        case -1: return "yesterday"
        case 2...6, -6 ... -2: return weekday(date)
        default: return date.formatted(style().day().month(.wide))
        }
    }

    /// "Thursday 16:30", "tomorrow 16:30".
    public func dayAndTime(_ date: Date, from now: Date) -> String {
        "\(day(date, from: now)) \(time(date))"
    }

    /// "Updated Sunday", "Updated yesterday".
    public func updated(_ date: Date, from now: Date) -> String {
        "Updated \(day(date, from: now))"
    }

    /// "Episode 10 airs Thursday 16:30".
    public func airs(_ next: SnapshotNextAiring, from now: Date) -> String {
        "Episode \(next.episode) airs \(dayAndTime(next.airsAt, from: now))"
    }

    /// "Next: Thursday 16:30 · Hoshikuzu no Tabibito · Episode 10".
    public func nextLine(_ next: SnapshotNextAiring, from now: Date) -> String {
        "Next: \(capitalized(dayAndTime(next.airsAt, from: now))) · \(next.title) · Episode \(next.episode)"
    }

    /// The same, shorter, for the small size: "Next: Thursday 16:30, Hoshikuzu no Tabibito E10".
    public func nextLineShort(_ next: SnapshotNextAiring, from now: Date) -> String {
        "Next: \(capitalized(dayAndTime(next.airsAt, from: now))), \(next.title) E\(next.episode)"
    }

    public func capitalized(_ text: String) -> String {
        text.prefix(1).uppercased() + text.dropFirst()
    }
}
