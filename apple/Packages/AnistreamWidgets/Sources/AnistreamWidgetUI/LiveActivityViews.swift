// The downloads' Live Activity on iPhone: the Lock Screen card and the Dynamic Island, as
// NativeLiveActivity draws them. Download progress is water; amber is only Play once a download
// has finished.

import AnistreamDesign
import AnistreamWidgetData
import SwiftUI
import WidgetKit
#if os(iOS)
import ActivityKit
#endif

/// The Lock Screen's card: the cover, the title, two lines, the button, and the progress bar.
public struct DownloadsLockScreenView: View {
    let state: DownloadsActivityState
    let stale: Bool

    public init(state: DownloadsActivityState, stale: Bool = false) {
        self.state = state
        self.stale = stale
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        let text = DownloadsActivityText(state, stale: stale, locale: locale)
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                DownloadCover(file: state.coverFile, width: 36, height: 54, radius: 6)
                VStack(alignment: .leading, spacing: 2) {
                    Text(text.title)
                        .font(text.titleIsShow ? WidgetType.title(15) : WidgetType.ui(15, weight: .semibold))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(1)
                    Text(text.line1)
                        .font(WidgetType.ui(13))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(1)
                    Text(text.line2)
                        .font(WidgetType.ui(13))
                        .monospacedDigit()
                        .foregroundStyle(Palette.haze)
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                DownloadsButton(state: state, stale: stale)
            }
            if !state.finished {
                WaterBar(fraction: state.fraction, track: Palette.silk.opacity(0.18))
                    .padding(.top, 10)
            }
        }
        .padding(EdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 14))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(text.accessibilityLabel)
    }
}

/// Pause, Resume, or once finished the amber Play: the activity's one button.
struct DownloadsButton: View {
    let state: DownloadsActivityState
    var stale = false
    @Environment(\.widgetDrawsTaps) private var drawsTaps

    var body: some View {
        if state.finished {
            let play = HStack(spacing: 6) {
                Image(systemName: "play.fill").font(.system(size: 10))
                Text("Play E\(state.episode)").font(WidgetType.ui(15, weight: .semibold))
            }
            .foregroundStyle(Palette.ground)
            .padding(.horizontal, 14)
            .frame(height: 36)
            .background(Capsule().fill(Palette.amber))
            if drawsTaps, let id = state.downloadID {
                Link(destination: WidgetLink.download(id).url) { play }
                    .accessibilityLabel("Play episode \(state.episode)")
            } else {
                play
            }
        } else if !stale {
            let glyph = Image(systemName: state.paused ? "play.fill" : "pause.fill")
                .font(.system(size: state.paused ? 12 : 14))
                .foregroundStyle(Palette.silk)
                .frame(width: 36, height: 36)
                .background(Circle().fill(Palette.silk.opacity(0.14)))
                .frame(width: 44, height: 44)
            let label = state.paused ? "Resume downloads" : "Pause downloads"
            #if os(iOS)
            if drawsTaps {
                if state.paused {
                    Button(intent: ResumeDownloadsIntent()) { glyph }.buttonStyle(.plain).accessibilityLabel(label)
                } else {
                    Button(intent: PauseDownloadsIntent()) { glyph }.buttonStyle(.plain).accessibilityLabel(label)
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

/// Download progress: water on a faint track, 4 points. Playback's is amber, as the player's.
struct WaterBar: View {
    let fraction: Double
    let track: Color
    var fill: Color = Palette.water

    var body: some View {
        Capsule()
            .fill(track)
            .overlay(alignment: .leading) {
                GeometryReader { proxy in
                    Capsule().fill(fill).frame(width: proxy.size.width * min(max(fraction, 0), 1))
                }
            }
            .frame(height: 4)
            .accessibilityHidden(true)
    }
}

/// The Dynamic Island's ring: water on a faint circle.
public struct DownloadRing: View {
    let fraction: Double
    var size: CGFloat = 20
    var stroke: CGFloat = 2.5

    public init(fraction: Double, size: CGFloat = 20, stroke: CGFloat = 2.5) {
        self.fraction = fraction
        self.size = size
        self.stroke = stroke
    }

    public var body: some View {
        ZStack {
            Circle().stroke(Palette.silk.opacity(0.2), lineWidth: stroke)
            Circle()
                .trim(from: 0, to: min(max(fraction, 0), 1))
                .stroke(Palette.water, style: StrokeStyle(lineWidth: stroke, lineCap: .round))
                .rotationEffect(.degrees(-90))
        }
        .padding(stroke / 2)
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// The cover, from the App Group's `widgets/live/`.
struct DownloadCover: View {
    let file: String?
    let width: CGFloat
    let height: CGFloat
    let radius: CGFloat
    @Environment(\.widgetFiles) private var files

    var body: some View {
        if let file, let files, let image = ArtThumbnail.image(at: files.liveDirectory.appending(path: file), maxPixelSize: 300) {
            Image(decorative: image, scale: 1)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: width, height: height, alignment: .top)
                .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
                .accessibilityHidden(true)
        }
    }
}

// MARK: - The Dynamic Island's regions

/// Expanded, at the top: the title and the episode.
public struct DownloadsIslandTitle: View {
    let state: DownloadsActivityState

    public init(state: DownloadsActivityState) {
        self.state = state
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        let text = DownloadsActivityText(state, locale: locale)
        VStack(alignment: .leading, spacing: 2) {
            Text(text.title)
                .font(text.titleIsShow ? WidgetType.title(15) : WidgetType.ui(15, weight: .semibold))
                .foregroundStyle(.white)
                .lineLimit(1)
            Text(state.finished ? text.line1 : (state.count > 1 ? "\(state.title) E\(state.episode)" : "Episode \(state.episode)"))
                .font(WidgetType.ui(13))
                .foregroundStyle(.white.opacity(0.7))
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Expanded, at the top right: the time left, in water.
public struct DownloadsIslandTimeLeft: View {
    let state: DownloadsActivityState

    public init(state: DownloadsActivityState) {
        self.state = state
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        if let left = DownloadsActivityText(state, locale: locale).timeLeft {
            Text(left)
                .font(WidgetType.ui(15, weight: .semibold))
                .monospacedDigit()
                .foregroundStyle(Palette.water)
        } else if state.paused {
            Text("Paused").font(WidgetType.ui(15, weight: .semibold)).foregroundStyle(.white.opacity(0.7))
        }
    }
}

/// Expanded, at the bottom: the bar, the bytes and the speed, and the button.
public struct DownloadsIslandBottom: View {
    let state: DownloadsActivityState

    public init(state: DownloadsActivityState) {
        self.state = state
    }

    @Environment(\.locale) private var locale

    public var body: some View {
        let text = DownloadsActivityText(state, locale: locale)
        HStack(alignment: .center, spacing: 12) {
            if state.finished {
                Text(text.line2)
                    .font(WidgetType.ui(13))
                    .foregroundStyle(.white.opacity(0.7))
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    WaterBar(fraction: state.fraction, track: .white.opacity(0.2))
                    Text(state.count > 1 ? text.line2 : text.detail)
                        .font(WidgetType.ui(13))
                        .monospacedDigit()
                        .foregroundStyle(.white.opacity(0.7))
                        .lineLimit(1)
                }
            }
            DownloadsButton(state: state)
        }
    }
}

/// The compact island's leading side: the cover as a rounded square.
public struct DownloadsIslandCover: View {
    let state: DownloadsActivityState

    public init(state: DownloadsActivityState) {
        self.state = state
    }

    public var body: some View {
        DownloadCover(file: state.coverFile, width: 24, height: 24, radius: 7)
    }
}

#if os(iOS)
/// The Live Activity itself.
public struct DownloadsLiveActivity: Widget {
    public init() {}

    public var body: some WidgetConfiguration {
        ActivityConfiguration(for: DownloadsAttributes.self) { context in
            DownloadsLockScreenView(state: context.state, stale: context.isStale)
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
                    DownloadsIslandTimeLeft(state: context.state).padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.center) {
                    DownloadsIslandTitle(state: context.state)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    DownloadsIslandBottom(state: context.state).padding(.horizontal, 4)
                }
            } compactLeading: {
                DownloadsIslandCover(state: context.state).environment(\.widgetFiles, files)
            } compactTrailing: {
                DownloadRing(fraction: context.state.fraction)
            } minimal: {
                DownloadRing(fraction: context.state.fraction)
            }
            .keylineTint(Palette.water)
        }
    }
}
#endif
