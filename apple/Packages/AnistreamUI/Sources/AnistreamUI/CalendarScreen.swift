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
        AirTime.words(date, now: now)
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
    @Namespace private var rotor
    /// The page's height, and the headings', which the columns fill between them at least.
    @State private var page: CGFloat = 0
    @State private var heading: CGFloat = 0

    var body: some View {
        let days = weekDays(offset: navigator.calendarWeek)
        let broadcasts = store.broadcasts(mine: navigator.calendarMine)
        let now = Date.now
        ListStates(state: store.calendar.list.state, placeholder: AnyView(Color.clear)) { _ in
            // One page that scrolls, the days' headings pinned above it: each column as long as
            // its day, and every one as long as the longest, or the window when that is longer.
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                    Section {
                        HStack(alignment: .top, spacing: 4) {
                            ForEach(days, id: \.self) { day in
                                DayBroadcasts(
                                    day: day,
                                    broadcasts: broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) },
                                    now: now,
                                    minHeight: max(0, page - heading)
                                )
                            }
                        }
                        .fixedSize(horizontal: false, vertical: true)
                    } header: {
                        HStack(alignment: .top, spacing: 4) {
                            ForEach(days, id: \.self) { day in
                                DayHeading(day: day, rotor: rotor)
                            }
                        }
                        .background(.ground)
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { heading = $0 }
                    }
                }
                .padding(EdgeInsets(top: 4, leading: 12, bottom: 12, trailing: 16))
            }
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { page = $0 - 16 }
            .daysRotor(days, broadcasts: broadcasts, in: rotor)
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

/// A day's heading over its column: its name and date, today marked, on the raised colour that
/// runs down today's column.
struct DayHeading: View {
    let day: Date
    /// Where the Days rotor finds this heading.
    let rotor: Namespace.ID

    var body: some View {
        let today = Calendar.current.isDateInToday(day)
        VStack(alignment: .leading, spacing: 0) {
            Text(CalendarWords.weekday(day))
                .font(.mac(13, weight: today ? .bold : .semibold))
                .foregroundStyle(.silk)
            // Today beside the date, or beneath it in a column too narrow for both.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) { dayMonth; if today { todayMark } }
                VStack(alignment: .leading, spacing: 0) { dayMonth; if today { todayMark } }
            }
            .font(.mac(12))
            .lineLimit(1)
        }
        .macLineHeight(16)
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(.faintRule).frame(height: 1) }
        .background(
            today ? Palette.raised.opacity(0.6) : .clear,
            in: UnevenRoundedRectangle(topLeadingRadius: 12, topTrailingRadius: 12)
        )
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
        .accessibilityRotorEntry(id: day, in: rotor)
    }

    private var dayMonth: some View {
        Text(CalendarWords.dayMonth(day)).foregroundStyle(.haze)
    }

    private var todayMark: some View {
        HStack(spacing: 4) {
            Circle().fill(.amber).frame(width: 6, height: 6)
            Text("Today").fontWeight(.medium).foregroundStyle(.silk)
        }
    }
}

/// A day's broadcasts, newest episodes with their art, in its column of the page.
struct DayBroadcasts: View {
    let day: Date
    let broadcasts: [Broadcast]
    let now: Date
    /// At least this tall: the page below the headings, so today's colour runs to its foot.
    var minHeight: CGFloat = 0

    var body: some View {
        let today = Calendar.current.isDateInToday(day)
        VStack(alignment: .leading, spacing: 4) {
            if broadcasts.isEmpty {
                Text("Nothing airing")
                    .font(.mac(12))
                    .foregroundStyle(.haze)
                    .padding(4)
            }
            ForEach(broadcasts) { broadcast in
                BroadcastCell(broadcast: broadcast, now: now)
            }
        }
        .padding(4)
        // As long as the longest day, which the row's fixed height makes every column.
        .frame(maxWidth: .infinity, minHeight: minHeight, maxHeight: .infinity, alignment: .topLeading)
        .background(
            today ? Palette.raised.opacity(0.6) : .clear,
            in: UnevenRoundedRectangle(bottomLeadingRadius: 12, bottomTrailingRadius: 12)
        )
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
                        Text(CalendarWords.time(broadcast.airs)).font(.mac(13)).monospacedDigit()
                        Spacer(minLength: 4)
                        HStack(spacing: 4) {
                            Text("Ep \(broadcast.episode)")
                            Image(systemName: "checkmark").font(.mac(10, weight: .semibold))
                        }
                        .font(.mac(12))
                    }
                    .macLineHeight(16)
                    Text(broadcast.entry.showTitle)
                        .font(.macShowTitle(size: 13))
                        .macLineHeight(17)
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
                            .font(.mac(13))
                            .monospacedDigit()
                            .macLineHeight(16)
                            .foregroundStyle(.silk)
                        Text(broadcast.entry.showTitle)
                            .font(.macShowTitle(size: 13))
                            .macLineHeight(17)
                            .foregroundStyle(.silk)
                            .lineLimit(2)
                            // Two lines, as the board has them: without this a column's stack
                            // cuts the iPad's larger type to one.
                            .fixedSize(horizontal: false, vertical: true)
                        HStack(alignment: .firstTextBaseline, spacing: 4) {
                            if broadcast.isNew(now: now) {
                                // The dot centred on the first line, where a line of one has it.
                                Text(verbatim: "X").hidden().frame(width: 6)
                                    .overlay { Circle().fill(.water).frame(width: 6, height: 6) }
                            }
                            // Two lines in a column narrower than the line, as on an iPad in
                            // portrait, rather than cut before it says when.
                            Text(broadcast.line(now: now))
                                .lineLimit(2)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .font(.mac(12))
                        .macLineHeight(16)
                        .foregroundStyle(.haze)
                    }
                }
                .padding(4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .buttonStyle(.plain)
        .pointerHover()
        .contentShape(.rect(cornerRadius: 8))
        .titleElement(broadcast.entry, value: broadcast.spoken(now: now)) { TitleActions(entry: broadcast.entry) }
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
                    .font(.mac(13))
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
