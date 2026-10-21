// The calendar: a week of broadcasts, a column a day on the Mac and iPad, a day at a time on
// iPhone.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// One broadcast: a title's episode, and when it airs or aired.
struct Broadcast: Identifiable, Equatable {
    var id: String { "\(entry.id)-\(episode)-\(airs.timeIntervalSince1970)" }
    let entry: Entry
    let episode: UInt32
    let airs: Date

    /// Whether you have watched this episode.
    var watched: Bool {
        (entry.progress?.done ?? 0) >= episode
    }

    /// "Ep 8 out 1d 2h ago" once it has aired, "Ep 9 in 1h 30m" before.
    func line(now: Date) -> String {
        let seconds = Int64(airs.timeIntervalSince(now))
        return seconds <= 0
            ? "Ep \(episode) out \(Wording.ago(-seconds))"
            : "Ep \(episode) in \(Wording.countdown(seconds))"
    }

    /// Whether it has aired and is not watched: the water dot.
    func isNew(now: Date) -> Bool {
        airs <= now && !watched
    }
}

/// The calendar's words for days and times, in English and on a 24-hour clock, as the boards
/// set them.
enum CalendarWords {
    static let locale = Locale(identifier: "en_GB")

    static func time(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).hour(.twoDigits(amPM: .omitted)).minute(.twoDigits))
    }

    static func weekday(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).weekday(.wide))
    }

    static func shortWeekday(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).weekday(.abbreviated)).uppercased()
    }

    static func dayMonth(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).day().month(.wide))
    }

    static func dayNumber(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).day())
    }

    /// "19–25 October 2026", or across months "28 September – 4 October 2026".
    static func range(_ week: [Date]) -> String {
        guard let first = week.first, let last = week.last else { return "" }
        let calendar = Calendar.current
        let year = last.formatted(Date.FormatStyle(locale: locale).year())
        if calendar.isDate(first, equalTo: last, toGranularity: .month) {
            return "\(dayNumber(first))–\(dayMonth(last)) \(year)"
        }
        return "\(dayMonth(first)) – \(dayMonth(last)) \(year)"
    }

    /// "Thursday 16:30" for a time this week, "22 October 16:30" further away.
    static func weekdayTime(_ date: Date, now: Date = .now) -> String {
        let days = date.timeIntervalSince(now) / 86_400
        return days < 6 ? "\(weekday(date)) \(time(date))" : "\(dayMonth(date)) \(time(date))"
    }
}

extension AppStore {
    /// The calendar's broadcasts, oldest first, timed from the window they were loaded against.
    func broadcasts(mine: Bool) -> [Broadcast] {
        let anchor = Date(timeIntervalSince1970: TimeInterval(calendar.anchor ?? Int64(Date.now.timeIntervalSince1970)))
        return calendar.rows.compactMap { row -> Broadcast? in
            guard let airingIn = row.airingIn, let episode = row.airingEpisode else { return nil }
            // The title as it now stands: its progress may have moved since the calendar came.
            var entry = row
            if let known = titles[row.id] {
                entry.progress = known.progress
                entry.listStatus = known.listStatus
            }
            if mine, !Self.isMine(entry) { return nil }
            return Broadcast(entry: entry, episode: episode, airs: anchor.addingTimeInterval(TimeInterval(airingIn)))
        }
    }

    /// Whether a title is yours: under way here, or on your Watching or Planning list.
    static func isMine(_ entry: Entry) -> Bool {
        entry.progress != nil || entry.listStatus == .watching || entry.listStatus == .planning
    }
}

/// The days of the week `offset` weeks from this one, Monday first.
func weekDays(offset: Int, now: Date = .now) -> [Date] {
    var calendar = Calendar(identifier: .iso8601)
    calendar.timeZone = .current
    let today = calendar.startOfDay(for: now)
    let monday = calendar.date(from: calendar.dateComponents([.yearForWeekOfYear, .weekOfYear], from: today))!
    let start = calendar.date(byAdding: .day, value: offset * 7, to: monday)!
    return (0..<7).map { calendar.date(byAdding: .day, value: $0, to: start)! }
}

/// The calendar on the Mac and a wide iPad: the week as seven columns, today's set apart.
struct CalendarScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let days = weekDays(offset: navigator.calendarWeek)
        let broadcasts = store.broadcasts(mine: navigator.calendarMine)
        let now = Date.now
        ListStates(state: store.calendar.list.state, placeholder: AnyView(Color.clear)) { _ in
            HStack(alignment: .top, spacing: 4) {
                ForEach(days, id: \.self) { day in
                    DayColumn(
                        day: day,
                        broadcasts: broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) },
                        now: now
                    )
                }
            }
            .padding(EdgeInsets(top: 4, leading: 12, bottom: 12, trailing: 16))
        } empty: {
            QuietLine("Nothing airing")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .task { if store.calendar.list.state == .idle { store.loadCalendar() } }
        .onChange(of: navigator.calendarWeek) { _, week in
            // An earlier week than is loaded asks for the broadcasts before.
            guard week < 0, store.calendar.morePast, let first = days.first,
                  let earliest = broadcasts.first?.airs, earliest > first,
                  let anchor = store.calendar.anchor
            else { return }
            store.send(.loadCalendarPast(anchor: anchor, page: store.calendar.pages))
        }
    }
}

/// One day: its name and date, then each broadcast, newest episodes with their art.
struct DayColumn: View {
    let day: Date
    let broadcasts: [Broadcast]
    let now: Date

    var body: some View {
        let today = Calendar.current.isDateInToday(day)
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                Text(CalendarWords.weekday(day))
                    .font(.system(size: 13, weight: today ? .bold : .semibold))
                    .foregroundStyle(.silk)
                HStack(spacing: 8) {
                    Text(CalendarWords.dayMonth(day)).foregroundStyle(.haze)
                    if today {
                        HStack(spacing: 4) {
                            Circle().fill(.amber).frame(width: 6, height: 6)
                            Text("Today").fontWeight(.medium).foregroundStyle(.silk)
                        }
                    }
                }
                .font(.system(size: 12))
            }
            .fixedLineHeight(16)
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .bottom) { Rectangle().fill(.faintRule).frame(height: 1) }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isHeader)

            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    if broadcasts.isEmpty {
                        Text("Nothing airing")
                            .font(.system(size: 12))
                            .foregroundStyle(.haze)
                            .padding(4)
                    }
                    ForEach(broadcasts) { broadcast in
                        BroadcastCell(broadcast: broadcast, now: now)
                    }
                }
                .padding(4)
            }
            .scrollIndicators(.never)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(today ? Palette.raised.opacity(0.6) : .clear, in: .rect(cornerRadius: 12))
    }
}

/// A broadcast in a day's column: an episode to watch with its art, one watched as a quiet line.
struct BroadcastCell: View {
    @Environment(Navigator.self) private var navigator
    let broadcast: Broadcast
    let now: Date

    var body: some View {
        Button {
            navigator.openTitle(broadcast.entry.id)
        } label: {
            if broadcast.watched {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(CalendarWords.time(broadcast.airs)).font(.system(size: 13)).monospacedDigit()
                        Spacer(minLength: 4)
                        HStack(spacing: 4) {
                            Text("Ep \(broadcast.episode)")
                            Image(systemName: "checkmark").font(.system(size: 10, weight: .semibold))
                        }
                        .font(.system(size: 12))
                    }
                    .fixedLineHeight(16)
                    Text(broadcast.entry.showTitle)
                        .font(.showTitle(size: 13))
                        .fixedLineHeight(17)
                        .lineLimit(1)
                }
                .foregroundStyle(.haze)
                .padding(4)
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    BannerArt(entry: broadcast.entry)
                        .aspectRatio(16 / 9, contentMode: .fit)
                        .clipShape(.rect(cornerRadius: 6))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(CalendarWords.time(broadcast.airs))
                            .font(.system(size: 13))
                            .monospacedDigit()
                            .fixedLineHeight(16)
                            .foregroundStyle(.silk)
                        Text(broadcast.entry.showTitle)
                            .font(.showTitle(size: 13))
                            .fixedLineHeight(17)
                            .foregroundStyle(.silk)
                            .lineLimit(2)
                        HStack(spacing: 4) {
                            if broadcast.isNew(now: now) {
                                Circle().fill(.water).frame(width: 6, height: 6)
                            }
                            Text(broadcast.line(now: now))
                        }
                        .font(.system(size: 12))
                        .fixedLineHeight(16)
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                    }
                }
                .padding(4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .buttonStyle(.plain)
        .contentShape(.rect(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .contextMenu { TitleMenu(entry: broadcast.entry) }
    }
}

/// The calendar's toolbar: the week, and whether it shows your titles or every airing one.
struct CalendarToolbar: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        HStack(spacing: 8) {
            HStack(spacing: 0) {
                Button("Previous Week", systemImage: "chevron.left") { navigator.calendarWeek -= 1 }
                    .labelStyle(.iconOnly)
                Text(weekName)
                    .font(.system(size: 13))
                    .frame(minWidth: 84)
                Button("Next Week", systemImage: "chevron.right") { navigator.calendarWeek += 1 }
                    .labelStyle(.iconOnly)
            }
            Picker("Show", selection: $navigator.calendarMine) {
                Text("My List").tag(true)
                Text("All Airing").tag(false)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
        }
    }

    private var weekName: String {
        switch navigator.calendarWeek {
        case 0: "This Week"
        case -1: "Last Week"
        case 1: "Next Week"
        default: CalendarWords.range(weekDays(offset: navigator.calendarWeek))
        }
    }
}
