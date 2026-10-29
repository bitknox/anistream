// The playback and airing Live Activities on iPhone: the Lock Screen cards and the Dynamic
// Island, in the downloads activity's layout. Playback's bar is amber, as the player's playhead
// is; the airing countdown is silk on the Lock Screen and water in the island, as the downloads'
// time left is. Both run on dates, so the progress, the time left and the countdown move with no
// update from the app.

import AnistreamDesign
import AnistreamStrings
import AnistreamWidgetData
import SwiftUI
import WidgetKit
#if os(iOS)
import ActivityKit
#endif

// MARK: - Timers

private struct RunsTimersKey: EnvironmentKey {
    static let defaultValue = true
}

extension EnvironmentValues {
    /// Whether the progress, the time left and the countdown run on the system's timers: off for
    /// the shots, which ImageRenderer draws at one moment and which have no system progress.
    public var widgetRunsTimers: Bool {
        get { self[RunsTimersKey.self] }
        set { self[RunsTimersKey.self] = newValue }
    }
}

// MARK: - Playback

/// The Lock Screen's card: the cover, the title, the episode and the time left, Play or Pause,
/// and the bar.
public struct PlaybackLockScreenView: View {
    let state: PlaybackActivityState
    let stale: Bool

    public init(state: PlaybackActivityState, stale: Bool = false) {
        self.state = state
        self.stale = stale
    }

    public var body: some View {
        let text = PlaybackActivityText(state)
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                DownloadCover(file: state.coverFile, width: 36, height: 54, radius: 6)
                VStack(alignment: .leading, spacing: 2) {
                    Text(text.title)
                        .font(WidgetType.title(15))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(1)
                    Text(text.episode)
                        .font(WidgetType.ui(13))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(1)
                    if !stale {
                        PlaybackTimeLeft(state: state)
                            .font(WidgetType.ui(13))
                            .monospacedDigit()
                            .foregroundStyle(Palette.haze)
                            .lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                PlaybackButton(state: state, stale: stale)
            }
            PlaybackBar(state: state, stale: stale, track: Palette.silk.opacity(0.18))
                .padding(.top, 10)
        }
        .padding(EdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 14))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(text.accessibilityLabel)
    }
}

/// "18:20 left", running while it plays; "Paused · 18:20 left" while paused.
struct PlaybackTimeLeft: View {
    let state: PlaybackActivityState
    @Environment(\.widgetRunsTimers) private var runsTimers

    var body: some View {
        if state.paused {
            Text(PlaybackActivityText(state).pausedLine)
        } else if runsTimers {
            Text("\(Text(timerInterval: state.timeline, countsDown: true)) left")
        } else {
            Text("\(PlaybackActivityText.clock(state.remaining / max(state.rate, 0.01))) left")
        }
    }
}

/// Play or Pause, in silk: the activity's one button. None once stale, when the app is not there
/// to take it.
struct PlaybackButton: View {
    let state: PlaybackActivityState
    var stale = false
    @Environment(\.widgetDrawsTaps) private var drawsTaps

    var body: some View {
        if !stale {
            let glyph = Image(systemName: state.paused ? "play.fill" : "pause.fill")
                .font(.system(size: state.paused ? 12 : 14))
                .foregroundStyle(Palette.silk)
                .frame(width: 36, height: 36)
                .background(Circle().fill(Palette.silk.opacity(0.14)))
                .frame(width: 44, height: 44)
            let label = state.paused ? L.actionPlay : L.actionPause
            #if os(iOS)
            if drawsTaps {
                if state.paused {
                    Button(intent: ResumePlaybackIntent()) { glyph }.buttonStyle(.plain).accessibilityLabel(label)
                } else {
                    Button(intent: PausePlaybackIntent()) { glyph }.buttonStyle(.plain).accessibilityLabel(label)
                }
            } else {
                glyph.accessibilityLabel(label)
            }
            #else
            glyph.accessibilityLabel(label)
            #endif
        }
    }
}

/// How far in: amber, running on the episode's dates while it plays.
struct PlaybackBar: View {
    let state: PlaybackActivityState
    let stale: Bool
    let track: Color
    @Environment(\.widgetRunsTimers) private var runsTimers

    var body: some View {
        Group {
            if !runsTimers {
                WaterBar(fraction: state.fraction, track: track, fill: Palette.amber)
            } else if state.paused || stale {
                ProgressView(value: state.fraction)
                    .progressViewStyle(.linear)
            } else {
                ProgressView(timerInterval: state.timeline, countsDown: false) {
                    EmptyView()
                } currentValueLabel: {
                    EmptyView()
                }
                .progressViewStyle(.linear)
            }
        }
        .tint(Palette.amber)
        .accessibilityHidden(true)
    }
}

/// Expanded, at the top: the title and the episode.
public struct PlaybackIslandTitle: View {
    let state: PlaybackActivityState

    public init(state: PlaybackActivityState) {
        self.state = state
    }

    public var body: some View {
        let text = PlaybackActivityText(state)
        VStack(alignment: .leading, spacing: 2) {
            Text(text.title)
                .font(WidgetType.title(15))
                .foregroundStyle(.white)
                .lineLimit(1)
            Text(text.episode)
                .font(WidgetType.ui(13))
                .foregroundStyle(.white.opacity(0.7))
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The time left, as the island's trailing side shows it: running while it plays, "Paused" while
/// paused.
public struct PlaybackIslandTimeLeft: View {
    let state: PlaybackActivityState
    var compact = false
    @Environment(\.widgetRunsTimers) private var runsTimers

    public init(state: PlaybackActivityState, compact: Bool = false) {
        self.state = state
        self.compact = compact
    }

    public var body: some View {
        Group {
            if state.paused {
                if compact {
                    Image(systemName: "pause.fill").font(.system(size: 11)).accessibilityLabel(L.libraryStatusPaused)
                } else {
                    Text(L.libraryStatusPaused)
                }
            } else if runsTimers {
                Text(timerInterval: state.timeline, countsDown: true)
            } else {
                Text(PlaybackActivityText.clock(state.remaining / max(state.rate, 0.01)))
            }
        }
        .font(WidgetType.ui(compact ? 13 : 15, weight: .semibold))
        .monospacedDigit()
        .multilineTextAlignment(.trailing)
        .foregroundStyle(state.paused ? .white.opacity(0.7) : .white)
        .frame(maxWidth: compact ? 52 : 80, alignment: .trailing)
    }
}

/// Expanded, at the bottom: the bar, and Play or Pause.
public struct PlaybackIslandBottom: View {
    let state: PlaybackActivityState

    public init(state: PlaybackActivityState) {
        self.state = state
    }

    public var body: some View {
        HStack(alignment: .center, spacing: 12) {
            PlaybackBar(state: state, stale: false, track: .white.opacity(0.2))
            PlaybackButton(state: state)
        }
    }
}

// MARK: - Airing

/// The Lock Screen's card: the cover, the title, the episode and its time, and the countdown,
/// which turns to "Out now".
public struct AiringLockScreenView: View {
    let state: AiringActivityState
    let out: Bool

    /// `out`: it has aired, as the app saw or the stale date says.
    public init(state: AiringActivityState, out: Bool) {
        self.state = state
        self.out = out
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        let text = AiringActivityText(state, out: out, locale: locale)
        HStack(spacing: 12) {
            DownloadCover(file: state.coverFile, width: 36, height: 54, radius: 6)
            VStack(alignment: .leading, spacing: 2) {
                Text(text.title)
                    .font(WidgetType.title(15))
                    .foregroundStyle(Palette.silk)
                    .lineLimit(1)
                Text(text.episode)
                    .font(WidgetType.ui(13))
                    .foregroundStyle(Palette.silk)
                    .lineLimit(1)
                Text(text.time)
                    .font(WidgetType.ui(13))
                    .monospacedDigit()
                    .foregroundStyle(Palette.haze)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            AiringCountdown(state: state, out: out)
                .font(out ? WidgetType.ui(15, weight: .semibold) : WidgetType.ui(22, weight: .semibold))
                .foregroundStyle(Palette.silk)
                .padding(.trailing, 4)
        }
        .padding(EdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 14))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(text.accessibilityLabel)
    }
}

/// "12:04" counting down to the air time, or "Out now".
public struct AiringCountdown: View {
    let state: AiringActivityState
    let out: Bool
    var maxWidth: CGFloat = 96
    @Environment(\.widgetRunsTimers) private var runsTimers

    public init(state: AiringActivityState, out: Bool, maxWidth: CGFloat = 96) {
        self.state = state
        self.out = out
        self.maxWidth = maxWidth
    }

    public var body: some View {
        Group {
            if out {
                Text(AiringActivityText.outNow)
            } else if runsTimers {
                Text(timerInterval: state.countdown, countsDown: true)
            } else {
                Text(PlaybackActivityText.clock(state.airsAt.timeIntervalSinceNow))
            }
        }
        .monospacedDigit()
        .lineLimit(1)
        .multilineTextAlignment(.trailing)
        .frame(maxWidth: maxWidth, alignment: .trailing)
    }
}

/// The countdown as the island shows it, in water: compact at the trailing side, or expanded.
public struct AiringIslandCountdown: View {
    let state: AiringActivityState
    let out: Bool
    var compact = false

    public init(state: AiringActivityState, out: Bool, compact: Bool = false) {
        self.state = state
        self.out = out
        self.compact = compact
    }

    public var body: some View {
        AiringCountdown(state: state, out: out, maxWidth: compact ? 56 : 80)
            .font(WidgetType.ui(compact ? 13 : 15, weight: .semibold))
            .foregroundStyle(Palette.water)
    }
}

/// Expanded, at the top: the title and the episode.
public struct AiringIslandTitle: View {
    let state: AiringActivityState
    let out: Bool

    public init(state: AiringActivityState, out: Bool) {
        self.state = state
        self.out = out
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        let text = AiringActivityText(state, out: out, locale: locale)
        VStack(alignment: .leading, spacing: 2) {
            Text(text.title)
                .font(WidgetType.title(15))
                .foregroundStyle(.white)
                .lineLimit(1)
            Text("\(text.episode) · \(text.time)")
                .font(WidgetType.ui(13))
                .foregroundStyle(.white.opacity(0.7))
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The cover as a rounded square, for the compact and minimal island.
public struct LiveActivityCover: View {
    let file: String?

    public init(file: String?) {
        self.file = file
    }

    public var body: some View {
        DownloadCover(file: file, width: 24, height: 24, radius: 7)
    }
}

#if os(iOS)
/// The playback's Live Activity.
public struct PlaybackLiveActivity: Widget {
    public init() {}

    public var body: some WidgetConfiguration {
        ActivityConfiguration(for: PlaybackAttributes.self) { context in
            PlaybackLockScreenView(state: context.state, stale: context.isStale)
                .environment(\.widgetFiles, WidgetFiles.shared())
                .environment(\.colorScheme, .dark)
                .activityBackgroundTint(Palette.ground.opacity(0.92))
                .activitySystemActionForegroundColor(Palette.silk)
        } dynamicIsland: { context in
            let files = WidgetFiles.shared()
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    DownloadCover(file: context.state.coverFile, width: 36, height: 54, radius: 6)
                        .environment(\.widgetFiles, files)
                        .padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    PlaybackIslandTimeLeft(state: context.state).padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.center) {
                    PlaybackIslandTitle(state: context.state)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    PlaybackIslandBottom(state: context.state).padding(.horizontal, 4)
                }
            } compactLeading: {
                LiveActivityCover(file: context.state.coverFile).environment(\.widgetFiles, files)
            } compactTrailing: {
                PlaybackIslandTimeLeft(state: context.state, compact: true)
            } minimal: {
                LiveActivityCover(file: context.state.coverFile).environment(\.widgetFiles, files)
            }
            .keylineTint(Palette.amber)
        }
    }
}

/// The airing Live Activity. Its stale date is the air time, so `isStale` turns it to "Out now"
/// whether the app is running or not.
public struct AiringLiveActivity: Widget {
    public init() {}

    public var body: some WidgetConfiguration {
        ActivityConfiguration(for: AiringAttributes.self) { context in
            let out = context.state.out || context.isStale
            AiringLockScreenView(state: context.state, out: out)
                .environment(\.widgetFiles, WidgetFiles.shared())
                .environment(\.colorScheme, .dark)
                .activityBackgroundTint(Palette.ground.opacity(0.92))
                .activitySystemActionForegroundColor(Palette.silk)
                .widgetURL(WidgetLink.title(context.state.titleID).url)
        } dynamicIsland: { context in
            let files = WidgetFiles.shared()
            let out = context.state.out || context.isStale
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    DownloadCover(file: context.state.coverFile, width: 36, height: 54, radius: 6)
                        .environment(\.widgetFiles, files)
                        .padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    AiringIslandCountdown(state: context.state, out: out).padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.center) {
                    AiringIslandTitle(state: context.state, out: out)
                }
            } compactLeading: {
                LiveActivityCover(file: context.state.coverFile).environment(\.widgetFiles, files)
            } compactTrailing: {
                AiringIslandCountdown(state: context.state, out: out, compact: true)
            } minimal: {
                LiveActivityCover(file: context.state.coverFile).environment(\.widgetFiles, files)
            }
            .widgetURL(WidgetLink.title(context.state.titleID).url)
            .keylineTint(Palette.water)
        }
    }
}
#endif
