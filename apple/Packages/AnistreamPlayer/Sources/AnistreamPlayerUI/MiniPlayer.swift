// The Mac's mini player: the video in a small floating panel in the corner of the screen while
// the main window goes back to browsing. One mpv throughout: the picture's layer moves from the
// main window into the panel and back, and nothing reloads.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import AppKit
import SwiftUI

/// Where the mini player sits: a quarter of the screen's width at 16:9, 16 pt in from the bottom
/// right corner of its visible frame.
enum MiniPlayerFrame {
    static func frame(in visible: CGRect) -> CGRect {
        let width = (visible.width / 4).rounded()
        let height = (width * 9 / 16).rounded()
        return CGRect(x: visible.maxX - width - 16, y: visible.minY + 16, width: width, height: height)
    }
}

/// The panel the mini player floats in, above other windows and on every Space.
@MainActor
final class MiniPlayerPanel {
    private var panel: NSPanel?

    var isShown: Bool { panel != nil }

    func show(_ model: PlayerModel, store: AppStore) {
        guard panel == nil else { return }
        let screen = NSScreen.main ?? NSScreen.screens.first
        let frame = MiniPlayerFrame.frame(in: screen?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1512, height: 945))
        let panel = NSPanel(
            contentRect: frame, styleMask: [.titled, .nonactivatingPanel, .fullSizeContentView, .resizable],
            backing: .buffered, defer: false)
        panel.isReleasedWhenClosed = false
        panel.identifier = NSUserInterfaceItemIdentifier("mini-player")
        panel.title = "Mini Player"
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            panel.standardWindowButton(kind)?.isHidden = true
        }
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isMovableByWindowBackground = true
        panel.backgroundColor = .black
        panel.appearance = NSAppearance(named: .darkAqua)
        panel.contentAspectRatio = NSSize(width: 16, height: 9)
        panel.contentMinSize = NSSize(width: 240, height: 135)
        panel.contentView = NSHostingView(rootView: MiniPlayerView(model: model).environment(store))
        panel.orderFrontRegardless()
        self.panel = panel
        PlayerModel.note(.info, "mini player at \(Int(frame.width))×\(Int(frame.height)) pt, bottom right")
    }

    func hide() {
        guard let panel else { return }
        panel.orderOut(nil)
        panel.contentView = nil
        panel.close()
        self.panel = nil
        PlayerModel.note(.info, "mini player closed")
    }

    #if FIXTURES
    /// The panel, for a screenshot.
    var window: NSWindow? { panel }
    #endif
}

/// The mini player's content: the picture, and on hover play and pause, the seek line, the
/// volume and the way back to the main window.
struct MiniPlayerView: View {
    let model: PlayerModel
    @State private var hovering = false
    @State private var height: CGFloat = 0
    private let metrics = PlayerMetrics.mac

    private var visible: Bool { model.controlsVisible || hovering }

    var body: some View {
        ZStack {
            Color.black
            PlayerPicture(model: model)
            if model.starting != nil || !model.hasPicture {
                Palette.ground
                LoaderLine(reduceMotion: false)
                    .frame(width: 120, height: 2)
            }
            controls
                .opacity(visible ? 1 : 0)
                .animation(visible ? .fadeIn : .fadeOut, value: visible)
                .allowsHitTesting(visible)
        }
        .foregroundStyle(.silk)
        .ignoresSafeArea()
        .onHover { hovering = $0 }
        .onContinuousHover { phase in
            if case .active = phase { model.poke() }
        }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { height = $0 }
        .onChange(of: SubtitleLift(visible: visible, height: height), initial: true) { _, lift in
            // Above the mini player's control row while it is up.
            model.liftSubtitles(lift.visible ? 34 / max(lift.height, 1) : 0, animated: true)
        }
        .preferredColorScheme(.dark)
    }

    private var controls: some View {
        ZStack {
            Scrim(stops: [
                .init(color: Palette.ground.opacity(0), location: 0.35),
                .init(color: Palette.ground.opacity(0.72), location: 1),
            ])
            .allowsHitTesting(false)
            VStack(spacing: 0) {
                HStack {
                    Spacer()
                    button("arrow.up.left.and.arrow.down.right", "Exit Mini Player") { model.miniPlayer = false }
                }
                .padding(.top, 6)
                .padding(.trailing, 6)
                Spacer(minLength: 0)
                HStack(spacing: 4) {
                    button(model.paused ? "play.fill" : "pause.fill", model.paused ? "Play" : "Pause") {
                        model.togglePause()
                    }
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
            }
            .overVideoText()
        }
    }

    private func button(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 14, weight: .medium))
                .frame(width: 30, height: 30)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }
}
#endif
