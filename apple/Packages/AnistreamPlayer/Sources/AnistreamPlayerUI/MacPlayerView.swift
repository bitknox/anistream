// The Mac's player, in the main window (M4): the MacPlayer board, its keys and its pointer. The
// video fills the window's content where the sidebar, the toolbar and the list were, and Back
// returns to the title page.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import AnistreamStrings
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
    /// The top bar's control under the pointer, as the title bar strip says it: where the
    /// controls' own hover cannot see it.
    @State private var pointed: String?
    /// The watch party's popover on the people button.
    @State private var partyOpen = false
    @FocusState private var focused: Bool
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.playerZoom) private var zoom
    private let metrics = PlayerMetrics.mac

    typealias Panel = PlayerMenu

    public init(model: PlayerModel) {
        self.model = model
    }

    private var visible: Bool { (model.controlsVisible || panel != nil || partyOpen || overBar) && !model.stillWatching && model.failure == nil && !curtain }

    /// Whether the pointer may hide over the video (PlayerPointer).
    private var pointerIdle: Bool {
        PlayerPointer.idle(
            controlsShown: model.controlsVisible || panel != nil || partyOpen || overBar,
            loading: curtain, failed: model.failure != nil, askingStillWatching: model.stillWatching)
    }

    /// The banner holds until the first frame, and comes back between episodes.
    private var curtain: Bool { model.starting != nil || !model.hasPicture || model.holdsCurtain }

    public var body: some View {
        ZStack {
            Color.black
            PlayerPicture(model: model)
            // Clicks on the video, beneath the controls: on the player's root they came before
            // every button's own, and a click on Full Screen paused instead.
            Color.clear
                .contentShape(.rect)
                .gesture(
                    TapGesture(count: 2).onEnded { model.fullScreen.toggle() }
                        .exclusively(before: TapGesture().onEnded { model.togglePause() })
                )
            StartingCurtain(model: model, metrics: metrics)
                .opacity(curtain ? 1 : 0)
                .animation(curtain ? .easeIn(duration: 0.25) : .fadeIn, value: curtain)
                .allowsHitTesting(curtain)
            BufferingLine(model: model, reduceMotion: reduceMotion)
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
        // Hidden with the controls, and shown again on any way out (PlayerPointer).
        .modifier(IdlePointer(idle: pointerIdle, window: window))
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
        .onChange(of: model.menuRequests) { panel = model.requestedMenu }
        .onChange(of: store.party.panelRequests) {
            panel = nil
            partyOpen = store.party.inParty
        }
        // The party is over: its popover goes with it.
        .onChange(of: store.party.inParty) { _, inParty in if !inParty { partyOpen = false } }
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
            // The subtitles rise above the control row while it is up, as MacPlayer has them.
            model.liftSubtitles(lift.visible ? metrics.subtitleLift / max(lift.height, 1) : 0, animated: !reduceMotion)
        }
        .onAppear {
            // From the banner's frame to the window's, on the standard spring.
            withAnimation(.standard) { grown = true }
        }
        .onDisappear {
            // Back to browsing: the player stopped, or moved into Picture in Picture. The main
            // window is given back as it was.
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
        // Stats for nerds stays up while the controls hide, below where the top bar's capsule
        // is, under the menus that hang from it.
        StatsPanel(model: model, metrics: metrics)
            .padding(.top, 60)
            .padding(.trailing, 16)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
            .animation(model.statsPanel != nil ? .fadeIn : .fadeOut, value: model.statsPanel != nil)
        // A panel is part of the chrome: it fades in and out as the controls do (T4).
        ZStack {
            if let panel {
                panelLayer(panel)
            }
        }
        .animation(panel != nil ? .fadeIn : .fadeOut, value: panel)
    }

    private func scrim(stops: [Gradient.Stop]) -> some View {
        Scrim(stops: stops)
    }

    // MARK: The context menu

    @ViewBuilder
    private var contextMenu: some View {
        Button(model.paused ? L.actionPlay : L.actionPause) { model.togglePause() }
        if let skip = model.skip {
            Button(skip.label) { model.takeSkip() }
        }
        Divider()
        Button(L.playerMenuSubtitles) { toggle(.tracks) }
        Button(L.playerMenuAudio) { toggle(.tracks) }
        Button(L.playerMenuEpisodes) { toggle(.episodes) }
        Button(L.playerMenuPictureSound) { toggle(.picture) }
            .disabled(model.menus == nil)
        Toggle(L.playerStats, isOn: Binding(get: { model.statsShown }, set: { model.showStats($0) }))
        Divider()
        Button(L.playerPictureInPicture) { model.togglePictureInPicture() }
            .disabled(!model.pictureInPictureAvailable)
        if model.playOnTVAvailable {
            Button(L.tvPlayOnMenu) { model.playOnTV() }
        }
        Button(L.partyOpen) { people() }
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
            // In the window's title bar strip outside full screen, where each of the bar's
            // controls takes its clicks this way and the rest of the strip drags the window.
            icon("chevron.backward", L.actionBack, lit: pointed == L.actionBack) { back() }
                .titleBarControl(active: visible, pointed: { point(L.actionBack, $0) }) { back() }
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
                partyButton
                capsuleButton("captions.bubble", L.playerSubtitlesAndAudio, on: panel == .tracks) { toggle(.tracks) }
                capsuleButton("list.bullet", L.playerEpisodes, on: panel == .episodes) { toggle(.episodes) }
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
            icon(model.paused ? "play.fill" : "pause.fill", model.paused ? L.actionPlay : L.actionPause) { model.togglePause() }
            icon("gobackward.10", L.playerBack10) { model.seek(by: -10) }
            icon("goforward.10", L.playerForward10) { model.seek(by: 10) }
            PlayerTime(seconds: model.position, metrics: metrics)
                .frame(width: 44, alignment: .leading)
                .padding(.leading, 8)
                .accessibilityIdentifier("Elapsed")
            SeekLine(model: model, metrics: metrics)
                .padding(.horizontal, 8)
            PlayerTime(seconds: max(0, (model.duration ?? 0) - model.position), remaining: true, metrics: metrics)
                .frame(width: 48, alignment: .trailing)
            // The speaker on a 28 pt circle that lights, and the line where the board has them:
            // the speaker's centre 22 pt in, the line 38 pt in.
            HStack(spacing: 2) {
                Button { model.toggleMute() } label: {
                    Image(systemName: model.muted ? "speaker.slash.fill" : "speaker.wave.2.fill")
                        .font(.system(size: 13))
                        .frame(width: 28, height: 28)
                        .contentShape(.circle)
                }
                .buttonStyle(OverVideoButtonStyle())
                .accessibilityLabel(model.muted ? L.playerUnmute : L.playerMute)
                VolumeLine(model: model)
            }
            .padding(.leading, 8)
            Button { toggle(.speed) } label: {
                HStack(spacing: 5) {
                    Text(speedLabel(model.speed)).monospacedDigit()
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
                }
                .padding(.horizontal, 8)
                .frame(height: 28)
                .contentShape(.capsule)
            }
            .buttonStyle(OverVideoButtonStyle(on: panel == .speed))
            .padding(.leading, 8)
            .accessibilityLabel(L.playerSpeedIs(speed: speedLabel(model.speed)))
            .help(L.playerSpeed)
            icon("slider.horizontal.3", L.playerPictureAndSound, on: panel == .picture) { toggle(.picture) }
                .disabled(model.menus == nil)
                .opacity(model.menus == nil ? 0.4 : 1)
            if model.playOnTVAvailable {
                icon("tv", L.tvPlayOn) { model.playOnTV() }
            }
            icon("pip.enter", L.playerPictureInPicture) { model.togglePictureInPicture() }
                .disabled(!model.pictureInPictureAvailable)
                .opacity(model.pictureInPictureAvailable ? 1 : 0.4)
            icon(
                model.fullScreen ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                model.fullScreen ? L.playerExitFullScreen : L.playerEnterFullScreen
            ) { model.fullScreen.toggle() }
        }
        .overVideoText()
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
        .frame(height: 36 + 16)
    }

    private func icon(
        _ symbol: String, _ label: String, on: Bool = false, lit: Bool = false, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15, weight: .medium))
                .frame(width: 32, height: 32)
                .contentShape(.circle)
        }
        .buttonStyle(OverVideoButtonStyle(on: on, lit: lit))
        .accessibilityLabel(label)
        .voiceControlLabels(label)
        .help(label)
    }

    private func capsuleButton(_ symbol: String, _ label: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15))
                .frame(width: 32, height: 32)
                .contentShape(.circle)
        }
        .buttonStyle(OverVideoButtonStyle(on: on, lit: pointed == label))
        .accessibilityLabel(label)
        .voiceControlLabels(label)
        .help(label)
        .titleBarControl(active: visible, pointed: { point(label, $0) }, action: action)
    }

    /// The pointer came onto the top bar's `label`, or left it.
    private func point(_ label: String, _ over: Bool) {
        if over {
            pointed = label
        } else if pointed == label {
            pointed = nil
        }
    }

    // MARK: Panels

    private func toggle(_ which: Panel) {
        panel = panel == which ? nil : which
        partyOpen = false
        model.poke()
    }

    /// The people button: the party's popover while in one, the form that starts one otherwise
    /// (a sheet over the player, as from a title).
    private var partyButton: some View {
        let inParty = store.party.inParty
        return capsuleButton(PartyButton.symbol(inParty: inParty), PartyButton.label(inParty: inParty), on: partyOpen) { people() }
            .popover(isPresented: $partyOpen, arrowEdge: .bottom) {
                PartyPanel(metrics: metrics)
                    .environment(store)
                    .preferredColorScheme(.dark)
            }
    }

    private func people() {
        panel = nil
        if store.party.inParty {
            partyOpen.toggle()
        } else {
            store.party.open(.playing)
        }
        model.poke()
    }

    @ViewBuilder
    private func panelLayer(_ which: Panel) -> some View {
        Color.clear
            .contentShape(.rect)
            .onTapGesture { panel = nil }
        MenuPanelFrame {
            ScrollViewReader { list in
                MenuScroll {
                    VStack(alignment: .leading, spacing: 0) {
                        switch which {
                        case .tracks: TrackLists(model: model, metrics: metrics)
                        case .episodes: EpisodeList(model: model, metrics: metrics)
                        case .speed: SpeedList(model: model, metrics: metrics)
                        case .picture: PictureLists(model: model, metrics: metrics)
                        }
                    }
                    .padding(5)
                    .environment(\.pointedMenuRow, model.pointedRow)
                }
                .onAppear {
                    if let row = selectedRow(which) { list.scrollTo(row, anchor: .center) }
                }
            }
        }
        .frame(width: Self.panelWidth(which))
        .background(.glassFill, in: .rect(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Palette.glassEdge, lineWidth: 1))
        .shadow(color: Color(rgb: 0x0A0C18, opacity: 0.4), radius: 12, y: 8)
        // Below the title strip and above the control row, whichever end it hangs from: the top
        // bar's menus under the capsule, the control row's over their buttons.
        .padding(EdgeInsets(top: 50, leading: 0, bottom: 60, trailing: Self.panelTrailing(which, playOnTV: model.playOnTVAvailable)))
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: Self.panelHangs(which))
        // One panel for each menu, so going from one to another crossfades rather than jumping
        // the same panel across.
        .id(which)
        .transition(.opacity)
    }

    /// How far in from the trailing edge a panel ends: the control row's in line with the end of
    /// the button that opened it, past Full Screen, Picture in Picture, Play on TV when the row
    /// has it and, for the speed, Picture and sound, each 32 pt and 4 apart, in from the row's 16.
    static func panelTrailing(_ which: Panel, playOnTV: Bool = false) -> CGFloat {
        let tv: CGFloat = playOnTV ? 36 : 0
        return switch which {
        case .tracks, .episodes: 16
        case .picture: 84 + tv
        case .speed: 84 + tv + 36
        }
    }

    /// How wide a panel is: Picture and sound wider, so the engine's notes on the upscaling
    /// looks fit on their rows.
    static func panelWidth(_ which: Panel) -> CGFloat {
        which == .picture ? 368 : 264
    }

    static func panelHangs(_ which: Panel) -> Alignment {
        switch which {
        case .tracks, .episodes: .topTrailing
        case .speed, .picture: .bottomTrailing
        }
    }

    /// The row in use in a panel, which a list too long for the window scrolls to as it opens.
    private func selectedRow(_ which: Panel) -> String? {
        switch which {
        case .tracks: TrackLists.selectedRow(model)
        case .episodes: EpisodeList.selectedRow(model)
        case .speed, .picture: nil
        }
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
            case "p" where command && option: model.togglePictureInPicture()
            case "m": model.toggleMute()
            case "n": model.step(shift ? -1 : 1)
            case "f" where !command: model.fullScreen.toggle()
            case "e": toggle(.episodes)
            case "i" where !command: model.toggleStats()
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
        PlayerPointer.shared.release()
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

    /// Into or out of full screen as the model asks, on the next turn of the run loop. The
    /// change arrives in SwiftUI's update, inside the hosting view's layout: a transition started
    /// there resizes the window under that layout, which then sizes the window back from the
    /// size it had, and leaves the content view off by the difference (the video in a box at
    /// the window's corner, the title strip in mid-window). By the time it runs, the window may
    /// already be where the model says, or between the two, and AppKit's own notices bring the
    /// model back to what the window does.
    private func fullScreen(_ on: Bool) {
        DispatchQueue.main.async { [weak window, model] in
            guard let window, model.fullScreen == on, window.styleMask.contains(.fullScreen) != on else { return }
            if on { enteredFullScreen = true }
            window.toggleFullScreen(nil)
        }
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
        .accessibilityLabel(L.playerVolume)
        .accessibilityValue((model.volume / 100).formatted(.percent.locale(AirTime.locale)))
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
