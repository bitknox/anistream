// Pieces every screen uses: art over its plate, the meter, the buttons and the sync capsule.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// A cover or banner: the plate first, and the art fading in over it once it arrives.
struct ArtworkView: View {
    @Environment(AppStore.self) private var store
    let url: String?
    /// The cover's own colour, for the plate.
    var color: Color? = nil
    /// The most pixels the art needs on its long side.
    let maxPixelSize: Int
    /// A flat plate in place of the cover's colour, muted: the metadata's placeholder.
    var flat: Color? = nil
    /// Whether the art shown last stays while a new URL's is on its way, for `holdFor` at most,
    /// and then crossfades to it: for art that changes in place, such as a title page's banner
    /// across a season switch. Otherwise the plate shows meanwhile.
    var holds = false

    /// How long held art waits for the next: as long as T8 waits before it shows placeholders.
    static let holdFor: Duration = .milliseconds(600)

    /// The art on screen, and the URL it is.
    private struct Shown {
        let url: String
        let image: CGImage
    }

    @State private var last: Shown?
    /// Raised for each URL shown, so the newest art lies over the art it replaces.
    @State private var layer = 0.0

    /// Art arriving fades in, over the art it replaces where there was some, which stays whole
    /// beneath until then and only goes once covered: never the plate showing through between
    /// two images.
    private static let swap = AnyTransition.asymmetric(
        insertion: .opacity,
        removal: AnyTransition.opacity.animation(.fadeIn.delay(0.2))
    )

    var body: some View {
        let arrived = url.flatMap { url in store.artwork.image(url).map { Shown(url: url, image: $0) } }
        let shown = arrived ?? (holds ? last : nil)
        Color.clear
            .overlay {
                ZStack {
                    if let flat { Rectangle().fill(flat) } else { CoverPlate(color: color) }
                    if let shown {
                        // One image per URL: a new URL's crossfades in over the last one's,
                        // and a larger decode of the same URL takes its place unseen.
                        Image(decorative: shown.image, scale: 1)
                            .resizable()
                            .scaledToFill()
                            .id(shown.url)
                            .transition(Self.swap)
                            .zIndex(layer)
                    }
                }
                .animation(.fadeIn, value: shown?.url)
            }
            .clipped()
            .onChange(of: arrived?.url, initial: true) {
                if holds, let arrived { last = arrived }
            }
            .onChange(of: shown?.url) { layer += 1 }
            .task(id: url) {
                // Held art waits a moment for the next; past that the plate shows until it lands.
                if holds, arrived == nil, last != nil {
                    try? await Task.sleep(for: Self.holdFor)
                    if !Task.isCancelled, url.flatMap({ store.artwork.image($0) }) == nil { last = nil }
                }
            }
            .task(id: url) {
                // For as long as the art is on screen, so it is asked again if it fails.
                if let url { await store.artwork.show(url, maxPixelSize: maxPixelSize) }
            }
    }
}

/// A small card's wide art, in a grid that needs every card's frame, as Calendar's and This
/// Season's are: the banner, else the still of an episode already watched when the title's
/// episodes are here already (a calendar asks for no episodes), never one not yet seen, so never
/// the episode a Calendar tile is about. Else the cover filling the frame (`CoverFill`): cropped
/// around its focus, or blurred behind itself where it is too small to crop.
struct BannerArt: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let entry: Entry

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            let wide = entry.bannerUrl ?? store.titles.episodes[entry.id].flatMap {
                HeroArt.watchedStill($0, done: entry.progress?.done ?? 0)
            }
            if let wide {
                ArtworkView(
                    url: wide, color: entry.plateColor,
                    maxPixelSize: Int(max(size.width, size.height) * scale)
                )
            } else {
                CoverFill(entry: entry)
            }
        }
    }
}

/// Text cut to `lines`, with More to show the rest and Less to cut it again, only when there is
/// more to show.
struct ClampedText: View {
    let text: String
    var lines = 4
    @State private var expanded = false
    @State private var full: CGFloat = 0
    @State private var shown: CGFloat = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(text)
                .lineLimit(expanded ? nil : lines)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { shown = $0 }
                .background(alignment: .topLeading) {
                    Text(text)
                        .fixedSize(horizontal: false, vertical: true)
                        .hidden()
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { full = $0 }
                }
            if expanded || full > shown + 1 {
                Button(expanded ? L.shellLess : L.actionMore) {
                    withAnimation(.standard) { expanded.toggle() }
                }
                .buttonStyle(.plain)
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.silk)
            }
        }
    }
}

/// A progress meter: amber over the rule colour, rounded.
struct Meter: View {
    let fraction: Double
    var height: CGFloat = 3
    /// What VoiceOver says for it where the time left is known: "14 min left", not a percentage.
    var spoken: String?
    /// Silk, where the view's one amber is its main action (Home's cards).
    var silk = false

    var body: some View {
        Capsule()
            .fill(.rule)
            .overlay(alignment: .leading) {
                GeometryReader { proxy in
                    Capsule()
                        .fill(silk ? Palette.silk : Palette.amber)
                        .frame(width: proxy.size.width * min(max(fraction, 0), 1))
                }
            }
            .frame(height: height)
            .animation(.standard, value: fraction)
            .accessibilityElement()
            .accessibilityLabel(L.shellProgress)
            .accessibilityValue(spoken.map(Text.init) ?? Text(fraction, format: .percent.precision(.fractionLength(0))))
    }
}

/// The one amber action on a screen: a capsule with ground-coloured text.
struct PrimaryCapsuleStyle: ButtonStyle {
    var height: CGFloat = 34
    var fontSize: CGFloat = 13

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.board(fontSize, weight: .semibold))
            .monospacedDigit()
            .foregroundStyle(.ground)
            .modifier(CapsuleLines())
            .padding(.horizontal, height < 34 ? 14 : 18)
            .frame(minHeight: height)
            .background(.amber.opacity(configuration.isPressed ? 0.85 : 1), in: .capsule)
            .contentShape(.capsule)
            .pointerLift()
            .modifier(DimmedWhenDisabled())
    }
}

/// In the main button's place when nothing out is left to play: when the next episode airs, a
/// haze line with the water dot upcoming episodes have, and no amber on the page
/// ("Episode 10 airs Thursday 16:30").
struct AirsStatus: View {
    let text: String
    var height: CGFloat = 34
    var fontSize: CGFloat = 13
    /// Whether it spans its row, as the iPhone's main button does.
    var fills = false

    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(.water).frame(width: 6, height: 6)
            Text(text).foregroundStyle(.haze)
        }
        .font(.board(fontSize))
        .monospacedDigit()
        .lineLimit(2)
        .frame(maxWidth: fills ? .infinity : nil, alignment: .leading)
        .frame(minHeight: height)
        .accessibilityElement(children: .combine)
    }
}

/// In the main button's place when no source has the title, so its episodes are AniList's alone:
/// "Add a source to play", which opens Sources. In the secondary style, since nothing on the
/// page can play yet and amber would promise it could.
struct AddSourceButton: View {
    @Environment(Navigator.self) private var navigator
    /// Unused since every platform says it in sentence case; the title page still passes it.
    var sentence = false
    var height: CGFloat = 34
    var fontSize: CGFloat = 13
    /// Whether it spans its row, as the iPhone's main button does.
    var fills = false

    var body: some View {
        Button {
            navigator.openProviders()
        } label: {
            Text(AddSourceWords.play(sentence: sentence))
                .frame(maxWidth: fills ? .infinity : nil)
        }
        .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: fontSize))
        .accessibilityHint(L.shellOpensSources)
    }
}

/// What a title no source has offers in place of Play.
enum AddSourceWords {
    /// "Add a source to play". `sentence` is unused: every platform says it in sentence case.
    static func play(sentence: Bool = true) -> String {
        L.titleButtonAddSource
    }

    /// As a context menu says it.
    static var menu: String { play() }
}

/// A capsule's text: one line, and up to three at the accessibility sizes, where one would be
/// cut off.
struct CapsuleLines: ViewModifier {
    @Environment(\.dynamicTypeSize) private var typeSize

    func body(content: Content) -> some View {
        content
            .lineLimit(typeSize.isAccessibilitySize ? 3 : 1)
            .multilineTextAlignment(.center)
            .padding(.vertical, typeSize.isAccessibilitySize ? 8 : 0)
    }
}

extension View {
    /// The pointer over a row, a cell or a chip drawn plain: its highlight on iPad, and on the
    /// Mac the pointing hand, with no highlight, since Mac rows light on press and selection only
    /// (motion.md).
    func pointerHover() -> some View {
        #if os(iOS)
        hoverEffect(.highlight)
        #else
        pointingHand()
        #endif
    }

    /// The pointer over a list row: its highlight on iPad, and on the Mac the arrow, as Mac lists
    /// have it; the row lights on press and selection only.
    @ViewBuilder
    func rowHover() -> some View {
        #if os(iOS)
        hoverEffect(.highlight)
        #else
        self
        #endif
    }

    /// The pointer over a capsule button: its lift on iPad, and the pointing hand on the Mac.
    func pointerLift() -> some View {
        #if os(iOS)
        hoverEffect(.lift)
        #else
        pointingHand()
        #endif
    }
}

/// A custom button's look when it cannot be pressed: faded, as the system fades its own.
struct DimmedWhenDisabled: ViewModifier {
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        content.opacity(isEnabled ? 1 : 0.45)
    }
}

/// A secondary action: a raised capsule with a hairline edge on the ground, or the selected
/// raised colour on a raised surface.
struct SecondaryCapsuleStyle: ButtonStyle {
    var height: CGFloat = 28
    /// Whether it sits on a raised surface, such as the setup card.
    var raisedSurface = false
    var fontSize: CGFloat = 13

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.board(fontSize))
            .foregroundStyle(.silk)
            .modifier(CapsuleLines())
            .padding(.horizontal, 14)
            .frame(minHeight: height)
            .background(background(pressed: configuration.isPressed), in: .capsule)
            .overlay(Capsule().strokeBorder(raisedSurface ? .clear : Palette.faintRule, lineWidth: 1))
            .contentShape(.capsule)
            .pointerLift()
            .modifier(DimmedWhenDisabled())
    }

    private func background(pressed: Bool) -> Color {
        if raisedSurface { return pressed ? Palette.rule : Palette.raisedSelected }
        return pressed ? Palette.raisedSelected : Palette.raised
    }
}

/// The dot a state is shown with.
struct StateDot: View {
    let state: SyncSummary.State

    var body: some View {
        Group {
            switch state {
            case .signedOut:
                Circle().strokeBorder(.haze, lineWidth: 1.5)
            case .synced:
                Circle().fill(.water)
            case .waiting:
                Circle().fill(.haze)
            case .problem:
                Circle().fill(.vermilion)
            }
        }
        .frame(width: 6, height: 6)
        .accessibilityHidden(true)
    }
}

/// The toolbar's sync capsule: where sync stands, as a button that opens a popover of every
/// running job with its elapsed time and note, the TUI's activity row whole (native-ux, Mac).
struct SyncCapsule: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    /// Accounts in Settings, from the popover's foot.
    let open: () -> Void
    @State private var showing = false

    var body: some View {
        let summary = shown
        Button { showing.toggle() } label: {
            HStack(spacing: 6) {
                StateDot(state: summary.state)
                Text(summary.text)
                    .font(.mac(12, weight: .medium))
                    .foregroundStyle(summary.state == .signedOut ? .haze : .silk)
            }
            .padding(.horizontal, 4)
        }
        .accessibilityLabel(accessibilityLabel(summary))
        .accessibilityHint(L.shellSyncHint)
        .popover(isPresented: $showing, arrowEdge: .bottom) {
            JobsPopover {
                showing = false
                open()
            }
        }
    }

    /// Where sync stands, or on Home offline, Home's own line: offline is said once there.
    private var shown: SyncSummary {
        let summary = store.accounts.summary
        guard navigator.section == .home, navigator.titleOnMac == nil, let line = store.homeOfflineLine else {
            return summary
        }
        return SyncSummary(text: line, state: .waiting, tracker: summary.tracker)
    }

    private func accessibilityLabel(_ summary: SyncSummary) -> String {
        let text = summary.state == .signedOut
            ? L.shellSyncNotConnected(tracker: AccountsModel.name(summary.tracker ?? "anilist"))
            : summary.text
        return text
    }
}

/// Every job running, oldest first: what it does, how long it has run, and what it waits on.
struct JobsPopover: View {
    @Environment(AppStore.self) private var store
    let openAccounts: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(store.accounts.summary.text)
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.silk)
            if store.jobs.running.isEmpty {
                Text(L.shellNothingRunning)
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .padding(.top, 8)
            } else {
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(store.jobs.running) { job in
                            HStack(alignment: .firstTextBaseline, spacing: 12) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(job.activity.firstCapitalized)
                                        .foregroundStyle(.silk)
                                    if let note = job.note {
                                        Text(note).foregroundStyle(.haze)
                                    }
                                }
                                Spacer(minLength: 12)
                                Text(Self.elapsed(job))
                                    .monospacedDigit()
                                    .foregroundStyle(.haze)
                            }
                            .font(.mac(13))
                            .accessibilityElement(children: .combine)
                        }
                    }
                }
                .padding(.top, 10)
            }
            Rectangle().fill(.faintRule).frame(height: 1).padding(.vertical, 12)
            Button(L.accountsSettings, action: openAccounts)
                .buttonStyle(SecondaryCapsuleStyle())
        }
        .padding(16)
        .frame(width: 300, alignment: .leading)
    }

    /// "4s", "1m 12s".
    static func elapsed(_ job: Job) -> String {
        let seconds = Int((ContinuousClock.now - job.started).components.seconds)
        return seconds < 60
            ? L.durationSeconds(seconds: seconds)
            : L.durationMinutesSeconds(minutes: seconds / 60, seconds: String(seconds % 60))
    }
}
