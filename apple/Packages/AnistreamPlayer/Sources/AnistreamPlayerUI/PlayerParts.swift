// What the Mac and iPhone players share: the picture, the starting screen, the seek line and the
// cards over the video. Sizes are the boards' (MacPlayer, PhonePlayer…); `PlayerMetrics` holds
// the ones that differ.

import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The sizes that differ between the Mac's player and the iPhone's.
struct PlayerMetrics {
    var touch: Bool
    var body: CGFloat { touch ? 15 : 13 }
    var button: CGFloat { touch ? 44 : 28 }
    var skipHeight: CGFloat { touch ? 44 : 34 }
    var cardRadius: CGFloat { touch ? 20 : 16 }
    var still: CGSize { touch ? CGSize(width: 96, height: 54) : CGSize(width: 112, height: 63) }
    /// The corner card's distance from the trailing and bottom edges.
    /// How far the subtitles lift while the controls are up, as the boards draw them: from
    /// 18 pt above the bottom to 78 on the iPhone, from 34 to 62 on the Mac.
    var subtitleLift: CGFloat { touch ? 60 : 28 }
    var cornerInset: EdgeInsets {
        touch ? EdgeInsets(top: 0, leading: 0, bottom: 84, trailing: 56) : EdgeInsets(top: 0, leading: 0, bottom: 160, trailing: 16)
    }
    /// Labels as each platform writes buttons: title case on the Mac.
    func label(_ phone: String, mac: String) -> String { touch ? phone : mac }

    static let mac = PlayerMetrics(touch: false)
    static let phone = PlayerMetrics(touch: true)
}

// MARK: The picture

/// mpv's layer, with the model told when it resizes and which screen it is on.
struct PlayerPicture: View {
    let model: PlayerModel

    var body: some View {
        if let layer = model.layer {
            PlayerSurface(
                layer: layer,
                resized: { [weak model] in model?.surfaceResized() },
                display: { [weak model] display in model?.surfaceDisplay(display) },
                pointer: { [weak model] in model?.poke() })
        }
    }
}

/// From the playback staged to the first frame: the title's wide art at the screen's full size
/// under a calm ground scrim (the banner, else a still already seen, else the ground alone),
/// the title, "Starting episode 7" over a thin amber line that sweeps while it waits, and what
/// the wait is on below. After 20 s it offers another source, and to stop. When the playback
/// never played, it says so: "Couldn't play episode 7", with Choose source and Close. Between
/// episodes the last frame fades through the ground colour, never black.
struct StartingCurtain: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        // The title the engine names with the starting screen, else the one staged: its art by
        // the hero-art rule where the title is known here, else the banner the engine sends.
        let id = model.starting?.id ?? model.staged?.id
        let entry = id.flatMap { store.titles[$0] }
        let url = entry.map { store.heroArt($0).url } ?? model.starting?.bannerUrl
        let image = url.flatMap { store.artwork.image($0) }
        GeometryReader { proxy in
            ZStack {
                Palette.ground
                if let image {
                    FilledArt(image: image)
                        .transition(.opacity)
                    Palette.ground.opacity(0.55)
                }
                block
                    .frame(width: min(520, max(0, proxy.size.width - 2 * (metrics.touch ? 56 : 48))), alignment: .leading)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .animation(.fadeIn, value: image != nil)
        .clipped()
        .task(id: url) {
            // As sharp as the screen, which the player may come to fill.
            if let url { store.artwork.request(url, maxPixelSize: ScreenPixels.longSide) }
        }
        .task(id: entry?.id) {
            // A title with no banner shows a still already seen, which the episode list has.
            if let entry, entry.bannerUrl == nil, store.titles.episodes[entry.id] == nil {
                store.loadEpisodes(entry.id)
            }
        }
    }

    @ViewBuilder
    private var block: some View {
        let episode = model.startingEpisode ?? ""
        VStack(alignment: .leading, spacing: 0) {
            Text(model.showTitle)
                .font(.showTitle(size: metrics.touch ? 22 : 30))
                .lineLimit(2)
            if model.failure != nil {
                Text("Couldn't play episode \(episode)")
                    .font(.system(size: metrics.touch ? 17 : 15, weight: .semibold))
                    .padding(.top, 10)
                Text("No source had a stream that played. Another may.")
                    .font(.system(size: 13))
                    .foregroundStyle(.haze)
                    .padding(.top, 4)
                HStack(spacing: 8) {
                    Button(metrics.label("Choose source", mac: "Choose Source")) { model.chooseSource() }
                        .buttonStyle(PillButtonStyle(kind: .primary, metrics: metrics))
                        .keyboardShortcut(.defaultAction)
                    Button("Close") { model.viewerClosed() }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                        .keyboardShortcut(.cancelAction)
                }
                .padding(.top, 16)
            } else {
                let starting = model.starting
                let note = starting.flatMap { PlayerModel.startingNote(in: store.jobs.running, episode: $0.episode) }
                Text("Starting episode \(episode)")
                    .font(.system(size: metrics.touch ? 17 : 15, weight: .medium))
                    .padding(.top, 8)
                LoaderLine(reduceMotion: reduceMotion)
                    .frame(height: 2)
                    .padding(.top, 12)
                // Its place kept while there is nothing to say, so nothing moves when there is.
                Text(starting?.wait(at: model.clock, note: note) ?? " ")
                    .font(.system(size: 13))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .contentTransition(.opacity)
                    .padding(.top, 10)
                if starting?.offersChoice(at: model.clock) == true {
                    HStack(spacing: 8) {
                        Button(metrics.label("Choose source", mac: "Choose Source")) { model.chooseSource() }
                            .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                        Button("Stop") { model.viewerClosed() }
                            .buttonStyle(PillButtonStyle(kind: .quiet, metrics: metrics))
                    }
                    .padding(.top, 16)
                    .transition(.opacity)
                }
            }
            if model.choosingSource {
                SourceList(model: model, metrics: metrics)
                    .padding(.top, 16)
                    .transition(.opacity)
            }
        }
        .foregroundStyle(.silk)
        .shadow(color: Color(rgb: 0x0A0C18, opacity: 0.5), radius: 1, y: 1)
        .animation(.standard, value: model.failure)
        .animation(.standard, value: model.choosingSource)
        .accessibilityElement(children: .contain)
    }
}

/// Indeterminate progress as a thin amber line: a stretch of amber easing across a faint track,
/// back and forth, calm. Still, with Reduce Motion.
struct LoaderLine: View {
    let reduceMotion: Bool
    @State private var across = false

    var body: some View {
        GeometryReader { proxy in
            let stretch = proxy.size.width * 0.32
            ZStack(alignment: .leading) {
                Capsule().fill(Palette.silk.opacity(0.14))
                if reduceMotion {
                    Capsule().fill(Palette.amber.opacity(0.8))
                } else {
                    Capsule()
                        .fill(Palette.amber)
                        .frame(width: stretch)
                        .offset(x: across ? proxy.size.width - stretch : 0)
                }
            }
        }
        .onAppear {
            guard !reduceMotion else { return }
            // The standard spring's shape, slowed to a calm sweep; never a bounce.
            withAnimation(.spring(duration: 1.4, bounce: 0).repeatForever(autoreverses: true)) { across = true }
        }
        .accessibilityElement()
        .accessibilityLabel("Starting")
        .accessibilityAddTraits(.updatesFrequently)
    }
}

/// The episode's releases, to play one instead: the engine's list, best first.
struct SourceList: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Sources").font(.system(size: 13, weight: .semibold)).foregroundStyle(.haze)
                Spacer()
                Button("Cancel") { model.cancelChoosingSource() }
                    .buttonStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(.haze)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            if let choices = model.sourceChoices {
                if choices.isEmpty {
                    Text("No releases found for this episode.")
                        .font(.system(size: 13)).foregroundStyle(.haze)
                        .padding(10)
                } else {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 0) {
                            ForEach(choices, id: \.id) { candidate in
                                MenuRow(
                                    title: candidate.title, detail: Self.detail(candidate), checked: candidate.autoPick,
                                    metrics: metrics
                                ) { model.play(candidate) }
                            }
                        }
                        .padding(.horizontal, 5)
                        .padding(.bottom, 5)
                    }
                    .frame(maxHeight: metrics.touch ? 180 : 220)
                }
            } else {
                Text("Finding releases…").font(.system(size: 13)).foregroundStyle(.haze).padding(10)
            }
        }
        .overVideo(radius: 12)
    }

    /// "1080p · 120 seeders · 1.4 GB · lantern-archive".
    static func detail(_ candidate: SourceCandidate) -> String {
        var parts: [String] = []
        if let quality = candidate.quality { parts.append("\(quality)p") }
        if let seeders = candidate.seeders { parts.append("\(seeders) seeders") }
        if let size = candidate.size { parts.append(size) }
        if candidate.dubbed { parts.append("dub") } else if candidate.dualAudio { parts.append("dual audio") }
        parts.append(candidate.providerId)
        return parts.joined(separator: " · ")
    }
}

/// The long side of the main screen in pixels, for art the player may come to fill.
@MainActor
enum ScreenPixels {
    static var longSide: Int {
        #if os(macOS)
        let screen = NSScreen.main
        let size = screen?.frame.size ?? CGSize(width: 1920, height: 1080)
        return Int(max(size.width, size.height) * (screen?.backingScaleFactor ?? 2))
        #else
        let screen = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.screen }.first
        let size = screen?.nativeBounds.size ?? CGSize(width: 2796, height: 1290)
        return Int(max(size.width, size.height))
        #endif
    }
}

/// Art filling the space it is given, cropped to it, and never asking for more: a banner is
/// often 4.75:1, and filled into a 16:9 player as a plain `scaledToFill()` it would have made
/// the player 4.75 times its height wide, with the picture and the controls laid out across it.
struct FilledArt: View {
    let image: CGImage

    var body: some View {
        Color.clear
            .overlay {
                Image(decorative: image, scale: 1)
                    .resizable()
                    .scaledToFill()
            }
            .clipped()
    }
}

// MARK: The seek line

/// The playhead across the file: played time amber, openings and endings water, the rest silk at
/// 22%. Dragging scrubs, with keyframe seeks as it moves and an exact one on release; the frame
/// under the thumb shows above it with the time and the chapter. Moving the finger up slows the
/// scrub to half speed past 48 pt and a quarter past 96 pt, as ⌥ does on the Mac, and crossing
/// an opening or ending mark gives a selection tick.
struct SeekLine: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    var showsPreview = true
    @State private var touched = false
    @State private var pace = ScrubPace()

    private var lineHeight: CGFloat { metrics.touch ? 6 : (touched ? 6 : 4) }

    var body: some View {
        GeometryReader { proxy in
            let duration = model.duration ?? 0
            let segments = segments(duration)
            let gap: CGFloat = 2
            let usable = max(0, proxy.size.width - gap * CGFloat(max(0, segments.count - 1)))
            HStack(spacing: gap) {
                ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                    let width = duration > 0 ? usable * (segment.end - segment.start) / duration : 0
                    let played = min(1, max(0, (model.position - segment.start) / max(segment.end - segment.start, 0.001)))
                    Capsule()
                        .fill(track(segment.kind))
                        .overlay(alignment: .leading) {
                            Rectangle()
                                .fill(.amber)
                                .frame(width: width * played)
                        }
                        .clipShape(.capsule)
                        .frame(width: width, height: lineHeight)
                }
            }
            .frame(width: proxy.size.width, height: proxy.size.height, alignment: .leading)
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        guard duration > 0 else { return }
                        let width = proxy.size.width
                        if !touched {
                            touched = true
                            model.scrub(to: pace.start(at: value.startLocation.x, width: width, duration: duration))
                        }
                        let lifted = max(0, proxy.size.height / 2 - value.location.y)
                        let time = pace.time(
                            at: value.location.x, lifted: lifted, option: Self.optionHeld, width: width,
                            duration: duration, current: model.scrubbing ?? model.position)
                        model.scrub(to: time, pace: pace.factor)
                    }
                    .onEnded { _ in
                        touched = false
                        model.endScrub()
                    }
            )
            .onContinuousHover { phase in
                #if os(macOS)
                // The Mac's pointer shows the bubble without a click; a finger only by dragging.
                guard showsPreview, duration > 0 else { return }
                switch phase {
                case .active(let location):
                    let fraction = min(1, max(0, location.x / max(proxy.size.width, 1)))
                    model.hover(at: fraction * duration)
                case .ended:
                    model.hover(at: nil)
                }
                #endif
            }
            .overlay(alignment: .topLeading) {
                if showsPreview, let time = model.scrubbing ?? model.hovering, duration > 0 {
                    let width = ScrubBubble.width
                    let x = proxy.size.width * time / duration - width / 2
                    ScrubBubble(model: model, time: time)
                        .frame(width: width, alignment: .bottom)
                        .offset(
                            x: min(max(x, -16), proxy.size.width - width + 16),
                            y: proxy.size.height / 2 - lineHeight - 8 - ScrubBubble.height(model))
                        .allowsHitTesting(false)
                        .transition(.opacity)
                }
            }
        }
        .frame(height: metrics.touch ? 44 : 28)
        .animation(.quick, value: touched)
        .sensoryFeedback(trigger: markUnderThumb) { old, new in
            guard let old, let new, old != new else { return nil }
            #if os(iOS)
            return .selection
            #else
            return .alignment
            #endif
        }
        .accessibilityElement()
        .accessibilityLabel("Position")
        .accessibilityValue(
            "\(PlayerClock.spoken(model.position)) of \(PlayerClock.spoken(model.duration ?? 0))")
        .accessibilityAdjustableAction { direction in
            model.seek(by: direction == .increment ? 10 : -10)
        }
    }

    /// While scrubbing, the opening or ending under the thumb, or none: a change is a mark crossed.
    private var markUnderThumb: Int? {
        guard let time = model.scrubbing else { return nil }
        let segments = model.segments
        guard let index = segments.firstIndex(where: { time >= $0.start && time < $0.end }) else { return nil }
        return segments[index].kind == .plain ? -1 : index
    }

    private static var optionHeld: Bool {
        #if os(macOS)
        NSEvent.modifierFlags.contains(.option)
        #else
        false
        #endif
    }

    private func segments(_ duration: Double) -> [SeekSegment] {
        let marked = model.segments
        guard marked.isEmpty else { return marked }
        return duration > 0 ? [SeekSegment(kind: .plain, start: 0, end: duration)] : []
    }

    private func track(_ kind: SeekSegment.Kind) -> Color {
        switch kind {
        case .plain: Palette.silk.opacity(0.22)
        case .opening, .ending: Palette.water.opacity(0.72)
        }
    }
}

/// Above the scrubbing thumb: the pace when it is slowed, the frame there from the hidden mpv
/// (none in Low Power Mode), and the time with the chapter's name.
struct ScrubBubble: View {
    let model: PlayerModel
    let time: Double

    static let width: CGFloat = 220

    /// Its height as it is drawn now: the labels, and the frame when there is one.
    @MainActor
    static func height(_ model: PlayerModel) -> CGFloat {
        var height: CGFloat = 22
        if ScrubPace.label(model.scrubPace) != nil { height += 22 + 6 }
        if model.previewsOn, model.preview.layer != nil { height += SeekPreview.size.height + 6 }
        return height
    }

    var body: some View {
        VStack(spacing: 6) {
            if let pace = ScrubPace.label(model.scrubPace) {
                Text(pace)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.silk)
                    .padding(.horizontal, 8)
                    .frame(height: 22)
                    .background(.overVideo(0.72), in: .capsule)
            }
            if model.previewsOn, let layer = model.preview.layer {
                ZStack {
                    Palette.ground
                    PlayerSurface(
                        layer: layer,
                        resized: { [weak model] in model?.preview.resized() },
                        display: { _ in })
                        .opacity(model.preview.ready ? 1 : 0)
                }
                .frame(width: SeekPreview.size.width, height: SeekPreview.size.height)
                .clipShape(.rect(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
            }
            Text(model.chapter(at: time).map { "\(PlayerClock.label(time)) · \($0)" } ?? PlayerClock.label(time))
                .font(.system(size: 13, weight: .semibold))
                .monospacedDigit()
                .foregroundStyle(.silk)
                .padding(.horizontal, 8)
                .frame(height: 22)
                .background(.overVideo(0.72), in: .capsule)
        }
    }
}

/// Elapsed or remaining time in tabular digits.
struct PlayerTime: View {
    let seconds: Double
    var remaining = false
    let metrics: PlayerMetrics

    var body: some View {
        Text(remaining ? PlayerClock.remaining(seconds) : PlayerClock.label(seconds))
            .font(.system(size: 13))
            .monospacedDigit()
            .lineLimit(1)
    }
}

// MARK: Cards

/// Glass over video: the ground at 72% with a hairline inside its edge.
struct OverVideo: ViewModifier {
    let radius: CGFloat

    func body(content: Content) -> some View {
        content
            .background(.overVideo(0.72), in: .rect(cornerRadius: radius))
            .overlay {
                RoundedRectangle(cornerRadius: radius)
                    .strokeBorder(Palette.glassOverArtEdge, lineWidth: 1)
            }
    }
}

extension View {
    func overVideo(radius: CGFloat) -> some View { modifier(OverVideo(radius: radius)) }

    /// The 1 px dark outline labels over bright video get.
    func overVideoText() -> some View {
        shadow(color: Color(rgb: 0x0A0C18, opacity: 0.7), radius: 0.5, y: 1)
    }
}

/// A pill: Play Now in amber, the rest raised.
struct PillButtonStyle: ButtonStyle {
    enum Kind { case primary, secondary, quiet }
    let kind: Kind
    let metrics: PlayerMetrics

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: metrics.body, weight: kind == .primary ? .semibold : .regular))
            .foregroundStyle(kind == .primary ? Palette.ground : kind == .quiet ? Palette.haze : Palette.silk)
            .padding(.horizontal, metrics.touch ? (kind == .primary ? 18 : 16) : (kind == .primary ? 14 : 12))
            .frame(height: metrics.button)
            .background {
                switch kind {
                case .primary: Capsule().fill(.amber)
                case .secondary: Capsule().fill(.raisedSelected)
                case .quiet: Color.clear
                }
            }
            .opacity(configuration.isPressed ? 0.8 : 1)
            .contentShape(.capsule)
    }
}

/// Up Next: the next episode's still, its number and name, the countdown while it runs, and
/// Cancel and Play now. Opacity only; it never rises.
struct UpNextCardView: View {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let card = model.upNext {
            let countdown = model.countdown
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 12) {
                    still(card.next.thumbnail)
                        .frame(width: metrics.still.width, height: metrics.still.height)
                        .clipShape(.rect(cornerRadius: 6))
                    VStack(alignment: .leading, spacing: metrics.touch ? 1 : 2) {
                        Text("UP NEXT")
                            .font(.system(size: 11, weight: .medium))
                            .tracking(0.88)
                            .foregroundStyle(.haze)
                        Text("Episode \(card.next.number)")
                            .font(.system(size: metrics.body, weight: .semibold))
                        if let title = card.next.title {
                            Text(title)
                                .font(.system(size: 13))
                                .foregroundStyle(.haze)
                                .lineLimit(1)
                        }
                    }
                }
                if let countdown {
                    Capsule()
                        .fill(Palette.silk.opacity(0.22))
                        .overlay(alignment: .leading) {
                            GeometryReader { proxy in
                                Capsule()
                                    .fill(.amber)
                                    .frame(width: proxy.size.width * countdown.progress)
                                    .animation(.linear(duration: 0.25), value: countdown.progress)
                            }
                        }
                        .frame(height: 3)
                        .padding(.top, 10)
                        .accessibilityLabel("Countdown to episode \(card.next.number)")
                }
                HStack(spacing: 8) {
                    Text(countdown.map { "Plays in \($0.seconds) s" } ?? "")
                        .font(.system(size: 13))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button("Cancel") { model.cancelNext() }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                        .keyboardShortcut(.cancelAction)
                    Button(metrics.label("Play now", mac: "Play Now")) { model.playNext() }
                        .buttonStyle(PillButtonStyle(kind: .primary, metrics: metrics))
                        .keyboardShortcut(.defaultAction)
                        .sensoryFeedback(.impact(weight: .light), trigger: card.played) { _, played in played }
                }
                .padding(.top, metrics.touch ? 4 : 10)
            }
            .padding(EdgeInsets(top: 12, leading: 12, bottom: metrics.touch ? 8 : 12, trailing: 12))
            .frame(width: 320)
            .overVideo(radius: metrics.cardRadius)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Up next")
        }
    }

    @ViewBuilder
    private func still(_ url: String?) -> some View {
        let image = url.flatMap { store.artwork.image($0) }
        ZStack {
            CoverPlate(color: nil)
            if let image {
                FilledArt(image: image).transition(.opacity)
            }
        }
        .animation(.fadeIn, value: image != nil)
        .task(id: url) {
            if let url { store.artwork.request(url, maxPixelSize: 320) }
        }
    }
}

/// "Skip opening", while the playhead is in it; dims after 8 s and comes back under the pointer.
struct SkipButton: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    @State private var hovering = false

    var body: some View {
        Group { button }
            // T4: a light impact when the skip is taken, and not when the offer lapses.
            .sensoryFeedback(.impact(weight: .light), trigger: model.skipsTaken)
    }

    @ViewBuilder
    private var button: some View {
        if let skip = model.skip {
            Button {
                model.takeSkip()
            } label: {
                HStack(spacing: 8) {
                    Text(metrics.touch ? skip.label : skip.label.capitalized)
                    Image(systemName: "forward.end.fill")
                        .font(.system(size: metrics.touch ? 15 : 13))
                }
                .font(.system(size: metrics.body, weight: .semibold))
                .foregroundStyle(.silk)
                .padding(.horizontal, metrics.touch ? 18 : 16)
                .frame(height: metrics.skipHeight)
                .overVideo(radius: metrics.skipHeight / 2)
                .contentShape(.capsule)
            }
            .buttonStyle(.plain)
            .accessibilityInputLabels([Text(skip.label), Text("Skip")])
            .opacity(model.skipDimmed && !hovering ? 0.55 : 1)
            .animation(.standard, value: model.skipDimmed)
            .onHover { hovering = $0 }
        }
    }
}

/// "Skipped opening · Undo", for 6 s of playback.
struct UndoNote: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let note = model.undo {
            HStack(spacing: 6) {
                Text(note.label)
                Text("·").foregroundStyle(.haze)
                Button("Undo") { model.undoSkip() }
                    .buttonStyle(.plain)
                    .fontWeight(.semibold)
                    .foregroundStyle(.amber)
            }
            .font(.system(size: metrics.body))
            .foregroundStyle(.silk)
            .padding(.horizontal, metrics.touch ? 18 : 16)
            .frame(height: metrics.skipHeight)
            .overVideo(radius: metrics.skipHeight / 2)
        }
    }
}

/// "Source is buffering · 9 s", counting up, with Switch source and Keep waiting.
struct StallCardView: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let stall = model.stall {
            VStack(alignment: .leading, spacing: 10) {
                Text("Source is buffering · \(stall.held(at: model.clock)) s")
                    .font(.system(size: metrics.body, weight: .semibold))
                    .monospacedDigit()
                HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    Button(metrics.label("Keep waiting", mac: "Keep Waiting")) { model.keepWaiting() }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                        .keyboardShortcut(.cancelAction)
                    Button(metrics.label("Switch source", mac: "Switch Source")) { model.switchSource() }
                        .buttonStyle(PillButtonStyle(kind: .primary, metrics: metrics))
                        .keyboardShortcut(.defaultAction)
                }
            }
            .foregroundStyle(.silk)
            .padding(12)
            .frame(width: 320)
            .overVideo(radius: metrics.cardRadius)
            .accessibilityElement(children: .contain)
        }
    }
}

/// The one card in the bottom trailing corner: stall, then Undo, then Up Next, then Skip.
struct CornerCard: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            switch model.corner {
            case .stall: StallCardView(model: model, metrics: metrics).transition(.opacity)
            case .undo: UndoNote(model: model, metrics: metrics).transition(.opacity)
            case .upNext: UpNextCardView(model: model, metrics: metrics).transition(.opacity)
            case .skip: SkipButton(model: model, metrics: metrics).transition(.opacity)
            case nil: EmptyView()
            }
        }
        .animation(model.corner == nil ? .quick : .standard, value: model.corner)
    }
}

/// "Resumed at 12:04", with Start over, for 6 s of playback.
struct ResumeNoteView: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let note = model.resume {
            HStack(spacing: 10) {
                Text(note.label).monospacedDigit()
                Button(metrics.label("Start over", mac: "Start Over")) { model.startOver() }
                    .buttonStyle(.plain)
                    .fontWeight(.semibold)
                    .foregroundStyle(.amber)
            }
            .font(.system(size: metrics.body))
            .foregroundStyle(.silk)
            .padding(.horizontal, 14)
            .frame(height: metrics.touch ? 44 : 32)
            .overVideo(radius: metrics.touch ? 22 : 16)
            .transition(.opacity)
        }
    }
}

/// "Still watching?" after three episodes in a row with nobody there: playback is paused.
struct StillWatchingDialog: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        VStack(spacing: 0) {
            Text("Still watching?")
                .font(.system(size: 17, weight: .semibold))
            Text("Paused after \(model.stillWatchingAfter) episodes in a row.")
                .font(.system(size: 15))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            HStack(spacing: 8) {
                Button("Stop") { model.stopWatching() }
                    .buttonStyle(PillButtonStyle(kind: .quiet, metrics: .phone))
                Spacer(minLength: 0)
                Button(metrics.label("Continue watching", mac: "Continue Watching")) { model.continueWatching() }
                    .buttonStyle(PillButtonStyle(kind: .secondary, metrics: .phone))
                    .fontWeight(.semibold)
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 16)
        }
        .foregroundStyle(.silk)
        .multilineTextAlignment(.center)
        .padding(EdgeInsets(top: 20, leading: 20, bottom: 16, trailing: 20))
        .frame(width: 300)
        .overVideo(radius: 24)
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
    }
}

/// A message on the video, at the top.
struct PlayerAlertView: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let alert = model.alert {
            Label(alert.text, systemImage: alert.kind == .alert ? "exclamationmark.triangle.fill" : "info.circle")
                .font(.system(size: metrics.body))
                .foregroundStyle(alert.kind == .alert ? Palette.vermilion : Palette.silk)
                .padding(.horizontal, 14)
                .frame(height: metrics.touch ? 44 : 32)
                .overVideo(radius: metrics.touch ? 22 : 16)
                .transition(.opacity)
        }
    }
}

/// What a viewer's pick sends: the engine's menus as rows.
struct MenuRow: View {
    let title: String
    var detail: String? = nil
    let checked: Bool
    let metrics: PlayerMetrics
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: "checkmark")
                    .font(.system(size: 11, weight: .semibold))
                    .frame(width: 16)
                    .opacity(checked ? 1 : 0)
                Text(title).lineLimit(1)
                Spacer(minLength: 8)
                if let detail {
                    Text(detail).font(.system(size: 12)).foregroundStyle(.haze).lineLimit(1)
                }
            }
            .font(.system(size: metrics.body))
            .foregroundStyle(.silk)
            .padding(.leading, 6)
            .padding(.trailing, 10)
            .frame(height: metrics.touch ? 44 : 24)
            .contentShape(.rect)
        }
        .buttonStyle(MenuRowStyle())
        .accessibilityAddTraits(checked ? .isSelected : [])
    }
}

struct MenuRowStyle: ButtonStyle {
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(Palette.silk.opacity(configuration.isPressed ? 0.16 : 0))
            )
    }
}

struct MenuHeading: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(.haze)
            .padding(.leading, 26)
            .padding(.bottom, 3)
            .frame(height: 22, alignment: .bottomLeading)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct MenuDivider: View {
    var body: some View {
        Rectangle()
            .fill(Palette.glassRule)
            .frame(height: 1)
            .padding(.vertical, 5)
            .padding(.horizontal, 10)
    }
}

/// The Subtitles and Audio lists: Off, then each track by language with its title.
struct TrackLists: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        MenuHeading(title: "Subtitles")
        MenuRow(title: "Off", checked: model.subtitleTrack == nil, metrics: metrics) {
            model.selectSubtitle(nil)
        }
        ForEach(model.subtitleTracks) { track in
            MenuRow(title: track.name, checked: model.subtitleTrack == track.id, metrics: metrics) {
                model.selectSubtitle(track.id)
            }
        }
        MenuDivider()
        MenuHeading(title: "Audio")
        ForEach(model.audioTracks) { track in
            MenuRow(title: track.name, checked: model.audioTrack == track.id, metrics: metrics) {
                model.selectAudio(track.id)
            }
        }
        MenuDivider()
        Text("Remembered for this show")
            .font(.system(size: 12))
            .foregroundStyle(.haze)
            .padding(.leading, 26)
            .frame(height: 24, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The Episodes list, from the engine's menus.
struct EpisodeList: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        MenuHeading(title: "Episodes")
        ForEach(model.menus?.episodes ?? [], id: \.number) { row in
            MenuRow(
                title: row.title.map { "\(row.label) · \($0)" } ?? row.label,
                detail: row.filler ? "Filler" : row.watched ? "Watched" : nil,
                checked: row.current, metrics: metrics
            ) {
                model.choose(.episode(row.number))
            }
        }
    }
}

/// Speed, Upscaling, Night mode and Sources.
struct MoreLists: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    static let speeds: [Double] = [0.5, 0.75, 1, 1.25, 1.5, 2]

    var body: some View {
        MenuHeading(title: "Speed")
        ForEach(Self.speeds, id: \.self) { speed in
            MenuRow(title: speedLabel(speed), checked: abs(model.speed - speed) < 0.01, metrics: metrics) {
                model.setSpeed(speed)
            }
        }
        if let menus = model.menus {
            MenuDivider()
            MenuHeading(title: "Upscaling")
            ForEach(menus.upscaling.options, id: \.id) { choice in
                MenuRow(title: choice.label, detail: choice.hint, checked: menus.upscaling.current == choice.id, metrics: metrics) {
                    model.setUpscaling(choice.id)
                }
            }
            MenuDivider()
            MenuRow(title: "Night mode", checked: menus.nightMode, metrics: metrics) {
                model.setNightMode(!menus.nightMode)
            }
            if !menus.sources.isEmpty {
                MenuDivider()
                MenuHeading(title: "Sources")
                ForEach(menus.sources, id: \.id) { source in
                    MenuRow(title: source.label, detail: source.detail, checked: source.current, metrics: metrics) {
                        model.choose(.source(source.id))
                    }
                }
            }
        }
    }
}

func speedLabel(_ speed: Double) -> String {
    String(format: speed == speed.rounded() ? "%.1f×" : "%g×", speed)
}
