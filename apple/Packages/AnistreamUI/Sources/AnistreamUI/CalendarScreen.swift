// The calendar: a week of broadcasts, a column a day on the Mac and iPad, a day at a time on
// iPhone.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// One broadcast: a title's episode, and when it airs or aired.
struct Broadcast: Identifiable, Equatable {
    var id: String { "\(entry.id)-\(episode)-\(airs.timeIntervalSince1970)" }
    let entry: Entry
    let episode: UInt32
    let airs: Date

    /// When it was to air before the schedule moved it later, when it moved.
    var delayedFrom: Date? {
        entry.delayedFrom.map { Date(timeIntervalSince1970: TimeInterval($0)) }
    }

    /// "Moved from 23:00" after a delay.
    var delayNote: String? {
        delayedFrom.map { L.calendarMovedFrom(time: CalendarWords.time($0)) }
    }

    /// When it airs, "~16:30" when the time is the engine's estimate.
    var clock: String {
        let time = CalendarWords.time(airs)
        return entry.airingApproximate ? L.calendarTimeApproximate(time: time) : time
    }

    /// Whether the episode line says "Dubbed": the engine marks a row as the dub's own time, or
    /// of an episode out in the dub. Only dubs are marked, and nothing else is.
    var dubbed: Bool {
        entry.airingTranslation == .dub || entry.airingDubbed
    }

    /// The mark as an episode line ends with it: " · Dubbed", or nothing. The spaces don't break,
    /// so a line that wraps in a narrow column takes its last word down with the mark rather
    /// than leaving "· Dubbed" alone on a line.
    var markSuffix: String {
        dubbed ? "\u{00A0}·\u{00A0}\(L.audioDubbed)" : ""
    }

    /// Whether you have watched this episode.
    var watched: Bool {
        (entry.progress?.done ?? 0) >= episode
    }

    /// "E8 out 1d 2h ago" once it has aired, "E9 in 1h 30m" before, with "Dubbed" last for a dub.
    func line(now: Date) -> String {
        let seconds = Int64(airs.timeIntervalSince(now))
        let number = Int(episode)
        if seconds <= 0 {
            let ago = Wording.ago(-seconds)
            return dubbed ? L.calendarLineOutDubbed(episode: number, ago: ago) : L.calendarLineOutApple(episode: number, ago: ago)
        }
        let span = Wording.countdown(seconds)
        return dubbed ? L.calendarLineInDubbed(episode: number, span: span) : L.calendarLineInApple(episode: number, span: span)
    }

    /// "Episode 8", or "Episode 8 · Dubbed" for a dub.
    var episodeLong: String {
        dubbed ? L.titleEpisodeLongDubbed(episode: Int(episode)) : L.episodeNumber(number: Int(episode))
    }

    /// "E8", or "E8 Dubbed" for a dub.
    var episodeShort: String {
        dubbed ? L.titleEpisodeShortDubbed(episode: Int(episode)) : L.episodeNumberShort(number: Int(episode))
    }

    /// Whether it has aired and is not watched: the water dot.
    func isNew(now: Date) -> Bool {
        airs <= now && !watched
    }
}

/// The calendar's words for days and times, through the system's formatters, so they follow the
/// locale and its clock (copy rule 3).
enum CalendarWords {
    static var locale: Locale { AirTime.locale }

    static func time(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(date: .omitted, time: .shortened, locale: locale))
    }

    static func weekday(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).weekday(.wide))
    }

    static func shortWeekday(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).weekday(.abbreviated))
    }

    static func dayMonth(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).day().month(.wide))
    }

    static func dayNumber(_ date: Date) -> String {
        date.formatted(Date.FormatStyle(locale: locale).day())
    }

    /// The week as the locale writes a range of days: "19–25 October 2026".
    static func range(_ week: [Date], locale: Locale = locale) -> String {
        guard let first = week.first, let last = week.last else { return "" }
        return (first..<last).formatted(Date.IntervalFormatStyle(locale: locale).day().month(.wide).year())
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

/// The week, as weeks from this one, of the day `days` after today.
func weekOffset(ofDaysAhead days: Int, now: Date = .now) -> Int {
    let thisWeek = weekDays(offset: 0, now: now)
    var calendar = Calendar(identifier: .iso8601)
    calendar.timeZone = .current
    let day = calendar.date(byAdding: .day, value: days, to: calendar.startOfDay(for: now))!
    let since = calendar.dateComponents([.day], from: thisWeek[0], to: day).day ?? 0
    return Int((Double(since) / 7).rounded(.down))
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

/// What a day's column holds when it is not simply its broadcasts.
enum ColumnKind: Equatable {
    case normal
    /// Before the floor or after the ceiling: nothing, and a line in the first of them.
    case outside(line: String?)
    /// Not loaded yet: placeholders, and the activity line in the first column.
    case loading(activity: Bool)
    /// A page failed: the line with Try again in the first column missing.
    case failed(first: Bool)
    /// My list with nothing this week: the line with the way out, in the first column.
    case mineEmpty(first: Bool)
    /// The data ends before the ceiling: a quiet line in the first day after it.
    case dataEnd(first: Bool)
}

/// The calendar on the Mac and a wide iPad: the week as seven columns, today's set apart.
struct CalendarScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Namespace private var rotor
    /// The page's height, and the headings', which the columns fill between them at least.
    @State private var page: CGFloat = 0
    @State private var heading: CGFloat = 0

    /// What the week needs asked again for: another week, a window, more rows.
    private struct Need: Hashable {
        let week: Int
        let anchor: Int64?
        let rows: Int
        let pages: UInt32
    }

    var body: some View {
        // The headings and the line for now move on each minute.
        TimelineView(.everyMinute) { context in
            content(now: context.date)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .task {
            if navigator.launchCalendarMine {
                navigator.launchCalendarMine = false
                store.setSwitch(.calendarMine, on: true)
            }
            if store.calendar.list.state == .idle { store.loadCalendar() }
        }
        .task(id: Need(week: navigator.calendarWeek, anchor: store.calendar.anchor, rows: store.calendar.rows.count, pages: store.calendar.pages + store.calendar.aheadPages)) {
            let days = weekDays(offset: navigator.calendarWeek)
            store.ensureCalendar(from: days[0], to: days[6].addingTimeInterval(86_400))
        }
    }

    private func content(now: Date) -> some View {
        let days = weekDays(offset: navigator.calendarWeek, now: now)
        let broadcasts = store.broadcasts(mine: store.calendarMine)
        let slide: AnyTransition = reduceMotion
            ? .opacity
            : .push(from: navigator.calendarForward ? .trailing : .leading)
        return ListStates(
            state: store.calendar.list.state,
            placeholder: AnyView(CalendarPlaceholder(days: days).padding(EdgeInsets(top: 4, leading: 12, bottom: 12, trailing: 16))),
            activity: "loading the calendar", // the engine job this list waits on
            failure: L.calendarLoadFailed,
            retry: { store.loadCalendar() },
            offlineInset: 12
        ) { _ in
            ZStack {
                weekView(days: days, broadcasts: broadcasts, now: now)
                    .id(navigator.calendarWeek)
                    .transition(slide)
            }
            .clipped()
        } empty: {
            weekView(days: days, broadcasts: broadcasts, now: now)
        }
    }

    private func weekView(days: [Date], broadcasts: [Broadcast], now: Date) -> some View {
        let kinds = columnKinds(days: days, broadcasts: broadcasts)
        let bounds = store.calendarBounds
        let span = days[0]...days[6].addingTimeInterval(86_400)
        // One page that scrolls, the days' headings pinned above it: each column as long as
        // its day, and every one as long as the longest, or the window when that is longer.
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                Section {
                    HStack(alignment: .top, spacing: 4) {
                        ForEach(Array(days.enumerated()), id: \.element) { index, day in
                            DayBroadcasts(
                                day: day,
                                broadcasts: broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) },
                                now: now,
                                minHeight: max(0, page - heading),
                                kind: kinds[index],
                                retry: { store.retryCalendar(from: span.lowerBound, to: span.upperBound) },
                                showAll: { store.setSwitch(.calendarMine, on: false) }
                            )
                        }
                    }
                    .fixedSize(horizontal: false, vertical: true)
                } header: {
                    HStack(alignment: .top, spacing: 4) {
                        ForEach(days, id: \.self) { day in
                            DayHeading(day: day, now: now, outside: !bounds.contains(day), rotor: rotor)
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
    }

    /// What each day's column holds, from the range, what is loaded and what failed.
    private func columnKinds(days: [Date], broadcasts: [Broadcast]) -> [ColumnKind] {
        let bounds = store.calendarBounds
        let calendar = Calendar.current
        let model = store.calendar
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let firstDay = airs.first.map { calendar.startOfDay(for: $0) }
        let lastDay = airs.last.map { calendar.startOfDay(for: $0) }
        let anyInRange = days.contains { bounds.contains($0) }
        var firstOutside = true, firstMissing = true, firstEnd = true
        var kinds: [ColumnKind] = []
        let weekEmpty = anyInRange && model.list.state != .loading && broadcasts.allSatisfy { b in
            !days.contains { calendar.isDate(b.airs, inSameDayAs: $0) }
        }
        var mineLine = true
        for day in days {
            if !bounds.contains(day) {
                let line = day < bounds.floor ? L.calendarFloor : L.calendarCeiling
                // The line sits in the first day of the run: the first after the floor's run
                // reads left to right, and the ceiling's run starts at its first day.
                let isFirst = day < bounds.floor ? day == days.first : (firstOutside && day > bounds.ceiling)
                if day > bounds.ceiling { firstOutside = false }
                kinds.append(.outside(line: isFirst ? line : nil))
                continue
            }
            let missing = ((model.morePast && (firstDay.map { day < $0 } ?? true))
                || (model.moreAhead && (lastDay.map { day > $0 } ?? true)))
            if missing {
                let failed = (model.pastFailed && firstDay.map { day < $0 } == true)
                    || (model.aheadFailed && lastDay.map { day > $0 } == true)
                if failed {
                    kinds.append(.failed(first: firstMissing))
                } else {
                    kinds.append(.loading(activity: firstMissing))
                }
                firstMissing = false
                continue
            }
            if !model.moreAhead, let lastDay, day > lastDay, model.list.state != .loading {
                kinds.append(.dataEnd(first: firstEnd))
                firstEnd = false
                continue
            }
            if weekEmpty, store.calendarMine {
                kinds.append(.mineEmpty(first: mineLine))
                mineLine = false
                continue
            }
            kinds.append(.normal)
        }
        return kinds
    }
}

/// A day's heading over its column: its name and date, today marked with the word, on the
/// raised colour that runs down today's column. Days outside the range are in haze.
struct DayHeading: View {
    let day: Date
    var now: Date = .now
    var outside = false
    /// Where the Days rotor finds this heading.
    let rotor: Namespace.ID

    var body: some View {
        let today = Calendar.current.isDate(day, inSameDayAs: now)
        VStack(alignment: .leading, spacing: 0) {
            Text(today ? L.calendarToday : CalendarWords.weekday(day))
                .font(.mac(13, weight: today ? .bold : .semibold))
                .foregroundStyle(outside ? .haze : .silk)
            // Today names its weekday beside the date, or beneath it in a column too narrow.
            Text(today ? "\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))" : CalendarWords.dayMonth(day))
                .font(.mac(12))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .foregroundStyle(.haze)
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
}

/// A day's broadcasts, newest episodes with their art, in its column of the page.
struct DayBroadcasts: View {
    let day: Date
    let broadcasts: [Broadcast]
    let now: Date
    /// At least this tall: the page below the headings, so today's colour runs to its foot.
    var minHeight: CGFloat = 0
    var kind: ColumnKind = .normal
    var retry: () -> Void = {}
    var showAll: () -> Void = {}

    var body: some View {
        let today = Calendar.current.isDate(day, inSameDayAs: now)
        VStack(alignment: .leading, spacing: 4) {
            switch kind {
            case .normal:
                normal(today: today)
            case .outside(let line):
                // Empty, with one line that runs on over the days beside it.
                Color.clear.frame(height: 20)
                    .overlay(alignment: .topLeading) {
                        if let line {
                            CalendarEdgeLine(line, size: 12)
                                .lineLimit(1)
                                .fixedSize()
                                .padding(4)
                        }
                    }
            case .loading(let activity):
                StagedLoad(loading: true, activity: nil) {
                    CalendarColumnPlaceholder()
                } answer: {
                    Color.clear.frame(height: 20)
                }
                .overlay(alignment: .topLeading) {
                    if activity {
                        // The activity line, past 4 s, is this one.
                        CalendarSlowLine(text: L.calendarLoadingWeek)
                    }
                }
            case .failed(let first):
                if first {
                    CalendarFailedLine(text: L.calendarLoadWeekFailed, retry: retry, size: 12)
                        .fixedSize()
                        .padding(4)
                } else {
                    Color.clear.frame(height: 20)
                }
            case .mineEmpty(let first):
                if first {
                    VStack(alignment: .leading, spacing: 8) {
                        CalendarEdgeLine(L.calendarEmptyMine, size: 12).fixedSize()
                        Button(L.calendarShowAll, action: showAll)
                            .buttonStyle(SecondaryCapsuleStyle(height: 28, fontSize: 13))
                    }
                    .padding(4)
                } else {
                    Color.clear.frame(height: 20)
                }
            case .dataEnd(let first):
                if first {
                    CalendarEdgeLine(L.calendarEnd, size: 12).fixedSize().padding(4)
                } else {
                    Color.clear.frame(height: 20)
                }
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

    /// The day's broadcasts, with the line for now between the last that aired and the next.
    @ViewBuilder
    private func normal(today: Bool) -> some View {
        if broadcasts.isEmpty {
            Text(L.calendarDayEmptyApple)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(4)
        }
        let split = today ? broadcasts.firstIndex { $0.airs > now } ?? broadcasts.count : -1
        ForEach(Array(broadcasts.enumerated()), id: \.element.id) { index, broadcast in
            if today, index == split, index > 0 || !broadcasts.isEmpty {
                NowLine(now: now, size: 11).frame(height: 18)
            }
            BroadcastCell(broadcast: broadcast, now: now)
        }
        if today, split == broadcasts.count, !broadcasts.isEmpty {
            NowLine(now: now, size: 11).frame(height: 18)
        }
    }
}

/// The activity line of a stretch that has loaded past 4 s, in the first column.
struct CalendarSlowLine: View {
    let text: String
    @State private var shown = false

    var body: some View {
        Text(text)
            .font(.mac(12))
            .foregroundStyle(.haze)
            .fixedSize()
            .padding(.top, 150)
            .padding(.leading, 4)
            .opacity(shown ? 1 : 0)
            .task {
                try? await Task.sleep(for: .seconds(4))
                withAnimation(.easeOut(duration: 0.2)) { shown = true }
            }
            .accessibilityHidden(!shown)
    }
}

/// A column while its week loads: a cell or two where the broadcasts will stand.
struct CalendarColumnPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(0..<2, id: \.self) { _ in
                VStack(alignment: .leading, spacing: 8) {
                    RoundedRectangle(cornerRadius: 6).fill(.raised).aspectRatio(16 / 9, contentMode: .fit)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("17:30").font(.mac(13)) // i18n-skip
                        Text("Natsukaze Relay").font(.macShowTitle(size: 13)) // i18n-skip
                        Text("E9 in 1h 30m").font(.mac(12)) // i18n-skip
                    }
                    .redacted(reason: .placeholder)
                    .foregroundStyle(.haze)
                }
            }
        }
        .padding(4)
        .accessibilityHidden(true)
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
                        Text(broadcast.clock).font(.mac(13)).monospacedDigit()
                        Spacer(minLength: 4)
                        HStack(spacing: 4) {
                            Text(broadcast.episodeShort)
                                .lineLimit(1)
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
                        HStack(alignment: .firstTextBaseline, spacing: 4) {
                            Text(broadcast.clock)
                                .font(.mac(13))
                                .monospacedDigit()
                                .foregroundStyle(.silk)
                            if let note = broadcast.delayNote {
                                Text(note)
                                    .font(.mac(12))
                                    .monospacedDigit()
                                    .foregroundStyle(.haze)
                                    .lineLimit(1)
                            }
                        }
                        .macLineHeight(16)
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
                            Text(broadcast.line(now: now) + broadcast.markSuffix)
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

/// The calendar's toolbar: back, Today and forward grouped, and whether it shows every airing
/// title or only your list. Today is off while this week shows.
struct CalendarToolbar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let bounds = store.calendarBounds
        let week = navigator.calendarWeek
        HStack(spacing: 8) {
            ControlGroup {
                Button(L.calendarPreviousWeek, systemImage: "chevron.left") { navigator.showCalendarWeek(week - 1) }
                    .labelStyle(.iconOnly)
                    .disabled(!bounds.touches(weekDays(offset: week - 1)))
                Button(L.calendarToday) { navigator.showCalendarWeek(0) }
                    .disabled(week == 0)
                    .help(L.calendarToday)
                Button(L.calendarNextWeek, systemImage: "chevron.right") { navigator.showCalendarWeek(week + 1) }
                    .labelStyle(.iconOnly)
                    .disabled(!bounds.touches(weekDays(offset: week + 1)))
            }
            Picker(L.filterShow, selection: Binding(get: { store.calendarMine }, set: { store.setSwitch(.calendarMine, on: $0) })) {
                Text(L.calendarFilterAllTitle).tag(false)
                Text(L.calendarFilterMineTitle).tag(true)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
        }
    }
}

/// The week while it loads, past 600 ms: the days' headings and a cell or two in each column,
/// static and redacted, where the broadcasts will stand.
struct CalendarPlaceholder: View {
    let days: [Date]

    var body: some View {
        HStack(alignment: .top, spacing: 4) {
            ForEach(Array(days.enumerated()), id: \.offset) { index, day in
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(CalendarWords.weekday(day)).font(.mac(13, weight: .semibold)).foregroundStyle(.silk)
                        Text(CalendarWords.dayMonth(day)).font(.mac(12)).foregroundStyle(.haze)
                    }
                    .macLineHeight(16)
                    .padding(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .overlay(alignment: .bottom) { Rectangle().fill(.faintRule).frame(height: 1) }
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(0..<(index % 3 == 1 ? 1 : 2), id: \.self) { _ in
                            VStack(alignment: .leading, spacing: 8) {
                                RoundedRectangle(cornerRadius: 6).fill(.raised).aspectRatio(16 / 9, contentMode: .fit)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("17:30").font(.mac(13)) // i18n-skip
                                    Text("Natsukaze Relay").font(.macShowTitle(size: 13)) // i18n-skip
                                    Text("E9 in 1h 30m").font(.mac(12)) // i18n-skip
                                }
                                .redacted(reason: .placeholder)
                                .foregroundStyle(.haze)
                            }
                        }
                    }
                    .padding(8)
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .accessibilityHidden(true)
    }
}
