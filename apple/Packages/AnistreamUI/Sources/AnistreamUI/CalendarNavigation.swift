// What the calendar's screens share when they move through time: the range the engine reaches,
// the line for the current time, the quiet lines at each end, and the staged wait for a stretch
// that is still loading.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The first and last day the calendar reaches, as the engine says, from the start of each day.
struct CalendarBounds: Equatable {
    let floor: Date
    let ceiling: Date

    func contains(_ day: Date) -> Bool {
        let start = Calendar.current.startOfDay(for: day)
        return start >= floor && start <= ceiling
    }

    /// Whether any day of the week is inside the range.
    func touches(_ week: [Date]) -> Bool {
        week.contains { contains($0) }
    }
}

extension AppStore {
    /// The range from the engine's window; before it arrives, 90 days back and 28 ahead of today.
    var calendarBounds: CalendarBounds {
        let calendar = Calendar.current
        let now = Date.now
        let floor = self.calendar.floor.map { Date(timeIntervalSince1970: TimeInterval($0)) }
            ?? calendar.date(byAdding: .day, value: -90, to: now)!
        let ceiling = self.calendar.ceiling.map { Date(timeIntervalSince1970: TimeInterval($0)) }
            ?? calendar.date(byAdding: .day, value: 28, to: now)!
        return CalendarBounds(floor: calendar.startOfDay(for: floor), ceiling: calendar.startOfDay(for: ceiling))
    }

    /// Whether the calendar shows only the person's list: a setting the engine remembers.
    var calendarMine: Bool { settings.isOn(.calendarMine) }
}

extension Navigator {
    /// Shows another week, sliding towards the side it is on.
    func showCalendarWeek(_ week: Int) {
        guard week != calendarWeek else { return }
        calendarForward = week > calendarWeek
        withAnimation(.standard) { calendarWeek = week }
    }
}

/// The line for the current time: silk, 1 pt, with the time before it. It sits between the last
/// row that aired and the next, and is spoken as "Now, 16:42".
struct NowLine: View {
    let now: Date
    /// The label's size: 13 on phones, 11 on the grids.
    var size: CGFloat = 13
    /// The label's column, where the times of the rows stand.
    var labelWidth: CGFloat?

    var body: some View {
        let time = CalendarWords.time(now)
        HStack(spacing: 6) {
            Text(time)
                .font(.board(size, weight: .semibold))
                .monospacedDigit()
                .foregroundStyle(.silk)
                .contentTransition(.numericText())
                .animation(.standard, value: time)
                .fixedSize()
                .frame(minWidth: labelWidth, alignment: .leading)
            Rectangle().fill(.silk).frame(height: 1)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.calendarNowSpoken(time: time))
    }
}

/// A quiet line at the end of the calendar: where it stops, or what it cannot show.
struct CalendarEdgeLine: View {
    let text: String
    var size: CGFloat = 13

    init(_ text: String, size: CGFloat = 13) {
        self.text = text
        self.size = size
    }

    var body: some View {
        Text(text)
            .font(.board(size))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A stretch of the calendar that is loading: nothing for 600 ms, then `placeholder` in the real
/// layout for at least 0.3 s, one crossfade to `answer`, and under the placeholders an activity
/// line past 4 s.
struct StagedLoad<Placeholder: View, Answer: View>: View {
    let loading: Bool
    /// What the activity line says past 4 s.
    var activity: String?
    @ViewBuilder let placeholder: () -> Placeholder
    @ViewBuilder let answer: () -> Answer

    private enum Stage { case waiting, placeholders, answer }
    @State private var stage = Stage.answer
    @State private var since: ContinuousClock.Instant?
    @State private var slow = false

    var body: some View {
        Group {
            switch stage {
            case .waiting:
                answer()
            case .placeholders:
                VStack(alignment: .leading, spacing: 0) {
                    placeholder()
                    if slow, let activity {
                        Text(activity)
                            .font(.board(13))
                            .foregroundStyle(.haze)
                            .padding(.top, 8)
                            .transition(.opacity)
                    }
                }
                .transition(.opacity)
            case .answer:
                answer()
            }
        }
        .task(id: loading) { await follow() }
    }

    private func follow() async {
        if loading {
            slow = false
            stage = .waiting
            try? await Task.sleep(for: JobsModel.delay)
            guard !Task.isCancelled else { return }
            withAnimation(.easeOut(duration: 0.2)) { stage = .placeholders }
            since = .now
            try? await Task.sleep(for: .seconds(4) - JobsModel.delay)
            guard !Task.isCancelled else { return }
            withAnimation(.easeOut(duration: 0.2)) { slow = true }
        } else if stage == .placeholders {
            if let since {
                let held = ContinuousClock.now - since
                if held < .milliseconds(300) { try? await Task.sleep(for: .milliseconds(300) - held) }
            }
            guard !Task.isCancelled else { return }
            withAnimation(.standard) { stage = .answer }
        } else {
            stage = .answer
        }
    }
}

/// A page that failed to load: one line in the edge's place, with Try again.
struct CalendarFailedLine: View {
    let text: String
    let retry: () -> Void
    var size: CGFloat = 13

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            FailedLine(text, size: size)
            Button(L.actionTryAgain, action: retry)
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
