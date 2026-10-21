// The Mac's player, in the main window (M4): the MacPlayer board, its keys and its pointer. The
// video fills the window's content where the sidebar, the toolbar and the list were, and Back
// returns to the title page.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import AppKit
import SwiftUI

/// The player in the main window. The controls fade in on pointer movement or a key, and out 2 s
/// after (M5), never while the pointer is over the bar, while scrubbing or while a menu is open.
/// It grows out of the banner the play started from, and shrinks back into it on Back.
public struct MacPlayerView: View {
    let model: PlayerModel
    @State private var panel: Panel?
    @State private var overBar = false
    @State private var window: NSWindow?
    @State private var height: CGFloat = 0
    /// How the main window looked before the player took it over, to give it back.
    @State private var saved: SavedWindow?
    /// Full screen was the player's doing, and ends with it.
    @State private var enteredFullScreen = false
    /// Grown to the window from the banner; false while it is the banner's size.
    @State private var grown = false
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.playerZoom) private var zoom
    private let metrics = PlayerMetrics.mac

    enum Panel: Equatable { case tracks, episodes, more }

    public init(model: PlayerModel) {
        self.model = model
    }

    private var visible: Bool { (model.controlsVisible || panel != nil || overBar) && !model.stillWatching && model.failure == nil && !curtain }

    /// The banner holds until the first frame, and comes back between episodes.
    private var curtain: Bool { model.starting != nil || !model.hasPicture || model.holdsCurtain }

    public var body: some View {
        ZStack {
            Color.black
            PlayerPicture(model: model)
            StartingCurtain(model: model, metrics: metrics)
                .opacity(curtain ? 1 : 0)
                .animation(curtain ? .easeIn(duration: 0.25) : .fadeIn, value: curtain)
                .allowsHitTesting(curtain)
            if model.showsSpinner {
                ProgressView().controlSize(.regular).tint(.silk).environment(\.colorScheme, .dark)
            }
            chrome
                .opacity(visible ? 1 : 0)
                .animation(visible ? .fadeIn : .fadeOut, value: visible)
                .allowsHitTesting(visible)
            cards
            if model.stillWatching {
                Palette.controlsDim
                StillWatchingDialog(model: model, metrics: metrics)
            }
        }
        .foregroundStyle(.silk)
        .font(.system(size: 13))
        .clipShape(.rect(cornerRadius: grown ? 0 : ArtShape.bannerRadius))
        .modifier(GrowFromBanner(id: model.staged?.id, zoom: reduceMotion ? nil : zoom, grown: grown))
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
        .playerAccessibility(model)
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onKeyPress(phases: [.down, .repeat]) { press in key(press) }
        .onAppear { focused = true }
        .onChange(of: model.fullScreen) { _, on in fullScreen(on) }
        .onChange(of: visible, initial: true) { _, on in titleBar(on) }
        .onChange(of: window) { _, window in if let window { opened(window) } }
        .onChange(of: model.backRequests) { back() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didEnterFullScreenNotification)) { note in
            guard let window, (note.object as? NSWindow) === window else { return }
            PlayerModel.note(.info, "player window full screen, \(Self.describe(window))")
            if !model.fullScreen { model.fullScreen = true }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didExitFullScreenNotification)) { note in
            guard let window, (note.object as? NSWindow) === window else { return }
            PlayerModel.note(.info, "player window out of full screen, \(Self.describe(window))")
            if model.fullScreen { model.fullScreen = false }
        }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { height = $0 }
        .onChange(of: SubtitleLift(visible: visible, height: height), initial: true) { _, lift in
            // The subtitles rise above the control row while it is up, as MacPlayer has them, and
            // above the mini player's.
            model.liftSubtitles(lift.visible ? metrics.subtitleLift / max(lift.height, 1) : 0, animated: !reduceMotion)
        }
        .onAppear {
            // From the banner's frame to the window's, on the standard spring.
            withAnimation(.standard) { grown = true }
        }
        .onDisappear {
            // Back to browsing: the player stopped, or moved into the mini player. The main window
            // is given back as it was.
            if let window { restore(window) }
        }
        .preferredColorScheme(.dark)
    }

    /// The cards over the video.
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

    private func scrim(stops: [Gradient.Stop]) -> some View {
        Scrim(stops: stops)
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
        Button("Picture in Picture") { model.togglePictureInPicture() }
            .disabled(!model.pictureInPictureAvailable)
    }

    // MARK: Chrome

    private var chrome: some View {
        ZStack {
            VStack(spacing: 0) {
                scrim(stops: [
                    .init(color: Palette.ground.opacity(0.6), location: 0),
                    .init(color: Palette.ground.opacity(0), location: 1),
                ])
                .frame(height: 112)
                Spacer(minLength: 0)
                scrim(stops: [
                    .init(color: Palette.ground.opacity(0), location: 0),
                    .init(color: Palette.ground.opacity(0.35), location: 0.45),
                    .init(color: Palette.ground.opacity(0.72), location: 1),
                ])
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
        HStack(spacing: 12) {
            // The window's own buttons sit here, outside full screen.
            Color.clear.frame(width: model.fullScreen ? 0 : 60, height: 12)
            icon("chevron.backward", "Back") { back() }
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
            .background(.overVideo(0.62), in: .capsule)
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
            icon("pip.enter", "Picture in Picture") { model.togglePictureInPicture() }
                .disabled(!model.pictureInPictureAvailable)
                .opacity(model.pictureInPictureAvailable ? 1 : 0.4)
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
        .voiceControlLabels(label)
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
        .voiceControlLabels(label)
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
        .background(.glassFill, in: .rect(cornerRadius: 12))
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
            } else if model.corner == .upNext {
                model.cancelNext()
            } else if model.corner == .stall {
                model.keepWaiting()
            } else if model.fullScreen {
                model.fullScreen = false
            } else {
                back()
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
            case "p" where command && option: model.togglePictureInPicture()
            case "m": model.toggleMute()
            case "n": model.step(shift ? -1 : 1)
            case "f" where !command: model.fullScreen.toggle()
            case "e": toggle(.episodes)
            case "[" where command: back()
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

    /// The player has the main window: its title bar clear over the video, dark, moved by its
    /// background, and full screen when the player was asked to open that way.
    private func opened(_ window: NSWindow) {
        if saved == nil { saved = SavedWindow(window) }
        // The toolbar gives way to the video; the window's own buttons stay, over it, and fade
        // with the controls.
        window.toolbar?.isVisible = false
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.styleMask.insert(.fullSizeContentView)
        window.isMovableByWindowBackground = true
        window.backgroundColor = .black
        // The player is dark whatever the system's appearance: AppKit's own controls in it, the
        // spinner among them, draw for a dark window.
        window.appearance = NSAppearance(named: .darkAqua)
        PlayerModel.note(.info, "player in the main window, \(Self.describe(window)), asked for full screen \(model.fullScreen)")
        if model.fullScreen, !window.styleMask.contains(.fullScreen) {
            enteredFullScreen = true
            // Once the window is on screen: a window still ordering in will not go full screen.
            DispatchQueue.main.async { [weak window] in
                guard let window, model.fullScreen, !window.styleMask.contains(.fullScreen) else { return }
                window.toggleFullScreen(nil)
            }
        }
    }

    /// The main window as it was before the player, and out of the full screen the player put it
    /// in.
    private func restore(_ window: NSWindow) {
        saved?.restore(window)
        saved = nil
        for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(kind)?.alphaValue = 1
        }
        if enteredFullScreen, window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil) }
        enteredFullScreen = false
        PlayerModel.note(.info, "main window back to browsing, \(Self.describe(window))")
    }

    /// Back to the title page: the video shrinks into the banner it came from, and the playback
    /// stops with its place saved.
    private func back() {
        PlayerModel.note(.info, "back from the player")
        guard zoom != nil, !reduceMotion else {
            model.viewerClosed()
            return
        }
        withAnimation(.standard) { grown = false }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { model.viewerClosed() }
    }

    /// "1058×595 pt at (227, 175), screen 1512×945 pt at 2.0×, full screen no", for the log.
    static func describe(_ window: NSWindow) -> String {
        let frame = window.frame
        let screen = window.screen.map {
            "screen \(Int($0.visibleFrame.width))×\(Int($0.visibleFrame.height)) pt at \($0.backingScaleFactor)×"
        } ?? "no screen"
        let content = window.contentView.map { "content \(Int($0.bounds.width))×\(Int($0.bounds.height))" } ?? "no content"
        return "\(Int(frame.width))×\(Int(frame.height)) pt at (\(Int(frame.minX)), \(Int(frame.minY))), \(content), \(screen), full screen \(window.styleMask.contains(.fullScreen) ? "yes" : "no")"
    }

    private func fullScreen(_ on: Bool) {
        guard let window, window.styleMask.contains(.fullScreen) != on else { return }
        if on { enteredFullScreen = true }
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

/// What the player changes about the main window, as it was, to put back.
struct SavedWindow {
    let transparentTitleBar: Bool
    let titleVisibility: NSWindow.TitleVisibility
    let fullSizeContent: Bool
    let movableByBackground: Bool
    let backgroundColor: NSColor
    let appearance: NSAppearance?
    let toolbarVisible: Bool?

    @MainActor
    init(_ window: NSWindow) {
        toolbarVisible = window.toolbar?.isVisible
        transparentTitleBar = window.titlebarAppearsTransparent
        titleVisibility = window.titleVisibility
        fullSizeContent = window.styleMask.contains(.fullSizeContentView)
        movableByBackground = window.isMovableByWindowBackground
        backgroundColor = window.backgroundColor
        appearance = window.appearance
    }

    @MainActor
    func restore(_ window: NSWindow) {
        window.titlebarAppearsTransparent = transparentTitleBar
        window.titleVisibility = titleVisibility
        if fullSizeContent { window.styleMask.insert(.fullSizeContentView) } else { window.styleMask.remove(.fullSizeContentView) }
        window.isMovableByWindowBackground = movableByBackground
        window.backgroundColor = backgroundColor
        window.appearance = appearance
        if let toolbarVisible { window.toolbar?.isVisible = toolbarVisible }
    }
}

/// The player at the banner's frame until it has grown: matched to the banner the play started
/// from while `grown` is false, its own frame once it is. Nothing without a namespace, as under
/// Reduce Motion, where it crossfades instead.
struct GrowFromBanner: ViewModifier {
    let id: AnilistId?
    let zoom: Namespace.ID?
    let grown: Bool

    func body(content: Content) -> some View {
        if let zoom, let id {
            content.matchedGeometryEffect(
                id: grown ? AnyHashable("player") : AnyHashable(id), in: zoom, properties: .frame, isSource: false)
        } else {
            content
        }
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
