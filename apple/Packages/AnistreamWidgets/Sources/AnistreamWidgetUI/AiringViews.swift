// Airing today: the day's times, with the shows in your list marked.

import AnistreamDesign
import AnistreamWidgetData
import SwiftUI
import WidgetKit

/// Airing today at one size, for one moment.
public struct AiringWidgetView: View {
    let content: AiringContent
    let layout: WidgetLayout
    let now: Date
    let format: WidgetFormat

    public init(
        snapshot: WidgetSnapshot?, publicSchedule: [SnapshotAiring]?, filter: AiringFilter,
        layout: WidgetLayout, now: Date, format: WidgetFormat = WidgetFormat()
    ) {
        content = AiringPresentation.content(
            snapshot: snapshot, publicSchedule: publicSchedule, filter: filter, at: now,
            calendar: format.calendar)
        self.layout = layout
        self.now = now
        self.format = format
    }

    public var body: some View {
        switch layout {
        case .inline: inline
        case .rectangular: rectangular
        default: home.widgetURL(WidgetLink.calendar.url)
        }
    }

    // MARK: Home Screen and desktop

    @ViewBuilder private var home: some View {
        let small = layout == .small
        switch content {
        case .rows(let day, let rows, let marks, let tomorrow):
            switch layout {
            case .small:
                AiringSmall(rows: AiringPresentation.comingUp(rows, count: 4), marks: marks, format: format)
            case .large:
                AiringLarge(
                    day: day, rows: AiringPresentation.window(rows, count: 7), marks: marks,
                    tomorrow: tomorrow, now: now, format: format)
            default:
                AiringMedium(day: day, rows: AiringPresentation.window(rows, count: 5), marks: marks, format: format)
            }
        case .nothingToday(let next):
            MessageWidget(
                title: small ? "Today" : "Airing today",
                message: small ? "Nothing airing." : "Nothing airing today.",
                extra: {
                    if let next {
                        Text(small ? format.nextLineShort(next, from: now) : format.nextLine(next, from: now))
                            .font(WidgetType.ui(12))
                            .foregroundStyle(Palette.haze)
                            .padding(.top, 10)
                    }
                })
        case .noSchedule:
            MessageWidget(
                title: small ? "Today" : "Airing today",
                message: "No schedule for today yet. Open anistream to update it.")
        }
    }

    // MARK: Lock Screen

    @ViewBuilder private var rectangular: some View {
        switch content {
        case .rows(_, let rows, _, _):
            VStack(alignment: .leading, spacing: 0) {
                ForEach(AiringPresentation.comingUp(rows, count: 3)) { row in
                    HStack(spacing: 6) {
                        Text(format.time(row.item.airsAt))
                            .font(WidgetType.ui(13))
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 42, alignment: .leading)
                        Text(row.item.title)
                            .font(WidgetType.title(13))
                            .foregroundStyle(row.marked ? .primary : .secondary)
                            .lineLimit(1)
                            .widgetAccentable(row.marked)
                        if row.marked { Bookmark(size: 8, color: .primary) }
                    }
                    .frame(height: 20)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(AiringPresentation.accessibilityLabel(row, format: format))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .widgetURL(WidgetLink.calendar.url)
        case .nothingToday(let next):
            VStack(alignment: .leading, spacing: 2) {
                Text("Nothing airing today").font(WidgetType.ui(15, weight: .semibold))
                if let next {
                    Text(format.nextLineShort(next, from: now)).font(WidgetType.ui(13)).foregroundStyle(.secondary).lineLimit(2)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        case .noSchedule:
            VStack(alignment: .leading, spacing: 2) {
                Text("Airing today").font(WidgetType.ui(15, weight: .semibold))
                Text("Open anistream to update it.").font(WidgetType.ui(13)).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder private var inline: some View {
        switch content {
        case .rows(_, let rows, _, _):
            if let next = AiringPresentation.nextUp(rows) {
                Label("\(format.time(next.item.airsAt)) · \(next.item.title) E\(next.item.episode)", systemImage: "calendar")
                    .widgetURL(WidgetLink.title(next.item.id).url)
            } else {
                Label("Nothing more today", systemImage: "calendar")
            }
        case .nothingToday:
            Label("Nothing airing today", systemImage: "calendar")
        case .noSchedule:
            Label("Airing today", systemImage: "calendar")
        }
    }
}

// MARK: - Sizes

struct AiringHeader: View {
    let title: String
    let detail: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title)
                .font(WidgetType.ui(13, weight: .semibold))
                .foregroundStyle(Palette.silk)
                .widgetAccentable()
            Spacer(minLength: 0)
            if let detail {
                Text(detail).font(WidgetType.ui(12)).foregroundStyle(Palette.haze).lineLimit(1)
            }
        }
        .frame(height: 20)
    }
}

/// One row of the small and medium sizes: time, mark, title and, in medium, the episode or its
/// state.
struct AiringLine: View {
    let row: AiringRow
    let marks: Bool
    let height: CGFloat
    let timeWidth: CGFloat
    let showsState: Bool
    let format: WidgetFormat

    var body: some View {
        let item = row.item
        TapLink(WidgetLink.title(item.id)) {
            HStack(spacing: 6) {
                Text(format.time(item.airsAt))
                    .font(WidgetType.ui(13))
                    .monospacedDigit()
                    .foregroundStyle(row.isUpcoming ? Palette.silk : Palette.haze)
                    .lineLimit(1)
                    .frame(width: timeWidth, alignment: .leading)
                Group {
                    if row.marked { Bookmark(size: 9) } else { Color.clear }
                }
                .frame(width: 10, height: 10)
                Text(item.title)
                    .font(WidgetType.title(13))
                    .foregroundStyle(row.marked || !marks ? Palette.silk : Palette.haze)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .widgetAccentable(row.marked)
                if showsState { state }
            }
            .frame(height: height)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(AiringPresentation.accessibilityLabel(row, format: format))
    }

    @ViewBuilder private var state: some View {
        Group {
            switch row.status {
            case .watched:
                HStack(spacing: 4) {
                    WatchedCheck()
                    Text("E\(row.item.episode)")
                }
            case .out:
                HStack(spacing: 5) {
                    WaterDot()
                    Text("E\(row.item.episode) out")
                }
            case .aired, .upcoming:
                Text(AiringPresentation.episodeShort(row.item))
            }
        }
        .font(WidgetType.ui(12))
        .foregroundStyle(Palette.haze)
        .lineLimit(1)
        .fixedSize()
    }
}

/// Small: the next four times still to come.
struct AiringSmall: View {
    let rows: [AiringRow]
    let marks: Bool
    let format: WidgetFormat

    var body: some View {
        let next = rows.first(where: \.isUpcoming)
        VStack(alignment: .leading, spacing: 2) {
            AiringHeader(title: "Today", detail: next.map { "Next \(format.time($0.item.airsAt))" })
            Color.clear.frame(height: 4)
            ForEach(rows) { row in
                AiringLine(row: row, marks: marks, height: 24, timeWidth: 42, showsState: false, format: format)
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, 13)
        .padding(.trailing, 12)
        .padding(.top, 12)
    }
}

/// Medium: five rows with the episode or its state.
struct AiringMedium: View {
    let day: Date
    let rows: [AiringRow]
    let marks: Bool
    let format: WidgetFormat

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            AiringHeader(title: "Airing today", detail: format.weekday(day))
            Color.clear.frame(height: 3)
            ForEach(rows) { row in
                AiringLine(row: row, marks: marks, height: 22, timeWidth: 46, showsState: true, format: format)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.top, 12)
    }
}

/// Large: the whole day with banner thumbnails, then tomorrow.
struct AiringLarge: View {
    let day: Date
    let rows: [AiringRow]
    let marks: Bool
    let tomorrow: [SnapshotAiring]?
    let now: Date
    let format: WidgetFormat

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            AiringHeader(title: "Airing today", detail: format.longDay(day))
            Color.clear.frame(height: 4)
            ForEach(rows) { row in
                AiringLargeRow(row: row, marks: marks, now: now, format: format)
            }
            if let tomorrow {
                Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.top, 6)
                Text(tomorrowLine(tomorrow))
                    .font(WidgetType.ui(12))
                    .foregroundStyle(Palette.haze)
                    .lineLimit(1)
                    .padding(.top, 7)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.top, 12)
    }

    /// "Tomorrow: nothing airing", "Tomorrow: 16:30 · Hoshikuzu no Tabibito and 2 more".
    private func tomorrowLine(_ rows: [SnapshotAiring]) -> String {
        let first = rows.first { !marks || $0.onList } ?? rows.first
        guard let first else { return "Tomorrow: nothing airing" }
        let more = rows.count - 1
        return "Tomorrow: \(format.time(first.airsAt)) · \(first.title)" + (more > 0 ? " and \(more) more" : "")
    }
}

struct AiringLargeRow: View {
    let row: AiringRow
    let marks: Bool
    let now: Date
    let format: WidgetFormat
    @Environment(\.widgetFiles) private var files

    var body: some View {
        let item = row.item
        TapLink(WidgetLink.title(item.id)) {
            HStack(spacing: 8) {
                Text(format.time(item.airsAt))
                    .font(WidgetType.ui(13))
                    .monospacedDigit()
                    .foregroundStyle(row.isUpcoming ? Palette.silk : Palette.haze)
                    .lineLimit(1)
                    .frame(width: 46, alignment: .leading)
                thumbnail
                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 5) {
                        Text(item.title)
                            .font(WidgetType.title(13))
                            .foregroundStyle(row.marked || !marks ? Palette.silk : Palette.haze)
                            .lineLimit(1)
                            .widgetAccentable(row.marked)
                        if row.marked { Bookmark(size: 9) }
                    }
                    HStack(spacing: 6) {
                        Text(AiringPresentation.episodeLong(item))
                        Text("·").accessibilityHidden(true)
                        state
                    }
                    .font(WidgetType.ui(11))
                    .foregroundStyle(Palette.haze)
                    .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .frame(height: 38)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(AiringPresentation.accessibilityLabel(row, format: format))
    }

    /// The banner as a 16:9 thumbnail, or the poster in its place.
    @ViewBuilder private var thumbnail: some View {
        if ArtImage.exists(row.item.art.wide, in: files) {
            ArtFrame(name: row.item.art.wide, width: 48, height: 27, radius: 5)
        } else if ArtImage.exists(row.item.art.poster, in: files) {
            ArtFrame(name: row.item.art.poster, width: 18, height: 27, radius: 3)
                .frame(width: 48)
        } else {
            Color.clear.frame(width: 48, height: 27)
        }
    }

    @ViewBuilder private var state: some View {
        switch row.status {
        case .watched:
            HStack(spacing: 4) {
                WatchedCheck()
                Text("Watched")
            }
        case .out:
            HStack(spacing: 5) {
                WaterDot()
                HStack(spacing: 0) { Text("Out "); RelativeTime(date: row.item.airsAt); Text(" ago") }
            }
        case .aired:
            HStack(spacing: 0) { Text("Out "); RelativeTime(date: row.item.airsAt); Text(" ago") }
        case .upcoming:
            HStack(spacing: 0) { Text("In "); RelativeTime(date: row.item.airsAt) }
        }
    }
}
