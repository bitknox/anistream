// Picture in Picture on the Mac: the system's floating window, showing mpv's own frames.
//
// mpv keeps playing in the one player throughout. While Picture in Picture runs, `PictureFeed`
// copies each frame mpv presents, after the shaders and with the subtitles, into the
// AVSampleBufferDisplayLayer the system's window shows; the picture's layer leaves the main
// window, which goes back to browsing, and renders at the size of the floating window. Returning
// puts the layer back in the main window's player. Nothing reloads either way.

#if os(macOS)
import AnistreamPlayer
import AppKit
import AVKit
import CoreImage
import CoreMedia

/// The system's side of Picture in Picture, as `PictureInPicture` drives it: AVKit's controller in
/// the app, a stand-in in the tests.
@MainActor
protocol PictureInPictureSystem: AnyObject {
    /// What the system says: started, failed, restore, stopped.
    var events: ((PictureInPictureEvent) -> Void)? { get set }
    /// The size the floating window renders at, in pixels, as it changes.
    var renderSize: ((CGSize) -> Void)? { get set }
    /// Ask for the floating window.
    func start()
    /// Ask the floating window to close.
    func stop()
    /// The playback paused, played or changed length: the window's buttons follow.
    func playbackChanged()
}

/// Picture in Picture for one player: the button, ⌥⌘P, the window minimised or the app hidden
/// while it plays, and the floating window's own buttons, carried out on the player's model.
@MainActor
public final class PictureInPicture {
    public private(set) var machine = PictureInPictureMachine()
    let feed: PictureFeed
    let system: PictureInPictureSystem
    weak var model: PlayerModel?
    /// Brings the main window back to the front, out of the Dock and out of hiding.
    var bringBackWindow: () -> Void = {}
    /// The size the floating window renders at, in pixels, as the system last said: mpv draws at
    /// it while the picture floats. A 16:9 window of 640 points until the system says.
    private var renderPixels = CGSize(width: 1280, height: 720)

    init(model: PlayerModel, feed: PictureFeed, system: PictureInPictureSystem) {
        self.model = model
        self.feed = feed
        self.system = system
        system.events = { [weak self] event in self?.receive(event) }
        system.renderSize = { [weak self] size in self?.rendering(at: size) }
        model.pictureInPictureToggle = { [weak self] in self?.toggle() }
        model.pictureInPictureFrame = { [feed] in feed.latestFrame.flatMap(Self.image) }
        watchPlayback()
        watchStill()
    }

    /// Picture in Picture for `model` on AVKit.
    public convenience init(model: PlayerModel) {
        let feed = PictureFeed()
        self.init(model: model, feed: feed, system: SystemPictureInPicture(feed: feed, model: model))
    }

    public var phase: PictureInPicturePhase { machine.phase }

    /// A frame as an 8-bit sRGB image.
    static func image(_ buffer: CVPixelBuffer) -> CGImage? {
        let image = CIImage(cvPixelBuffer: buffer)
        return CIContext().createCGImage(
            image, from: image.extent, format: .RGBA8, colorSpace: CGColorSpace(name: CGColorSpace.sRGB))
    }

    /// Into the floating window, or back out of it.
    public func toggle() { receive(.toggle) }

    /// The window was minimised or the app hidden: into the floating window if the player plays
    /// in the main window.
    public func leaving() {
        guard let model, model.isPresented, !model.paused, model.hasPicture, machine.phase == .inline else { return }
        PlayerModel.note(.info, "picture in picture: starting as the window leaves")
        receive(.toggle)
    }

    /// The player closed: the floating window goes with it.
    public func playerClosed() { receive(.playbackClosed) }

    func receive(_ event: PictureInPictureEvent) {
        let before = machine.phase
        let effects = machine.handle(event)
        if before != machine.phase || !effects.isEmpty {
            PlayerModel.note(.info, "picture in picture: \(event) in \(before) → \(machine.phase), \(effects)")
        }
        for effect in effects { carryOut(effect) }
        model?.pictureInPicturePhase = machine.phase
    }

    private func carryOut(_ effect: PictureInPictureEffect) {
        guard let model else { return }
        switch effect {
        case .start:
            guard let layer = model.layer, model.isPresented else {
                receive(.failedToStart)
                return
            }
            feed.start(layer)
            // Paused, mpv draws nothing new for the feed to copy: its screenshot instead.
            if model.paused { showStill() }
            system.start()
        case .stop, .dismiss:
            system.stop()
        case .leaveWindow:
            // Out of the mini player too: Picture in Picture takes over from it.
            model.miniPlayer = false
            model.pictureInPicture = true
            // The floating window has no controls of ours for the subtitles to rise above.
            model.liftSubtitles(0, animated: false)
            if let layer = model.layer {
                // No view hosts it while it floats: it renders at the floating window's size,
                // unseen, and the feed copies that.
                layer.removeFromSuperlayer()
                resizeFloating(layer)
            }
        case .returnToWindow:
            model.pictureInPicture = false
            bringBackWindow()
        case .endFeed:
            feed.stop()
            feed.flush()
        case .closePlayback:
            model.viewerClosed()
        }
    }

    /// The floating window changed size: mpv renders at its size, so the subtitles and the
    /// controls' text are drawn for it, and nothing larger than needed is copied.
    private func rendering(at pixels: CGSize) {
        guard pixels.width > 1, pixels.height > 1 else { return }
        PlayerModel.note(.info, "picture in picture renders at \(Int(pixels.width))×\(Int(pixels.height)) px")
        renderPixels = pixels
        guard machine.phase == .active, let layer = model?.layer, layer.superlayer == nil else { return }
        resizeFloating(layer)
    }

    /// The floating picture at the video's own shape, inside the size the window renders at: the
    /// window takes its shape from the frames, which in the main window had its bars.
    private func resizeFloating(_ layer: MpvLayer) {
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        let aspect = model?.videoAspect ?? 16 / 9
        var size = renderPixels
        if size.width / size.height > aspect {
            size.width = (size.height * aspect).rounded()
        } else {
            size.height = (size.width / aspect).rounded()
        }
        layer.resize(to: CGSize(width: size.width / scale, height: size.height / scale), scale: scale)
        model?.surfaceResized()
    }

    /// What mpv shows while paused, as it changes: paused, the feed's own frames stop about three
    /// short of it (see `PictureFeed`), and a skip in the floating window draws one frame and no
    /// more.
    private func watchStill() {
        withObservationTracking {
            _ = model?.paused
            _ = model?.position
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                if self.model?.paused == true, self.machine.phase != .inline { self.showStill() }
                self.watchStill()
            }
        }
    }

    private var stillTask: Task<Void, Never>?
    private var stillAgain = false

    /// mpv's screenshot of the window into the floating window, one at a time; a change while
    /// one is being taken takes another after it.
    private func showStill() {
        guard stillTask == nil else {
            stillAgain = true
            return
        }
        stillTask = Task { @MainActor [weak self] in
            guard let self else { return }
            repeat {
                self.stillAgain = false
                if let image = await self.model?.snapshot(), self.feed.isRunning { self.feed.show(image) }
            } while self.stillAgain
            self.stillTask = nil
        }
    }

    /// The floating window's play and pause follow the player's.
    private func watchPlayback() {
        withObservationTracking {
            _ = model?.paused
            _ = model?.duration
            _ = model?.isPresented
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.system.playbackChanged()
                if self.model?.isPresented == false, self.machine.phase != .inline { self.playerClosed() }
                self.watchPlayback()
            }
        }
    }
}

/// AVKit's Picture in Picture, on the feed's display layer.
@MainActor
final class SystemPictureInPicture: NSObject, PictureInPictureSystem {
    var events: ((PictureInPictureEvent) -> Void)?
    var renderSize: ((CGSize) -> Void)?
    private let feed: PictureFeed
    private weak var model: PlayerModel?
    private var controller: AVPictureInPictureController?
    private var waiting: Task<Void, Never>?
    private var stage: NSWindow?

    init(feed: PictureFeed, model: PlayerModel) {
        self.feed = feed
        self.model = model
    }

    func start() {
        let controller = self.controller ?? make()
        setStage()
        waiting?.cancel()
        waiting = Task { @MainActor [weak self] in
            // AVKit sizes its window from the frames the display layer has shown, and with none
            // shown yet it computes a NaN frame, which AppKit traps on. So: frames first.
            let deadline = ContinuousClock.now + .seconds(3)
            while let self, self.feed.counts.copied < 1, ContinuousClock.now < deadline {
                try? await Task.sleep(for: .milliseconds(20))
            }
            guard let self, !Task.isCancelled else { return }
            guard self.feed.counts.copied >= 1, controller.isPictureInPicturePossible else {
                PlayerModel.note(.error, "picture in picture: no frames, or not possible (\(controller.isPictureInPicturePossible))")
                self.clearStage()
                self.events?(.failedToStart)
                return
            }
            // One more frame's time, for the display layer to put the last one up.
            try? await Task.sleep(for: .milliseconds(50))
            PlayerModel.note(.info, "picture in picture: asking the system, \(self.feed.counts.copied) frames in")
            controller.startPictureInPicture()
        }
    }

    func stop() {
        waiting?.cancel()
        controller?.stopPictureInPicture()
    }

    /// The display layer in a window of its own while Picture in Picture runs. AVKit puts a view of
    /// its own beside the layer, into the root view of the layer's window, and AppKit traps when
    /// that root is a SwiftUI hosting view, as every window of the app's is; with the layer in no
    /// window, AVKit starts nothing. So the layer gets a plain window: borderless, transparent,
    /// passing clicks through, behind the main window and at its frame, where the floating window
    /// grows from and shrinks back to.
    private func setStage() {
        let main = NSApp.windows.first { $0.isVisible && !($0 is NSPanel) && $0 !== stage }
        let frame = main?.frame ?? NSScreen.main?.visibleFrame ?? CGRect(x: 0, y: 0, width: 960, height: 540)
        let stage = self.stage ?? {
            let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.identifier = NSUserInterfaceItemIdentifier("picture-in-picture-stage")
            window.isOpaque = false
            window.backgroundColor = .clear
            window.hasShadow = false
            window.ignoresMouseEvents = true
            window.collectionBehavior = [.transient, .ignoresCycle]
            let view = NSView(frame: CGRect(origin: .zero, size: frame.size))
            view.wantsLayer = true
            view.autoresizingMask = [.width, .height]
            window.contentView = view
            return window
        }()
        self.stage = stage
        stage.setFrame(frame, display: false)
        let layer = feed.displayLayer
        if let host = stage.contentView?.layer, layer.superlayer !== host {
            layer.removeFromSuperlayer()
            host.addSublayer(layer)
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        layer.frame = stage.contentView?.bounds ?? .zero
        CATransaction.commit()
        if let main {
            stage.order(.below, relativeTo: main.windowNumber)
        } else {
            stage.orderBack(nil)
        }
    }

    /// The stage goes once the floating window has closed.
    func clearStage() {
        stage?.orderOut(nil)
    }

    func playbackChanged() {
        controller?.invalidatePlaybackState()
    }

    private func make() -> AVPictureInPictureController {
        let source = AVPictureInPictureController.ContentSource(
            sampleBufferDisplayLayer: feed.displayLayer, playbackDelegate: self)
        let controller = AVPictureInPictureController(contentSource: source)
        controller.delegate = self
        self.controller = controller
        return controller
    }
}

extension SystemPictureInPicture: AVPictureInPictureControllerDelegate {
    nonisolated func pictureInPictureControllerDidStartPictureInPicture(_ controller: AVPictureInPictureController) {
        MainActor.assumeIsolated { events?(.started) }
    }

    nonisolated func pictureInPictureController(
        _ controller: AVPictureInPictureController, failedToStartPictureInPictureWithError error: any Error
    ) {
        MainActor.assumeIsolated {
            PlayerModel.note(.error, "picture in picture failed to start: \(error.localizedDescription)")
            clearStage()
            events?(.failedToStart)
        }
    }

    nonisolated func pictureInPictureController(
        _ controller: AVPictureInPictureController,
        restoreUserInterfaceForPictureInPictureStopWithCompletionHandler completionHandler: @escaping (Bool) -> Void
    ) {
        MainActor.assumeIsolated { events?(.restore) }
        completionHandler(true)
    }

    nonisolated func pictureInPictureControllerDidStopPictureInPicture(_ controller: AVPictureInPictureController) {
        MainActor.assumeIsolated {
            clearStage()
            events?(.stopped)
        }
    }
}

extension SystemPictureInPicture: AVPictureInPictureSampleBufferPlaybackDelegate {
    nonisolated func pictureInPictureController(_ controller: AVPictureInPictureController, setPlaying playing: Bool) {
        MainActor.assumeIsolated {
            guard let model, model.paused == playing else { return }
            model.togglePause()
        }
    }

    nonisolated func pictureInPictureControllerTimeRangeForPlayback(_ controller: AVPictureInPictureController) -> CMTimeRange {
        MainActor.assumeIsolated {
            guard let duration = model?.duration, duration > 0 else {
                // Not known yet: as for a live stream.
                return CMTimeRange(start: .negativeInfinity, duration: .positiveInfinity)
            }
            return CMTimeRange(start: .zero, duration: CMTime(seconds: duration, preferredTimescale: 1000))
        }
    }

    nonisolated func pictureInPictureControllerIsPlaybackPaused(_ controller: AVPictureInPictureController) -> Bool {
        MainActor.assumeIsolated { model?.paused ?? true }
    }

    nonisolated func pictureInPictureController(
        _ controller: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions
    ) {
        MainActor.assumeIsolated {
            renderSize?(CGSize(width: Int(newRenderSize.width), height: Int(newRenderSize.height)))
        }
    }

    nonisolated func pictureInPictureController(
        _ controller: AVPictureInPictureController, skipByInterval skipInterval: CMTime,
        completion completionHandler: @escaping () -> Void
    ) {
        let seconds = skipInterval.seconds
        MainActor.assumeIsolated { model?.seek(by: seconds) }
        completionHandler()
    }
}
#endif
