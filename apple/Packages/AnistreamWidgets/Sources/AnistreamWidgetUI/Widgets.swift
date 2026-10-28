// The two widgets, their timelines and Airing today's edit sheet.

import AnistreamDesign
import AnistreamStrings
import AnistreamWidgetData
import AppIntents
import SwiftUI
import WidgetKit

// MARK: - Continue watching

public struct ContinueEntry: TimelineEntry {
    public let date: Date
    public let snapshot: WidgetSnapshot?
    public let files: WidgetFiles?

    public init(date: Date, snapshot: WidgetSnapshot?, files: WidgetFiles?) {
        self.date = date
        self.snapshot = snapshot
        self.files = files
    }
}

public struct ContinueProvider: TimelineProvider {
    public init() {}

    public func placeholder(in context: Context) -> ContinueEntry {
        ContinueEntry(date: .now, snapshot: nil, files: nil)
    }

    public func getSnapshot(in context: Context, completion: @escaping (ContinueEntry) -> Void) {
        let files = WidgetFiles.shared()
        completion(ContinueEntry(date: .now, snapshot: files?.snapshot(), files: files))
    }

    public func getTimeline(in context: Context, completion: @escaping (Timeline<ContinueEntry>) -> Void) {
        let files = WidgetFiles.shared()
        let snapshot = files?.snapshot()
        let plan = WidgetTimeline.continuePlan(snapshot, now: .now)
        let entries = plan.dates.map { ContinueEntry(date: $0, snapshot: snapshot, files: files) }
        completion(Timeline(entries: entries, policy: plan.reloadAt.map { .after($0) } ?? .never))
    }
}

public struct ContinueWatchingWidget: Widget {
    public static let kind = WidgetKinds.continueWatching

    public init() {}

    public var body: some WidgetConfiguration {
        StaticConfiguration(kind: Self.kind, provider: ContinueProvider()) { entry in
            ContinueEntryView(entry: entry)
        }
        .configurationDisplayName(L.widgetContinueTitle)
        .description(L.widgetContinueDescription)
        .supportedFamilies(WidgetFamilies.supported)
        .contentMarginsDisabled()
    }
}

struct ContinueEntryView: View {
    let entry: ContinueEntry
    @Environment(\.widgetFamily) private var family

    var body: some View {
        ContinueWidgetView(snapshot: entry.snapshot, layout: WidgetLayout(family), now: entry.date)
            .environment(\.widgetFiles, entry.files)
            .widgetChrome(family)
    }
}

// MARK: - Airing today

/// The edit sheet's one choice.
public enum AiringShow: String, AppEnum {
    case all
    case myList

    public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Show"  // i18n-skip
    public static let caseDisplayRepresentations: [AiringShow: DisplayRepresentation] = [
        .all: "All airing", // i18n-skip
        .myList: "My list", // i18n-skip
    ]

    var filter: AiringFilter {
        switch self {
        case .all: .all
        case .myList: .myList
        }
    }
}

public struct AiringTodayConfiguration: WidgetConfigurationIntent {
    public static let title: LocalizedStringResource = "Airing today" // i18n-skip
    public static let description = IntentDescription("Today’s episodes, with the shows in your list marked.") // i18n-skip

    @Parameter(title: "Show", default: .all) // i18n-skip
    public var show: AiringShow

    public init() {}

    public init(show: AiringShow) {
        self.show = show
    }
}

public struct AiringEntry: TimelineEntry {
    public let date: Date
    public let snapshot: WidgetSnapshot?
    /// AniList's public schedule, fetched when there is no snapshot to read.
    public let publicSchedule: [SnapshotAiring]?
    public let filter: AiringFilter
    public let files: WidgetFiles?
}

public struct AiringProvider: AppIntentTimelineProvider {
    public init() {}

    public func placeholder(in context: Context) -> AiringEntry {
        AiringEntry(date: .now, snapshot: nil, publicSchedule: [], filter: .all, files: nil)
    }

    public func snapshot(for configuration: AiringTodayConfiguration, in context: Context) async -> AiringEntry {
        await entries(for: configuration, preview: context.isPreview).first
            ?? placeholder(in: context)
    }

    public func timeline(for configuration: AiringTodayConfiguration, in context: Context) async -> Timeline<AiringEntry> {
        let now = Date.now
        let files = WidgetFiles.shared()
        let snapshot = files?.snapshot()
        let publicSchedule = snapshot == nil ? await Self.publicSchedule(files: files, now: now) : nil
        let rows = snapshot?.schedule ?? publicSchedule ?? []
        let plan = WidgetTimeline.airingPlan(rows, now: now, publicSchedule: snapshot == nil)
        let entries = plan.dates.map {
            AiringEntry(
                date: $0, snapshot: snapshot, publicSchedule: publicSchedule,
                filter: configuration.show.filter, files: files)
        }
        return Timeline(entries: entries, policy: plan.reloadAt.map { .after($0) } ?? .atEnd)
    }

    private func entries(for configuration: AiringTodayConfiguration, preview: Bool) async -> [AiringEntry] {
        let files = WidgetFiles.shared()
        let snapshot = files?.snapshot()
        // The gallery's preview asks nothing of AniList.
        let publicSchedule = snapshot == nil && !preview ? await Self.publicSchedule(files: files, now: .now) : nil
        return [AiringEntry(
            date: .now, snapshot: snapshot, publicSchedule: publicSchedule ?? (snapshot == nil ? [] : nil),
            filter: configuration.show.filter, files: files)]
    }

    static func publicSchedule(files: WidgetFiles?, now: Date) async -> [SnapshotAiring]? {
        if let files { return await files.publicSchedule(at: now) }
        return try? await PublicSchedule.fetch(day: now)
    }
}

public struct AiringTodayWidget: Widget {
    public static let kind = WidgetKinds.airingToday

    public init() {}

    public var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: Self.kind, intent: AiringTodayConfiguration.self, provider: AiringProvider()) { entry in
            AiringEntryView(entry: entry)
        }
        .configurationDisplayName(L.widgetAiringToday)
        .description(L.widgetAiringDescription)
        .supportedFamilies(WidgetFamilies.supported)
        .contentMarginsDisabled()
    }
}

struct AiringEntryView: View {
    let entry: AiringEntry
    @Environment(\.widgetFamily) private var family

    var body: some View {
        AiringWidgetView(
            snapshot: entry.snapshot, publicSchedule: entry.publicSchedule, filter: entry.filter,
            layout: WidgetLayout(family), now: entry.date)
            .environment(\.widgetFiles, entry.files)
            .widgetChrome(family)
    }
}

// MARK: - Both

enum WidgetFamilies {
    /// iPhone and iPad: the three Home Screen sizes and two Lock Screen accessories. The Mac:
    /// small and medium, since the desktop is for a glance.
    static var supported: [WidgetFamily] {
        #if os(iOS)
        [.systemSmall, .systemMedium, .systemLarge, .accessoryRectangular, .accessoryInline]
        #else
        [.systemSmall, .systemMedium]
        #endif
    }
}

extension View {
    /// The ground behind a Home Screen or desktop widget, which the system takes away when it
    /// tints or clears it, and nothing behind an accessory. Dark always: the palette is.
    @ViewBuilder func widgetChrome(_ family: WidgetFamily) -> some View {
        if WidgetLayout(family).isAccessory {
            containerBackground(for: .widget) { Color.clear }
        } else {
            self
                .environment(\.colorScheme, .dark)
                .dynamicTypeSize(...DynamicTypeSize.xLarge)
                .containerBackground(for: .widget) { Palette.ground }
        }
    }
}
