// What each widget shows at a moment, worked out from the snapshot: its state, its rows and its
// copy. The views only lay these out.

import AnistreamStrings
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

    /// The episode as a row names it, "E7", with the mark the engine gave it: "E7 · Dub". Only
    /// dubs are marked, as in the app.
    public static func episodeShort(_ item: SnapshotContinue) -> String {
        marked(L.widgetEpisodeShort(episode: item.episode), item)
    }

    /// What plays, as Continue says it: "E7 · 14 min left", "Resume E7" or "Start E4", and for a
    /// dub "E7 · Dub · 14 min left".
    public static func line(_ item: SnapshotContinue) -> String {
        let episode = episodeShort(item)
        guard item.resumes else { return L.widgetLineStart(episode: episode) }
        if let minutes = item.minutesLeft {
            return L.widgetLineLeftApple(episode: episode, left: L.timeMinutesLeft(minutes: minutes))
        }
        return L.widgetLineResume(episode: episode)
    }

    /// The large widget's one action: "Resume E7" or "Start E4", "Resume E7 · Dub" for a dub.
    public static func action(_ item: SnapshotContinue) -> String {
        item.resumes ? L.widgetActionResume(episode: episodeShort(item)) : L.widgetActionStart(episode: episodeShort(item))
    }

    /// The large widget's episode line: "Episode 7 · The Lantern Road", or for a dub
    /// "Episode 7 · Dub · The Lantern Road".
    public static func episodeLine(_ item: SnapshotContinue) -> String {
        let episode = marked(L.widgetEpisodeLong(episode: item.episode), item)
        if let name = item.episodeTitle, !name.isEmpty { return "\(episode) · \(name)" }
        return episode
    }

    /// `label` with the row's mark after it, when it has one.
    static func marked(_ label: String, _ item: SnapshotContinue) -> String {
        item.mark.map { "\(label) · \($0)" } ?? label
    }

    /// "14 min left" under the large widget's progress.
    public static func timeLeft(_ item: SnapshotContinue) -> String? {
        guard item.resumes, let minutes = item.minutesLeft else { return nil }
        return L.timeMinutesLeft(minutes: minutes)
    }

    /// What VoiceOver reads for a show.
    public static func accessibilityLabel(_ item: SnapshotContinue) -> String {
        // The mark read as the row shows it: "episode 7, Dub, 14 minutes left".
        let number = L.widgetA11yEpisode(episode: item.episode)
        let episode = item.mark.map { L.widgetA11yEpisodeMarked(episode: number, mark: $0) } ?? number
        let what: String
        if item.resumes {
            what = item.minutesLeft.map { L.widgetA11yResume(episode: episode, minutes: $0) } ?? episode
        } else {
            what = L.widgetA11yStart(episode: episode)
        }
        return L.widgetA11yRow(title: item.title, what: what)
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
        item.translation == .dub
            ? L.titleEpisodeShortDubbed(episode: item.episode) : L.widgetEpisodeShort(episode: item.episode)
    }

    /// "Episode 9", "Episode 9 · Dub".
    public static func episodeLong(_ item: SnapshotAiring) -> String {
        item.translation == .dub
            ? L.titleEpisodeLongDubbed(episode: item.episode) : L.widgetEpisodeLong(episode: item.episode)
    }

    /// What VoiceOver reads for a row.
    public static func accessibilityLabel(_ row: AiringRow, format: WidgetFormat) -> String {
        var label = L.widgetA11yAiring(
            title: row.item.title, episode: row.item.episode, time: format.time(row.item.airsAt))
        switch row.status {
        case .watched: label = L.widgetA11yThenWatched(label: label)
        case .out: label = L.widgetA11yThenOut(label: label)
        case .aired, .upcoming: break
        }
        if row.marked { label = L.widgetA11yThenList(label: label) }
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
        case 0: return L.widgetDayToday
        case 1: return L.widgetDayTomorrow
        case -1: return L.widgetDayYesterday
        case 2...6, -6 ... -2: return weekday(date)
        default: return date.formatted(style().day().month(.wide))
        }
    }

    /// "Thursday 16:30", "tomorrow 16:30".
    public func dayAndTime(_ date: Date, from now: Date) -> String {
        L.whenWeekday(day: day(date, from: now), time: time(date))
    }

    /// "Updated Sunday", "Updated yesterday".
    public func updated(_ date: Date, from now: Date) -> String {
        L.widgetUpdated(when: day(date, from: now))
    }

    /// "Episode 10 airs Thursday 16:30".
    public func airs(_ next: SnapshotNextAiring, from now: Date) -> String {
        L.episodeAirs(episode: next.episode, when: dayAndTime(next.airsAt, from: now))
    }

    /// "Next: Thursday 16:30 · Hoshikuzu no Tabibito · Episode 10".
    public func nextLine(_ next: SnapshotNextAiring, from now: Date) -> String {
        L.widgetNextLine(when: capitalized(dayAndTime(next.airsAt, from: now)), title: next.title, episode: next.episode)
    }

    /// The same, shorter, for the small size: "Next: Thursday 16:30, Hoshikuzu no Tabibito E10".
    public func nextLineShort(_ next: SnapshotNextAiring, from now: Date) -> String {
        L.widgetNextLineShort(when: capitalized(dayAndTime(next.airsAt, from: now)), title: next.title, episode: next.episode)
    }

    public func capitalized(_ text: String) -> String {
        text.prefix(1).uppercased() + text.dropFirst()
    }
}
