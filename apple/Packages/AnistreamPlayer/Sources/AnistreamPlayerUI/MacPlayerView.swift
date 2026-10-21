// The Mac's player, in a window of its own (M4): the MacPlayer board, its keys and its pointer.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import AppKit
import SwiftUI

/// The player window's content. The controls fade in on pointer movement or a key, and out 2 s
/// after (M5), never while the pointer is over the bar, while scrubbing or while a menu is open.
public struct MacPlayerView: View {
    let model: PlayerModel
    @State private var panel: Panel?
    @State private var overBar = false
    @State private var window: NSWindow?
    @State private var height: CGFloat = 0
    /// The window's frame before the mini player, to go back to exactly.
    @State private var fullFrame: NSRect?
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let metrics = PlayerMetrics.mac

    enum Panel: Equatable { case tracks, episodes, more }

    public init(model: PlayerModel) {
        self.model = model
    }

    private var visible: Bool { (model.controlsVisible || panel != nil || overBar) && !model.stillWatching }

    /// The banner holds until the first frame, and comes back between episodes.
    private var curtain: Bool { model.starting != nil || !model.hasPicture || model.holdsCurtain }

    public var body: some View {
        ZStack {
            Color.black
            PlayerPicture(model: model)
            StartingCurtain(model: model, metrics: metrics)
                .opacity(curtain ? 1 : 0)
                .animation(curtain ? .easeIn(duration: 0.25) : .fadeIn, value: curtain)
                .allowsHitTesting(false)
            if model.showsSpinner {
                ProgressView().controlSize(.regular).tint(.silk).environment(\.colorScheme, .dark)
            }
            Group {
                if model.miniPlayer { miniChrome } else { chrome }
            }
            .opacity(visible ? 1 : 0)
            .animation(visible ? .fadeIn : .fadeOut, value: visible)
            .allowsHitTesting(visible)
            if !model.miniPlayer { cards }
            if model.stillWatching {
                Palette.controlsDim
                StillWatchingDialog(model: model, metrics: metrics)
            }
        }
        .foregroundStyle(.silk)
        .font(.system(size: 13))
        .ignoresSafeArea()
        .background(WindowReader { window = $0 })
        .background(ScrollMonitor(model: model))
        .contentShape(.rect)
        .onContinuousHover { phase in
            if case .active = phase { model.poke() }
        }
        .pointerVisibility(visible ? .automatic : .hidden)
        .gesture(
            TapGesture(count: 2).onEnded { model.fullScreen.toggle() }
                .exclusively(before: TapGesture().onEnded { model.togglePause() })
        )
        .contextMenu { contextMenu }
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onKeyPress(phases: [.down, .repeat]) { press in key(press) }
        .onAppear { focused = true }
        .onChange(of: model.fullScreen) { _, on in fullScreen(on) }
        .onChange(of: visible) { _, on in titleBar(on) }
        .onChange(of: model.miniPlayer) { _, on in mini(on) }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didExitFullScreenNotification)) { note in
            // Asked for the mini player in full screen: it follows once full screen has ended.
            if model.miniPlayer, fullFrame == nil, (note.object as? NSWindow) === window { mini(true) }
        }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { height = $0 }
        .onChange(of: SubtitleLift(visible: visible, height: height), initial: true) { _, lift in
            // The subtitles rise above the control row while it is up, as MacPlayer has them, and
            // above the mini player's.
            model.liftSubtitles(lift.visible ? metrics.subtitleLift / max(lift.height, 1) : 0, animated: !reduceMotion)
        }
        .onDisappear {
            // The window closed: ⌘W, its close button, or the model closing it. Closed small, it
            // opens next time as it was before the mini player.
            if let fullFrame, let window {
                window.level = .normal
                window.setFrame(fullFrame, display: false)
                self.fullFrame = nil
            }
            model.viewerClosed()
        }
        .preferredColorScheme(.dark)
    }

    /// The cards over the video, which the mini player leaves out.
    @ViewBuilder
    private var cards: some View {
        CornerCard(model: model, metrics: metrics)
            .padding(metrics.cornerInset)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        ResumeNoteView(model: model, metrics: metrics)
            .padding(.top, 60)
            .padding(.leading, 20)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .animation(.standard, value: model.resume)
        PlayerAlertView(model: model, metrics: metrics)
            .padding(.top, 60)
            .frame(maxHeight: .infinity, alignment: .top)
            .animation(.standard, value: model.alert)
        if let panel {
            panelLayer(panel)
        }
    }

    // MARK: The mini player

    /// The mini player's controls: play and pause, the seek line and the volume, and the way back.
    private var miniChrome: some View {
        ZStack {
            LinearGradient(
                stops: [
                    .init(color: Palette.ground.opacity(0), location: 0.35),
                    .init(color: Palette.ground.opacity(0.72), location: 1),
                ],
                startPoint: .top, endPoint: .bottom
            )
            .allowsHitTesting(false)
            VStack(spacing: 0) {
                HStack {
                    Spacer()
                    icon("arrow.up.left.and.arrow.down.right", "Exit Mini Player") { model.miniPlayer = false }
                }
                .padding(.top, 6)
                .padding(.trailing, 6)
                Spacer(minLength: 0)
                HStack(spacing: 4) {
                    icon(model.paused ? "play.fill" : "pause.fill", model.paused ? "Play" : "Pause") { model.togglePause() }
                    SeekLine(model: model, metrics: metrics, showsPreview: false)
                        .padding(.horizontal, 4)
                    Button { model.toggleMute() } label: {
                        Image(systemName: model.muted ? "speaker.slash.fill" : "speaker.wave.2.fill")
                            .font(.system(size: 12))
                            .frame(width: 18, height: 28)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(model.muted ? "Unmute" : "Mute")
                    VolumeLine(model: model, width: 56)
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 6)
                .onHover { overBar = $0 }
            }
            .overVideoText()
        }
    }

    /// The window shrinks to a quarter of the screen's width in the bottom right corner and
    /// floats over other windows; leaving puts it back exactly where it was.
    private func mini(_ on: Bool) {
        guard let window else { return }
        if on {
            guard fullFrame == nil else { return }
            if window.styleMask.contains(.fullScreen) {
                model.fullScreen = false
                return
            }
            guard let screen = window.screen ?? NSScreen.main else { return }
            fullFrame = window.frame
            panel = nil
            let visible = screen.visibleFrame
            let width = (visible.width / 4).rounded()
            let size = NSSize(width: width, height: (width * 9 / 16).rounded())
            window.contentMinSize = NSSize(width: 240, height: 135)
            window.level = .floating
            window.setFrame(
                NSRect(x: visible.maxX - size.width - 16, y: visible.minY + 16, width: size.width, height: size.height),
                display: true, animate: !reduceMotion)
        } else {
            window.level = .normal
            if let fullFrame { window.setFrame(fullFrame, display: true, animate: !reduceMotion) }
            fullFrame = nil
        }
    }

    // MARK: The context menu

    @ViewBuilder
    private var contextMenu: some View {
        Button(model.paused ? "Play" : "Pause") { model.togglePause() }
        if let skip = model.skip {
            Button(skip.label.capitalized) { model.takeSkip() }
        }
        Divider()
        Button("Subtitles…") { toggle(.tracks) }
        Button("Audio…") { toggle(.tracks) }
        Button("Episodes…") { toggle(.episodes) }
        Divider()
        Button(model.miniPlayer ? "Exit Mini Player" : "Mini Player") { model.miniPlayer.toggle() }
        Button("Picture in Picture") {}
            .disabled(true)
    }

    // MARK: Chrome

    private var chrome: some View {
        ZStack {
            VStack(spacing: 0) {
                LinearGradient(
                    colors: [Palette.ground.opacity(0.6), Palette.ground.opacity(0)],
                    startPoint: .top, endPoint: .bottom
                )
                .frame(height: 112)
                Spacer(minLength: 0)
                LinearGradient(
                    stops: [
                        .init(color: Palette.ground.opacity(0), location: 0),
                        .init(color: Palette.ground.opacity(0.35), location: 0.45),
                        .init(color: Palette.ground.opacity(0.72), location: 1),
                    ],
                    startPoint: .top, endPoint: .bottom
                )
                .frame(height: 240)
            }
            .allowsHitTesting(false)
            VStack(spacing: 0) {
                header
                Spacer(minLength: 0)
                controlRow
                    .onHover { overBar = $0 }
            }
        }
    }

    private var header: some View {
        HStack(spacing: 20) {
            // The window's own buttons sit here.
            Color.clear.frame(width: 52, height: 12)
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(model.showTitle)
                    .font(.showTitle(size: 15))
                Text(model.episodeLine)
                    .contentTransition(.opacity)
                    .animation(.standard, value: model.episodeLine)
            }
            .lineLimit(1)
            .overVideoText()
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 0) {
                capsuleButton("captions.bubble", "Subtitles and audio", on: panel == .tracks) { toggle(.tracks) }
                capsuleButton("list.bullet", "Episodes", on: panel == .episodes) { toggle(.episodes) }
            }
            .padding(2)
            .background(Palette.ground.opacity(0.62), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
        }
        .padding(.leading, 20)
        .padding(.trailing, 16)
        .frame(height: 52)
    }

    private var controlRow: some View {
        HStack(spacing: 4) {
            icon(model.paused ? "play.fill" : "pause.fill", model.paused ? "Play" : "Pause") { model.togglePause() }
            icon("gobackward.10", "Back 10 seconds") { model.seek(by: -10) }
            icon("goforward.10", "Forward 10 seconds") { model.seek(by: 10) }
            PlayerTime(seconds: model.position, metrics: metrics)
                .frame(width: 44, alignment: .leading)
                .padding(.leading, 8)
            SeekLine(model: model, metrics: metrics)
                .padding(.horizontal, 8)
            PlayerTime(seconds: max(0, (model.duration ?? 0) - model.position), remaining: true, metrics: metrics)
                .frame(width: 48, alignment: .trailing)
            HStack(spacing: 6) {
                Button { model.toggleMute() } label: {
                    Image(systemName: model.muted ? "speaker.slash.fill" : "speaker.wave.2.fill")
                        .font(.system(size: 13))
                        .frame(width: 20, height: 28)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.muted ? "Unmute" : "Mute")
                VolumeLine(model: model)
            }
            .padding(.leading, 12)
            Button { toggle(.more) } label: {
                HStack(spacing: 5) {
                    Text(speedLabel(model.speed)).monospacedDigit()
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
                }
                .padding(.horizontal, 8)
                .frame(height: 28)
                .background(Palette.silk.opacity(panel == .more ? 0.16 : 0), in: .capsule)
                .contentShape(.capsule)
            }
            .buttonStyle(.plain)
            .padding(.leading, 8)
            .accessibilityLabel("Playback speed, \(speedLabel(model.speed))")
            // Picture in Picture needs mpv's frames handed to an AVSampleBufferDisplayLayer,
            // which the player does not do yet.
            icon("pip.enter", "Picture in Picture") {}
                .disabled(true)
                .opacity(0.4)
            icon(
                model.fullScreen ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                model.fullScreen ? "Exit Full Screen" : "Enter Full Screen"
            ) { model.fullScreen.toggle() }
        }
        .overVideoText()
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
        .frame(height: 36 + 16)
    }

    private func icon(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15, weight: .medium))
                .frame(width: 32, height: 32)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }

    private func capsuleButton(_ symbol: String, _ label: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15))
                .frame(width: 32, height: 32)
                .background(Palette.silk.opacity(on ? 0.16 : 0), in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }

    // MARK: Panels

    private func toggle(_ which: Panel) {
        panel = panel == which ? nil : which
        model.poke()
    }

    @ViewBuilder
    private func panelLayer(_ which: Panel) -> some View {
        Color.clear
            .contentShape(.rect)
            .onTapGesture { panel = nil }
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    switch which {
                    case .tracks: TrackLists(model: model, metrics: metrics)
                    case .episodes: EpisodeList(model: model, metrics: metrics)
                    case .more: MoreLists(model: model, metrics: metrics)
                    }
                }
                .padding(5)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
        .frame(width: 264)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxHeight: 420)
        .background(Palette.glass, in: .rect(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Palette.glassEdge, lineWidth: 1))
        .shadow(color: Color(rgb: 0x0A0C18, opacity: 0.4), radius: 12, y: 8)
        .padding(which == .more ? EdgeInsets(top: 0, leading: 0, bottom: 60, trailing: 84) : EdgeInsets(top: 50, leading: 0, bottom: 0, trailing: 16))
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: which == .more ? .bottomTrailing : .topTrailing)
        .transition(.opacity)
    }

    // MARK: Keys

    /// The companion's keys (ux.md, Player window). Arrow keys and S act without showing the
    /// controls; everything else shows them.
    private func key(_ press: KeyPress) -> KeyPress.Result {
        let shift = press.modifiers.contains(.shift)
        let command = press.modifiers.contains(.command)
        let option = press.modifiers.contains(.option)
        switch press.key {
        case .space:
            model.togglePause()
        case .leftArrow:
            model.seek(by: shift ? -30 : -5)
            return .handled
        case .rightArrow:
            model.seek(by: shift ? 30 : 5)
            return .handled
        case .upArrow:
            model.stepVolume(by: 5)
        case .downArrow:
            model.stepVolume(by: -5)
        case .return:
            switch model.corner {
            case .stall: model.switchSource()
            case .upNext: model.playNext()
            case .skip: model.takeSkip()
            default: return .ignored
            }
        case .escape:
            if panel != nil {
                panel = nil
            } else if model.miniPlayer {
                model.miniPlayer = false
            } else if model.corner == .upNext {
                model.cancelNext()
            } else if model.corner == .stall {
                model.keepWaiting()
            } else if model.fullScreen {
                model.fullScreen = false
            } else {
                return .ignored
            }
        case .delete:
            model.setSpeed(1)
        default:
            switch press.characters.lowercased() {
            case "s" where option: toggle(.tracks)
            case "a" where option: toggle(.tracks)
            case "s":
                model.takeSkip()
                return .handled
            case "z" where command, "u":
                model.undoSkip()
            case "m" where command && option: model.miniPlayer.toggle()
            case "m": model.toggleMute()
            case "n": model.step(shift ? -1 : 1)
            case "f" where !command: model.fullScreen.toggle()
            case "e": toggle(.episodes)
            case "]": model.setSpeed(min(4, model.speed * 1.1))
            case "[": model.setSpeed(max(0.25, model.speed / 1.1))
            case ".":
                guard command else { return .ignored }
                model.viewerClosed()
            default:
                return .ignored
            }
        }
        model.poke()
        return .handled
    }

    // MARK: The window

    private func fullScreen(_ on: Bool) {
        guard let window, window.styleMask.contains(.fullScreen) != on else { return }
        window.toggleFullScreen(nil)
    }

    /// The title bar is transparent and hides with the controls.
    private func titleBar(_ visible: Bool) {
        guard let window else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = visible ? 0.2 : 0.3
            for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
                window.standardWindowButton(kind)?.animator().alphaValue = visible ? 1 : 0
            }
        }
    }
}

/// The volume as a 72 pt line, silk over silk at 22%; dragging steps it.
struct VolumeLine: View {
    let model: PlayerModel
    var width: CGFloat = 72

    var body: some View {
        GeometryReader { proxy in
            Capsule()
                .fill(Palette.silk.opacity(0.22))
                .overlay(alignment: .leading) {
                    Capsule().fill(.silk).frame(width: proxy.size.width * min(1, model.volume / 100))
                }
                .frame(height: 4)
                .frame(maxHeight: .infinity)
                .contentShape(.rect)
                .gesture(
                    DragGesture(minimumDistance: 0).onEnded { value in
                        let target = min(100, max(0, value.location.x / proxy.size.width * 100))
                        model.stepVolume(by: (target - model.volume).rounded())
                    }
                )
        }
        .frame(width: width, height: 28)
        .accessibilityElement()
        .accessibilityLabel("Volume")
        .accessibilityValue("\(Int(model.volume))%")
        .accessibilityAdjustableAction { model.stepVolume(by: $0 == .increment ? 5 : -5) }
    }
}

/// The NSWindow a view is in.
struct WindowReader: NSViewRepresentable {
    let found: (NSWindow?) -> Void

    func makeNSView(context: Context) -> NSView { Reader(found: found) }
    func updateNSView(_ view: NSView, context: Context) {}

    final class Reader: NSView {
        let found: (NSWindow?) -> Void
        init(found: @escaping (NSWindow?) -> Void) {
            self.found = found
            super.init(frame: .zero)
        }
        required init?(coder: NSCoder) { fatalError("not used") }
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let window {
                window.isMovableByWindowBackground = true
                window.titlebarAppearsTransparent = true
                window.backgroundColor = .black
                // The player is dark whatever the system's appearance: AppKit's own controls in
                // it, the spinner among them, draw for a dark window.
                window.appearance = NSAppearance(named: .darkAqua)
            }
            found(window)
        }
    }
}

/// The scroll wheel over the player: vertical scrolls the volume 2% a notch, horizontal two-finger
/// scrolls seek 0.25 s a point, momentum ignored.
struct ScrollMonitor: NSViewRepresentable {
    let model: PlayerModel

    func makeNSView(context: Context) -> NSView {
        let view = Monitor()
        view.model = model
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {}

    final class Monitor: NSView {
        weak var model: PlayerModel?
        private var monitor: Any?
        private var horizontal: CGFloat = 0

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            guard window != nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                guard let self, event.window === self.window else { return event }
                MainActor.assumeIsolated { self.scrolled(event) }
                return event
            }
        }

        private func scrolled(_ event: NSEvent) {
            guard let model, event.momentumPhase.isEmpty else { return }
            model.poke()
            if abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY) {
                horizontal += event.scrollingDeltaX
                if event.phase == .ended || event.phase == .cancelled || abs(horizontal) >= 8 {
                    model.seek(by: Double(-horizontal) * 0.25)
                    horizontal = 0
                }
            } else if event.scrollingDeltaY != 0 {
                let notches = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 10 : event.scrollingDeltaY
                let step = (Double(notches) * 2).rounded()
                if step != 0 { model.stepVolume(by: step) }
            }
        }

        isolated deinit {
            if let monitor { NSEvent.removeMonitor(monitor) }
        }
    }
}
#endif
