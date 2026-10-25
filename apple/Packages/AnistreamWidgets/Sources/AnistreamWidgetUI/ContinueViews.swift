// Continue watching: the next episode, its progress, a tap to open the title and one to play.

import AnistreamDesign
import AnistreamWidgetData
import SwiftUI
import WidgetKit

/// Continue watching at one size, for one moment.
public struct ContinueWidgetView: View {
    let content: ContinueContent
    let layout: WidgetLayout
    let now: Date
    let format: WidgetFormat

    public init(snapshot: WidgetSnapshot?, layout: WidgetLayout, now: Date, format: WidgetFormat = WidgetFormat()) {
        content = ContinuePresentation.content(snapshot, at: now)
        self.layout = layout
        self.now = now
        self.format = format
    }

    public var body: some View {
        switch layout {
        case .inline: inline
        case .rectangular: rectangular
        default: home
        }
    }

    // MARK: Home Screen and desktop

    @ViewBuilder private var home: some View {
        switch content {
        case .signedOut:
            MessageWidget(
                title: layout == .small ? "Continue" : "Continue watching",
                message: layout == .small
                    ? "Connect AniList, or play an episode, to see it here."
                    : "Connect AniList, or play an episode, to see what you’re watching here.",
                action: "Connect AniList"
            )
            .widgetURL(WidgetLink.accounts.url)
        case .caughtUp(let next, let art):
            CaughtUp(next: next, art: art, small: layout == .small, now: now, format: format)
                .widgetURL(next.map { WidgetLink.title($0.id).url })
        case .shows(let items, let updated):
            let stale = updated.map { format.updated($0, from: now) }
            switch layout {
            case .small:
                ContinueSmall(item: items[0], stale: stale)
                    .widgetURL(WidgetLink.title(items[0].id).url)
            case .large:
                ContinueLarge(items: Array(items.prefix(3)), stale: stale)
                    .widgetURL(WidgetLink.title(items[0].id).url)
            default:
                ContinueMedium(items: Array(items.prefix(2)), stale: stale)
                    .widgetURL(WidgetLink.title(items[0].id).url)
            }
        }
    }

    // MARK: Lock Screen

    @ViewBuilder private var rectangular: some View {
        switch content {
        case .signedOut:
            VStack(alignment: .leading, spacing: 2) {
                Text("Continue watching").font(WidgetType.ui(15, weight: .semibold))
                Text("Connect AniList to see it here.").font(WidgetType.ui(13)).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .widgetURL(WidgetLink.accounts.url)
        case .caughtUp(let next, _):
            VStack(alignment: .leading, spacing: 2) {
                Text("You’re caught up").font(WidgetType.ui(15, weight: .semibold))
                if let next {
                    Text(next.title).font(WidgetType.title(13)).lineLimit(1).widgetAccentable()
                    Text(format.airs(next, from: now)).font(WidgetType.ui(13)).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .widgetURL(next.map { WidgetLink.title($0.id).url })
        case .shows(let items, let updated):
            let item = items[0]
            VStack(alignment: .leading, spacing: 3) {
                Text(item.title)
                    .font(WidgetType.title(15))
                    .lineLimit(1)
                    .widgetAccentable()
                Text(ContinuePresentation.line(item))
                    .font(WidgetType.ui(13))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                if let updated {
                    Text(format.updated(updated, from: now)).font(WidgetType.ui(12)).foregroundStyle(.secondary)
                } else if let progress = item.progress {
                    ProgressBar(fraction: progress, height: 4, monochrome: true).padding(.top, 3)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityElement(children: .combine)
            .accessibilityLabel(ContinuePresentation.accessibilityLabel(item))
            .widgetURL(WidgetLink.title(item.id).url)
        }
    }

    @ViewBuilder private var inline: some View {
        switch content {
        case .signedOut:
            Label("Connect AniList", systemImage: "play.fill")
                .widgetURL(WidgetLink.accounts.url)
        case .caughtUp(let next, _):
            Label(next.map { "Caught up · E\($0.episode) \(format.day($0.airsAt, from: now))" } ?? "Caught up",
                  systemImage: "checkmark")
        case .shows(let items, _):
            Label("\(ContinuePresentation.episodeShort(items[0])) · \(items[0].title)", systemImage: "play.fill")
                .widgetURL(WidgetLink.title(items[0].id).url)
        }
    }
}

// MARK: - Sizes

/// Small: the first show, wide art across the top with the play button over it.
struct ContinueSmall: View {
    let item: SnapshotContinue
    let stale: String?
    @Environment(\.widgetFiles) private var files

    var body: some View {
        GeometryReader { proxy in
            let artHeight = (proxy.size.height * 0.53).rounded()
            VStack(alignment: .leading, spacing: 0) {
                if ArtImage.exists(item.art.wide, in: files) {
                    ArtFrame(name: item.art.wide, width: proxy.size.width, height: artHeight)
                        .overlay(alignment: .bottomTrailing) { play.padding(12) }
                    text.padding(.horizontal, 12).padding(.top, 8)
                } else {
                    // No banner: the poster beside, never stretched into the banner's place.
                    HStack(alignment: .bottom) {
                        if ArtImage.exists(item.art.poster, in: files) {
                            ArtFrame(name: item.art.poster, width: 48, height: 72, radius: 6)
                        }
                        Spacer(minLength: 0)
                        play
                    }
                    .padding([.horizontal, .top], 12)
                    text.padding(.horizontal, 12).padding(.top, 10)
                }
                Spacer(minLength: 0)
            }
        }
    }

    private var play: some View {
        PlayCircle(id: item.id, episode: item.episode, label: "Play episode \(item.episode) of \(item.title)", intent: true)
    }

    private var text: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(item.title)
                .font(WidgetType.title(14))
                .foregroundStyle(Palette.silk)
                .lineLimit(1)
                .lineBox(18)
                .widgetAccentable()
            Text(ContinuePresentation.line(item))
                .font(WidgetType.ui(12))
                .foregroundStyle(Palette.haze)
                .lineLimit(1)
                .lineBox(16)
            if let stale {
                Text(stale).font(WidgetType.ui(11)).foregroundStyle(Palette.haze).padding(.top, 4)
            } else if let progress = item.progress {
                ProgressBar(fraction: progress).padding(.top, 7)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(ContinuePresentation.accessibilityLabel(item))
    }
}

/// Medium: the first two shows as 16:9 cards, or the one across the widget.
struct ContinueMedium: View {
    let items: [SnapshotContinue]
    let stale: String?

    var body: some View {
        GeometryReader { proxy in
            let inner = proxy.size.width - 28
            let cardWidth = items.count > 1 ? ((inner - 12) / 2).rounded(.down) : inner
            let artHeight = proxy.size.height - 28 - 50
            HStack(alignment: .top, spacing: 12) {
                ForEach(items) { item in
                    ContinueCard(item: item, width: cardWidth, artHeight: artHeight, showsProgress: stale == nil)
                }
            }
            .padding(14)
            .frame(width: proxy.size.width, height: proxy.size.height, alignment: .topLeading)
            .overlay(alignment: .bottomTrailing) {
                if let stale {
                    Text(stale).font(WidgetType.ui(11)).foregroundStyle(Palette.haze)
                        .padding(.trailing, 14).padding(.bottom, 12)
                }
            }
        }
    }
}

struct ContinueCard: View {
    let item: SnapshotContinue
    let width: CGFloat
    let artHeight: CGFloat
    let showsProgress: Bool
    @Environment(\.widgetFiles) private var files

    var body: some View {
        TapLink(WidgetLink.title(item.id)) {
            VStack(alignment: .leading, spacing: 0) {
                art
                Text(item.title)
                    .font(WidgetType.title(13))
                    .foregroundStyle(Palette.silk)
                    .lineLimit(1)
                    .lineBox(17)
                    .padding(.top, 6)
                    .widgetAccentable()
                Text(ContinuePresentation.line(item))
                    .font(WidgetType.ui(12))
                    .foregroundStyle(Palette.haze)
                    .lineLimit(1)
                    .lineBox(16)
                    .padding(.top, 1)
                if showsProgress {
                    Group {
                        if let progress = item.progress {
                            ProgressBar(fraction: progress)
                        } else {
                            Color.clear.frame(height: 3)
                        }
                    }
                    .padding(.top, 6)
                }
            }
            .frame(width: width, alignment: .leading)
        }
        .accessibilityLabel(ContinuePresentation.accessibilityLabel(item))
        // The play button over the art, beside the card's link rather than inside it.
        .overlay(alignment: .topLeading) {
            PlayCircle(id: item.id, episode: item.episode, label: "Play episode \(item.episode) of \(item.title)")
                .padding(8)
                .frame(width: width, height: artHeight, alignment: .bottomTrailing)
        }
    }

    @ViewBuilder private var art: some View {
        if ArtImage.exists(item.art.wide, in: files) {
            ArtFrame(name: item.art.wide, width: width, height: artHeight, radius: 12)
        } else {
            // No banner: the poster at the art's height, never stretched into the banner's place.
            HStack(spacing: 0) {
                if ArtImage.exists(item.art.poster, in: files) {
                    ArtFrame(name: item.art.poster, width: (artHeight * 2 / 3).rounded(), height: artHeight, radius: 8)
                }
                Spacer(minLength: 0)
            }
            .frame(width: width, height: artHeight)
        }
    }
}

/// Large: the next episode with its one amber action, then two more shows.
struct ContinueLarge: View {
    let items: [SnapshotContinue]
    let stale: String?
    @Environment(\.widgetFiles) private var files

    var body: some View {
        let first = items[0]
        VStack(alignment: .leading, spacing: 0) {
            if ArtImage.exists(first.art.wide, in: files) {
                ArtFrame(name: first.art.wide, width: nil, height: 112, radius: 14)
                titleBlock(first).padding(.top, 8)
            } else {
                // No banner: a compact header with the poster beside the title.
                HStack(alignment: .bottom, spacing: 12) {
                    if ArtImage.exists(first.art.poster, in: files) {
                        ArtFrame(name: first.art.poster, width: 64, height: 96, radius: 8)
                    }
                    titleBlock(first)
                }
            }
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 5) {
                    if let stale {
                        Text(stale).font(WidgetType.ui(12)).foregroundStyle(Palette.haze)
                    } else {
                        if let progress = first.progress { ProgressBar(fraction: progress) }
                        if let left = ContinuePresentation.timeLeft(first) {
                            Text(left).font(WidgetType.ui(12)).foregroundStyle(Palette.haze).lineBox(14)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                TapLink(WidgetLink.play(first.id, episode: first.episode)) {
                    HStack(spacing: 6) {
                        Image(systemName: "play.fill").font(.system(size: 10))
                        Text(ContinuePresentation.action(first)).font(WidgetType.ui(14, weight: .semibold))
                    }
                    .foregroundStyle(Palette.ground)
                    .padding(.horizontal, 14)
                    .frame(height: 32)
                    .background(Capsule().fill(Palette.amber))
                    .widgetAccentable()
                }
                .accessibilityLabel("\(ContinuePresentation.action(first)) of \(first.title)")
            }
            .frame(height: 32)
            .padding(.top, 8)
            if items.count > 1 {
                Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.top, 8)
                ForEach(items.dropFirst()) { item in
                    ContinueRow(item: item, showsProgress: stale == nil)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(14)
    }

    private func titleBlock(_ item: SnapshotContinue) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(item.title)
                .font(WidgetType.title(17))
                .foregroundStyle(Palette.silk)
                .lineLimit(1)
                .lineBox(22)
                .widgetAccentable()
            Text(ContinuePresentation.episodeLine(item))
                .font(WidgetType.ui(13))
                .foregroundStyle(Palette.haze)
                .lineLimit(1)
                .lineBox(18)
        }
        .accessibilityElement(children: .combine)
    }
}

struct ContinueRow: View {
    let item: SnapshotContinue
    let showsProgress: Bool
    @Environment(\.widgetFiles) private var files

    var body: some View {
        TapLink(WidgetLink.title(item.id)) {
            HStack(spacing: 10) {
                if ArtImage.exists(item.art.poster, in: files) {
                    ArtFrame(name: item.art.poster, width: 32, height: 48, radius: 6)
                }
                VStack(alignment: .leading, spacing: 3) {
                    Text(item.title)
                        .font(WidgetType.title(14))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(1)
                        .lineBox(18)
                        .widgetAccentable()
                    HStack(spacing: 8) {
                        Text(ContinuePresentation.line(item))
                            .font(WidgetType.ui(12))
                            .foregroundStyle(Palette.haze)
                            .lineLimit(1)
                        if showsProgress, let progress = item.progress {
                            ProgressBar(fraction: progress, width: 120)
                        }
                    }
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(Palette.haze)
                    .accessibilityHidden(true)
            }
            .frame(height: 56)
        }
        .accessibilityLabel(ContinuePresentation.accessibilityLabel(item))
    }
}

/// Caught up: the next episode due from your list in place of a blank widget.
struct CaughtUp: View {
    let next: SnapshotNextAiring?
    let art: Art
    let small: Bool
    let now: Date
    let format: WidgetFormat
    @Environment(\.widgetFiles) private var files

    var body: some View {
        if small {
            MessageWidget(
                title: "Caught up",
                message: next.map { "\($0.title) E\($0.episode) airs \(format.day($0.airsAt, from: now))." }
                    ?? "You’re caught up.",
                extra: {
                    if let next {
                        HStack(spacing: 6) {
                            WaterDot()
                            Text(format.capitalized(format.dayAndTime(next.airsAt, from: now)))
                        }
                        .font(WidgetType.ui(12))
                        .foregroundStyle(Palette.haze)
                        .padding(.top, 8)
                    }
                })
        } else {
            MessageWidget(
                title: "You’re caught up",
                message: next == nil ? "Nothing more to continue." : "Next from your list:",
                extra: {
                    if let next {
                        HStack(spacing: 10) {
                            if ArtImage.exists(art.wide, in: files) {
                                ArtFrame(name: art.wide, width: 64, height: 36, radius: 6)
                            } else if ArtImage.exists(art.poster, in: files) {
                                ArtFrame(name: art.poster, width: 24, height: 36, radius: 4)
                            }
                            VStack(alignment: .leading, spacing: 1) {
                                Text(next.title)
                                    .font(WidgetType.title(13))
                                    .foregroundStyle(Palette.silk)
                                    .lineLimit(1)
                                    .widgetAccentable()
                                HStack(spacing: 5) {
                                    WaterDot()
                                    Text(format.airs(next, from: now))
                                }
                                .font(WidgetType.ui(12))
                                .foregroundStyle(Palette.haze)
                            }
                        }
                        .padding(.top, 10)
                    }
                })
        }
    }
}

/// A widget with nothing to show: what it would show, and how to get there.
struct MessageWidget<Extra: View>: View {
    let title: String
    let message: String
    var action: String?
    @ViewBuilder var extra: () -> Extra

    init(title: String, message: String, action: String? = nil, @ViewBuilder extra: @escaping () -> Extra) {
        self.title = title
        self.message = message
        self.action = action
        self.extra = extra
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(WidgetType.ui(13, weight: .semibold))
                .foregroundStyle(Palette.silk)
                .widgetAccentable()
            Text(message)
                .font(WidgetType.ui(13))
                .foregroundStyle(Palette.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
            extra()
            if let action {
                Text(action)
                    .font(WidgetType.ui(13, weight: .semibold))
                    .foregroundStyle(Palette.silk)
                    .padding(.top, 8)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14)
        .padding(.top, 14)
        .padding(.bottom, 12)
    }
}

extension MessageWidget where Extra == EmptyView {
    init(title: String, message: String, action: String? = nil) {
        self.init(title: title, message: message, action: action, extra: { EmptyView() })
    }
}
