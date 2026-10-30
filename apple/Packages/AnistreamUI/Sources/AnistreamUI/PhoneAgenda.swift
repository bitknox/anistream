// The iPhone's calendar: one agenda of days from the floor to the ceiling, opening on today,
// with the week as a strip above it.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The calendar on an iPhone and in a narrow window: days one under the other, past above and
/// later below, each loaded as it is reached. The strip names the week and follows the scroll.
struct PhoneCalendar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Namespace private var rotor
    @State private var position = ScrollPosition(idType: Date.self)
    /// The day at the top of the agenda, and the days on screen.
    @State private var topDay: Date?
    @State private var visible: Set<Date> = []
    /// The Monday of the week the strip shows.
    @State private var stripWeek: Date? = Self.monday(of: .now)
    /// A day asked for that is not loaded yet, to scroll to once it is.
    @State private var pending: Date?
    /// The day at the top when earlier days were asked for, to stay on once they arrive.
    @State private var holding: Date?
    @State private var opened = false

    private static func monday(of day: Date) -> Date {
        weekDays(offset: 0, now: day)[0]
    }

    var body: some View {
        TimelineView(.everyMinute) { context in
            agenda(now: context.date)
        }
        .pullToRefresh(store, .calendar)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                if !todayShown {
                    Button(L.calendarToday) { returnToToday() }
                        .transition(.opacity)
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Picker(L.filterShow, selection: Binding(get: { store.calendarMine }, set: { store.setSwitch(.calendarMine, on: $0) })) {
                    Text(L.calendarFilterAll).tag(false)
                    Text(L.calendarFilterMine).tag(true)
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }
        }
        .task {
            if navigator.launchCalendarMine {
                navigator.launchCalendarMine = false
                store.setSwitch(.calendarMine, on: true)
            }
            if store.calendar.list.state == .idle { store.loadCalendar() }
        }
    }

    /// Whether any of today is on screen.
    private var todayShown: Bool {
        visible.isEmpty || visible.contains(Calendar.current.startOfDay(for: .now))
    }

    // MARK: Agenda

    private func agenda(now: Date) -> some View {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: now)
        let bounds = store.calendarBounds
        let broadcasts = store.broadcasts(mine: store.calendarMine)
        let byDay = Dictionary(grouping: broadcasts) { calendar.startOfDay(for: $0.airs) }
        let model = store.calendar
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let first = min(airs.first.map { calendar.startOfDay(for: $0) } ?? today, today)
        let last = max(airs.last.map { calendar.startOfDay(for: $0) } ?? today, today)
        // My list with nothing from today on, once every day ahead is loaded: one line.
        let mineBare = store.calendarMine && !model.moreAhead
            && !broadcasts.contains { $0.airs >= today }
        let days = Self.days(from: max(first, bounds.floor), to: mineBare ? today : min(last, bounds.ceiling))
        let key = Need(count: days.count, first: days.first, last: days.last, mine: store.calendarMine, pages: model.pages + model.aheadPages)

        return ScrollView {
            ListStates(
                state: model.list.state,
                placeholder: AnyView(PhoneCalendarPlaceholder()),
                activity: "loading the calendar", // the engine job this list waits on
                failure: L.calendarLoadFailed,
                retry: { store.loadCalendar() }
            ) { _ in
                agendaDays(days, byDay: byDay, today: today, now: now, bounds: bounds, mineBare: mineBare)
            } empty: {
                agendaDays(days, byDay: byDay, today: today, now: now, bounds: bounds, mineBare: mineBare)
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 24)
        }
        .scrollPosition($position, anchor: .top)
        .onScrollTargetVisibilityChange(idType: Date.self, threshold: 0.1) { ids in
            visible = Set(ids)
            topDay = ids.min()
            prefetch(ids: ids, days: days, bounds: bounds)
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            strip(today: today, now: now, bounds: bounds, broadcasts: broadcasts)
        }
        .daysRotor(days, broadcasts: broadcasts, in: rotor)
        .task(id: model.list.state == .loading) { await open(today: today) }
        .onChange(of: key) { _, _ in settle(days: days, today: today) }
        .onChange(of: topDay) { _, day in
            guard let day else { return }
            let week = Self.monday(of: day)
            if week != stripWeek { withAnimation(reduceMotion ? nil : .standard) { stripWeek = week } }
        }
        .accessibilityElement(children: .contain)
    }

    private struct Need: Hashable {
        let count: Int
        let first: Date?
        let last: Date?
        let mine: Bool
        let pages: UInt32
    }

    static func days(from first: Date, to last: Date) -> [Date] {
        let calendar = Calendar.current
        var days: [Date] = []
        var day = first
        while day <= last, days.count < 200 {
            days.append(day)
            day = calendar.date(byAdding: .day, value: 1, to: day)!
        }
        return days
    }

    @ViewBuilder
    private func agendaDays(_ days: [Date], byDay: [Date: [Broadcast]], today: Date, now: Date, bounds: CalendarBounds, mineBare: Bool) -> some View {
        LazyVStack(alignment: .leading, spacing: 20) {
            top(days: days, bounds: bounds)
            ForEach(days, id: \.self) { day in
                let rows = byDay[day] ?? []
                VStack(alignment: .leading, spacing: 0) {
                    heading(day, today: today)
                        .accessibilityRotorEntry(id: day, in: rotor)
                    if day == today, mineBare {
                        VStack(alignment: .leading, spacing: 12) {
                            Text(L.calendarEmptyMineAhead).font(.ui(15)).foregroundStyle(.haze)
                            Button(L.calendarShowAll) { store.setSwitch(.calendarMine, on: false) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 36, fontSize: 15))
                        }
                        .padding(.top, 8)
                    } else if rows.isEmpty {
                        Text(L.calendarDayEmptyApple)
                            .font(.ui(15))
                            .foregroundStyle(.haze)
                            .padding(.top, 4)
                    } else {
                        let split = day == today ? rows.firstIndex { $0.airs > now } ?? rows.count : -1
                        if day == today, split == 0 { nowLine(now) }
                        ForEach(Array(rows.enumerated()), id: \.element.id) { index, broadcast in
                            if day == today, index == split, index > 0 { nowLine(now) }
                            PhoneBroadcastRow(broadcast: broadcast, now: now)
                            if index < rows.count - 1 {
                                Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 66)
                            }
                        }
                        if day == today, split == rows.count { nowLine(now) }
                    }
                }
                .id(day)
            }
            bottom(days: days, bounds: bounds, mineBare: mineBare)
        }
        .scrollTargetLayout()
        .padding(.top, 8)
    }

    /// The line for now, over the rule between two rows and taking no room.
    private func nowLine(_ now: Date) -> some View {
        Color.clear.frame(height: 0)
            .overlay { NowLine(now: now, size: 13, labelWidth: 52).frame(height: 16) }
            .padding(.vertical, 1)
    }

    private func heading(_ day: Date, today: Date) -> some View {
        let calendar = Calendar.current
        let tomorrow = calendar.date(byAdding: .day, value: 1, to: today)!
        let date = "\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))"
        return Group {
            if day == today || day == tomorrow {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    PhoneHeading(day == today ? L.calendarToday : L.dayTomorrow)
                    Text(date).font(.ui(15)).foregroundStyle(.haze)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isHeader)
            } else {
                PhoneHeading(date)
            }
        }
    }

    // MARK: Edges

    @ViewBuilder
    private func top(days: [Date], bounds: CalendarBounds) -> some View {
        let model = store.calendar
        if model.morePast {
            if model.pastFailed {
                CalendarFailedLine(text: L.calendarLoadEarlierFailed, retry: { retry() }, size: 15)
            } else {
                StagedLoad(loading: model.loadingPast, activity: L.calendarLoadingEarlier) {
                    AgendaPlaceholder()
                } answer: {
                    Color.clear.frame(height: 1)
                }
            }
        } else if let first = days.first, first <= bounds.floor {
            CalendarEdgeLine(L.calendarFloor, size: 15)
        }
    }

    @ViewBuilder
    private func bottom(days: [Date], bounds: CalendarBounds, mineBare: Bool) -> some View {
        let model = store.calendar
        if mineBare {
            EmptyView()
        } else if model.moreAhead {
            if model.aheadFailed {
                CalendarFailedLine(text: L.calendarLoadLaterFailed, retry: { retry() }, size: 15)
            } else {
                StagedLoad(loading: model.loadingAhead, activity: L.calendarLoadingLater) {
                    AgendaPlaceholder()
                } answer: {
                    Color.clear.frame(height: 1)
                }
            }
        } else if let last = days.last {
            CalendarEdgeLine(last >= bounds.ceiling ? L.calendarCeiling : L.calendarEnd, size: 15)
        }
    }

    private func retry() {
        let bounds = store.calendarBounds
        store.retryCalendar(from: bounds.floor, to: bounds.ceiling)
    }

    // MARK: Strip

    private func strip(today: Date, now: Date, bounds: CalendarBounds, broadcasts: [Broadcast]) -> some View {
        let calendar = Calendar.current
        let firstWeek = Self.monday(of: bounds.floor)
        let lastWeek = Self.monday(of: bounds.ceiling)
        var weeks: [Date] = []
        var week = firstWeek
        while week <= lastWeek, weeks.count < 40 {
            weeks.append(week)
            week = calendar.date(byAdding: .day, value: 7, to: week)!
        }
        let current = topDay.map { calendar.startOfDay(for: $0) }
        let fresh = Set(broadcasts.filter { !$0.watched }.map { calendar.startOfDay(for: $0.airs) })
        return ScrollView(.horizontal) {
            LazyHStack(spacing: 0) {
                ForEach(weeks, id: \.self) { monday in
                    HStack(spacing: 0) {
                        ForEach(0..<7, id: \.self) { offset in
                            let day = calendar.date(byAdding: .day, value: offset, to: monday)!
                            dayButton(day, today: today, current: current ?? today, inRange: bounds.contains(day), fresh: fresh.contains(day))
                                .frame(maxWidth: .infinity)
                        }
                    }
                    .containerRelativeFrame(.horizontal)
                }
            }
            .scrollTargetLayout()
        }
        .scrollTargetBehavior(.paging)
        .scrollPosition(id: $stripWeek)
        .scrollIndicators(.never)
        // Seven to a row: no larger than `.xxxLarge`, as controls are.
        .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
        .background(.ground)
    }

    private func dayButton(_ day: Date, today: Date, current: Date, inRange: Bool, fresh: Bool) -> some View {
        let selected = day == current
        let isToday = day == today
        return Button {
            go(to: day)
        } label: {
            VStack(spacing: 4) {
                Text(CalendarWords.shortWeekday(day))
                    .font(.ui(11, weight: selected || isToday ? .semibold : .medium))
                    .foregroundStyle(selected ? .amber : isToday ? .silk : .haze)
                Text(CalendarWords.dayNumber(day))
                    .font(.ui(17, weight: selected ? .semibold : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                    .frame(width: 30, height: 30)
                    // Today, while another day is current: a 1 pt silk ring round the numeral.
                    .overlay { if isToday, !selected { Circle().strokeBorder(.silk, lineWidth: 1) } }
                Circle().fill(fresh ? Palette.water : .clear).frame(width: 5, height: 5)
            }
            .frame(width: 44)
            .frame(minHeight: 64)
            .background(selected ? Palette.selection : .clear, in: .capsule)
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .disabled(!inRange)
        .opacity(inRange ? 1 : 0.32)
        .accessibilityLabel("\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    // MARK: Moving

    /// Opens on today, or the day the launch route names: with no animation.
    private func open(today: Date) async {
        guard !opened, store.calendar.list.state != .loading, store.calendar.list.state != .idle else { return }
        try? await Task.sleep(for: .milliseconds(80))
        var target = today
        if navigator.calendarOpensOn != 0 {
            target = Calendar.current.date(byAdding: .day, value: navigator.calendarOpensOn, to: today) ?? today
            navigator.calendarOpensOn = 0
        }
        position.scrollTo(id: target, anchor: .top)
        opened = true
        if target != today { return }
    }

    /// Scrolls to a day that arrived after it was asked for, and stays where the person was when
    /// earlier days come in above.
    private func settle(days: [Date], today: Date) {
        if let pending, days.contains(pending) {
            self.pending = nil
            withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(id: pending, anchor: .top) }
        } else if let holding, days.contains(holding) {
            self.holding = nil
            position.scrollTo(id: holding, anchor: .top)
        }
    }

    /// Asks for the next stretch when the loaded edge is a few days from the screen.
    private func prefetch(ids: [Date], days: [Date], bounds: CalendarBounds) {
        let model = store.calendar
        guard opened, !days.isEmpty else { return }
        let near = Set(days.prefix(3))
        let far = Set(days.suffix(3))
        if model.morePast, !model.pastFailed, !model.loadingPast, !near.isDisjoint(with: ids) {
            holding = ids.min()
            store.loadCalendarPast()
        }
        if model.moreAhead, !model.aheadFailed, !model.loadingAhead, !far.isDisjoint(with: ids) {
            store.loadCalendarAhead()
        }
    }

    /// A tap in the strip: scrolls to the day, asking for it first when it is not loaded.
    private func go(to day: Date) {
        let model = store.calendar
        let calendar = Calendar.current
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let last = airs.last.map { calendar.startOfDay(for: $0) } ?? calendar.startOfDay(for: .now)
        let first = airs.first.map { calendar.startOfDay(for: $0) } ?? calendar.startOfDay(for: .now)
        let end = calendar.date(byAdding: .day, value: 1, to: day)!
        if day > last, model.moreAhead {
            pending = day
            store.loadCalendarAhead(through: end)
        } else if day < first, model.morePast {
            pending = day
            store.loadCalendarPast(through: day)
        } else {
            withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(id: day, anchor: .top) }
        }
    }

    /// Today, from wherever: a jump to a screen short of it, then the last screen scrolled.
    private func returnToToday() {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: .now)
        guard !reduceMotion else {
            withAnimation(.standard) { position.scrollTo(id: today, anchor: .top) }
            return
        }
        let above = (topDay ?? today) > today
        let short = calendar.date(byAdding: .day, value: above ? 2 : -2, to: today)!
        position.scrollTo(id: short, anchor: .top)
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(30))
            withAnimation(.standard) { position.scrollTo(id: today, anchor: .top) }
        }
    }
}

/// Rows where an edge's next stretch will stand, static and redacted.
struct AgendaPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(0..<3, id: \.self) { _ in
                HStack(spacing: 14) {
                    Text("17:30").font(.ui(15)).frame(minWidth: 52, alignment: .leading) // i18n-skip
                    RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 50, height: 75)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Natsukaze Relay").font(.showTitle(size: 17, relativeTo: .body)) // i18n-skip
                        Text("Episode 9 in 1h 30m").font(.ui(15)) // i18n-skip
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 8)
            }
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .accessibilityHidden(true)
    }
}
#endif
