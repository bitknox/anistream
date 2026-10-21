// The iPhone's and iPad's player, full screen over the app (T2–T5): the PhonePlayer boards, the
// gestures AVKit has taught, and the audio playing on in the background.

#if os(iOS)
import AVKit
import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import SwiftUI
import UIKit

/// The player's content in its full-screen cover. The controls fade in on a tap and out 3 s
/// after (T4); they stay while paused, while scrubbing and while a sheet is up.
public struct PhonePlayerView: View {
    let model: PlayerModel
    @State private var sheet: Sheet?
    @GestureState private var drag: CGFloat = 0
    @GestureState private var held = false
    @State private var heldSpeed: Double?
    @State private var flash: SeekFlash?
    @State private var height: CGFloat = 0
    @Environment(\.scenePhase) private var phase
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let metrics = PlayerMetrics.phone

    enum Sheet: String, Identifiable {
        case tracks, episodes
        var id: String { rawValue }
    }

    /// Double taps' running count on one side: "+20 s".
    struct SeekFlash: Equatable {
        var forward: Bool
        var seconds: Int
        var at: ContinuousClock.Instant
    }

    public init(model: PlayerModel) {
        self.model = model
    }

    private var visible: Bool { (model.controlsVisible || sheet != nil) && !model.stillWatching && model.failure == nil && !curtain }
    private var curtain: Bool { model.starting != nil || !model.hasPicture || model.holdsCurtain }
    /// How far the swipe down has gone toward closing, 0 to 1.
    private var pull: CGFloat { min(drag / 120, 1) }

    public var body: some View {
        GeometryReader { proxy in
            ZStack {
                Color.black
                ZStack {
                    PlayerPicture(model: model)
                    StartingCurtain(model: model, metrics: metrics)
                        .opacity(curtain ? 1 : 0)
                        .animation(curtain ? .easeIn(duration: 0.25) : .fadeIn, value: curtain)
                        .allowsHitTesting(curtain)
                }
                .scaleEffect(reduceMotion ? 1 : 1 - 0.1 * pull)
                .clipShape(.rect(cornerRadius: reduceMotion ? 0 : 16 * pull))
                .opacity(reduceMotion ? 1 - 0.5 * pull : 1)
                .offset(y: reduceMotion ? 0 : drag)
                .animation(.standard, value: drag == 0)

                if model.showsSpinner {
                    ProgressView().controlSize(.large).tint(.silk).environment(\.colorScheme, .dark)
                }
                Group {
                    Palette.controlsDim.allowsHitTesting(false)
                    chrome
                }
                .opacity(visible && drag == 0 ? 1 : 0)
                .animation(visible ? .fadeIn : .fadeOut, value: visible)
                .allowsHitTesting(visible)

                CornerCard(model: model, metrics: metrics)
                    .padding(metrics.cornerInset)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                ResumeNoteView(model: model, metrics: metrics)
                    .padding(.top, 72)
                    .padding(.leading, 56)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .animation(.standard, value: model.resume)
                PlayerAlertView(model: model, metrics: metrics)
                    .padding(.top, 72)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .animation(.standard, value: model.alert)
                if held {
                    Text("2×")
                        .font(.system(size: 15, weight: .semibold))
                        .padding(.horizontal, 14)
                        .frame(height: 32)
                        .overVideo(radius: 16)
                        .padding(.top, 20)
                        .frame(maxHeight: .infinity, alignment: .top)
                        .transition(.opacity)
                }
                if let flash {
                    Text(flash.forward ? "+\(flash.seconds) s" : "−\(flash.seconds) s")
                        .font(.system(size: 17, weight: .semibold))
                        .monospacedDigit()
                        .padding(.horizontal, 16)
                        .frame(height: 40)
                        .overVideo(radius: 20)
                        .frame(maxWidth: .infinity, alignment: flash.forward ? .trailing : .leading)
                        .padding(.horizontal, proxy.size.width / 8)
                        .transition(.opacity)
                }
                if model.stillWatching {
                    Palette.controlsDim
                    StillWatchingDialog(model: model, metrics: metrics)
                }
            }
            .foregroundStyle(.silk)
            .contentShape(.rect)
            .gesture(taps(width: proxy.size.width))
            .simultaneousGesture(dismissal)
            .simultaneousGesture(speedHold(width: proxy.size.width))
            .simultaneousGesture(
                MagnifyGesture().onEnded { value in model.setFill(value.magnification > 1) }
            )
        }
        .ignoresSafeArea()
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .sensoryFeedback(.impact(weight: .light), trigger: drag > 120) { _, crossed in crossed }
        .sheet(item: $sheet) { sheet in sheetContent(sheet) }
        .onChange(of: sheet) { _, open in model.hold("sheet", open != nil) }
        .onChange(of: held) { _, on in speedHeld(on) }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { height = $0 }
        .onChange(of: SubtitleLift(visible: visible, height: height), initial: true) { _, lift in
            // The subtitles rise above the bar while the controls are up, as PhonePlayer has them.
            model.liftSubtitles(lift.visible ? metrics.subtitleLift / max(lift.height, 1) : 0, animated: !reduceMotion)
        }
        .onChange(of: phase) { _, phase in
            if phase == .background { model.backgrounded(true) }
            if phase == .active { model.backgrounded(false) }
        }
        .onAppear { PlayerOrientation.landscape(true) }
        .onDisappear { PlayerOrientation.landscape(false) }
        .preferredColorScheme(.dark)
    }

    // MARK: Chrome

    private var chrome: some View {
        VStack(spacing: 0) {
            header
            Spacer(minLength: 0)
            bottomBar
        }
        .overVideoText()
    }

    private var header: some View {
        HStack(spacing: 12) {
            Button { model.viewerClosed() } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 17, weight: .semibold))
                    .frame(width: 44, height: 44)
                    .background(Palette.ground.opacity(0.62), in: .circle)
                    .overlay(Circle().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Close player")
            VStack(alignment: .leading, spacing: 1) {
                Text(model.showTitle)
                    .font(.showTitle(size: 15))
                    .fixedLineHeight(20)
                Text(model.episodeLine)
                    .font(.system(size: 13))
                    .contentTransition(.opacity)
                    .animation(.standard, value: model.episodeLine)
            }
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 0) {
                headerButton("captions.bubble", "Subtitles and audio") { sheet = .tracks }
                AirPlayButton()
                    .frame(width: 44, height: 44)
                    .accessibilityLabel("AirPlay")
                headerButton("list.bullet", "Episodes") { sheet = .episodes }
            }
            .background(Palette.ground.opacity(0.62), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
        }
        .frame(height: 44)
        .padding(.horizontal, 56)
        .padding(.top, 16)
    }

    private var bottomBar: some View {
        HStack(spacing: 4) {
            barButton("gobackward.10", "Back 10 seconds") { model.seek(by: -10) }
            barButton(model.paused ? "play.fill" : "pause.fill", model.paused ? "Play" : "Pause") {
                model.togglePause()
            }
            barButton("goforward.10", "Forward 10 seconds") { model.seek(by: 10) }
            PlayerTime(seconds: model.position, metrics: metrics)
                .frame(width: 40, alignment: .leading)
                .padding(.leading, 8)
            SeekLine(model: model, metrics: metrics)
                .padding(.horizontal, 8)
            PlayerTime(seconds: max(0, (model.duration ?? 0) - model.position), remaining: true, metrics: metrics)
                .frame(width: 52, alignment: .trailing)
        }
        .frame(height: 44)
        .padding(.horizontal, 56)
        .padding(.bottom, 26)
    }

    private func headerButton(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    private func barButton(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 20, weight: .medium))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    @ViewBuilder
    private func sheetContent(_ sheet: Sheet) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                switch sheet {
                case .tracks:
                    TrackLists(model: model, metrics: metrics)
                    MenuDivider()
                    MoreLists(model: model, metrics: metrics)
                case .episodes:
                    EpisodeList(model: model, metrics: metrics)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .presentationDetents([.medium, .large])
        .presentationBackground(Palette.ground)
        .presentationDragIndicator(.visible)
        .preferredColorScheme(.dark)
    }

    // MARK: Gestures

    /// A tap shows or hides the controls and never pauses; a double tap on the left or right
    /// third seeks 10 s, and each further one within 0.6 s adds 10 more.
    private func taps(width: CGFloat) -> some Gesture {
        SpatialTapGesture(count: 2)
            .onEnded { value in
                let third = value.location.x / max(width, 1)
                guard third < 1 / 3 || third > 2 / 3 else {
                    toggleControls()
                    return
                }
                let forward = third > 2 / 3
                let now = ContinuousClock.now
                var next = SeekFlash(forward: forward, seconds: 10, at: now)
                if let flash, flash.forward == forward, now - flash.at < .milliseconds(600) {
                    next.seconds = flash.seconds + 10
                }
                withAnimation(.quick) { flash = next }
                model.seek(by: forward ? 10 : -10)
                Task {
                    try? await Task.sleep(for: .milliseconds(900))
                    if flash?.at == now { withAnimation(.fadeOut) { flash = nil } }
                }
            }
            .exclusively(before: TapGesture().onEnded { toggleControls() })
    }

    private func toggleControls() {
        if model.controlsVisible && !model.paused { model.hideControls() } else { model.poke() }
    }

    /// The video follows the finger down and closes past 120 pt or on a fast flick, otherwise
    /// springs back (T3).
    private var dismissal: some Gesture {
        DragGesture(minimumDistance: 12)
            .updating($drag) { value, state, _ in
                guard model.scrubbing == nil, abs(value.translation.height) > abs(value.translation.width) else { return }
                state = max(0, value.translation.height)
            }
            .onEnded { value in
                guard model.scrubbing == nil else { return }
                if value.translation.height > 120 || value.velocity.height > 800 { model.viewerClosed() }
            }
    }

    /// A long press on the right half plays at 2× while held.
    private func speedHold(width: CGFloat) -> some Gesture {
        LongPressGesture(minimumDuration: 0.5)
            .sequenced(before: DragGesture(minimumDistance: 0))
            .updating($held) { value, state, _ in
                if case .second(true, let drag?) = value, drag.startLocation.x > width / 2 { state = true }
            }
    }

    private func speedHeld(_ on: Bool) {
        if on {
            heldSpeed = model.speed
            model.setSpeed(2)
        } else if let heldSpeed {
            model.setSpeed(heldSpeed)
            self.heldSpeed = nil
        }
    }
}

/// AirPlay for audio: AirPlay video needs AVPlayer and a stream the receiver fetches itself.
struct AirPlayButton: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = UIColor(Palette.silk)
        view.activeTintColor = UIColor(Palette.amber)
        view.prioritizesVideoDevices = false
        return view
    }

    func updateUIView(_ view: AVRoutePickerView, context: Context) {}
}

/// The iPhone turns to landscape while the player is up and back to portrait after; the iPad
/// stays as it is held. The app delegate answers `supportedInterfaceOrientationsFor` with `mask`.
@MainActor
public enum PlayerOrientation {
    public private(set) static var mask: UIInterfaceOrientationMask = .allButUpsideDown

    static func landscape(_ on: Bool) {
        guard UIDevice.current.userInterfaceIdiom == .phone,
            let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        else { return }
        mask = on ? .landscape : .allButUpsideDown
        for window in scene.windows {
            window.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
        }
        scene.requestGeometryUpdate(.iOS(interfaceOrientations: on ? .landscape : .portrait)) { _ in }
    }
}
#endif
