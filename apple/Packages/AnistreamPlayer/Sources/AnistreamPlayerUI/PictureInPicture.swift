// Picture in Picture: the system's floating window, showing mpv's own frames, on the Mac, iPhone
// and iPad.
//
// mpv keeps playing in the one player throughout. While Picture in Picture runs, `PictureFeed`
// copies each frame mpv presents, after the shaders and with the subtitles, into the
// AVSampleBufferDisplayLayer the system's window shows; the picture's layer leaves the player,
// the app goes back to browsing, and the picture renders at the size of the floating window.
// Returning puts the layer back in the player. Nothing reloads either way.
//
// - Mac: the button, ⌥⌘P, the Window menu, or the window minimised or the app hidden while it
//   plays. The display layer sits in a plain window of its own while it runs (`setStage`).
// - iPhone and iPad: the button, or going home while it plays. The system does that by itself,
//   from a display layer it sees playing where the picture is: so there the feed runs whenever
//   the player is up, small, into a display layer beneath the picture, and at the floating
//   window's size while the picture floats.

import AnistreamPlayer
import AVKit
import CoreImage
import CoreMedia
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The system's side of Picture in Picture, as `PictureInPicture` drives it: AVKit's controller in
/// the app, a stand-in in the tests.
@MainActor
protocol PictureInPictureSystem: AnyObject {
    /// What the system says: starting by itself, started, failed, restore, stopped.
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
/// while it plays, going home while it plays, and the floating window's own buttons, carried out
/// on the player's model.
@MainActor
public final class PictureInPicture {
    public private(set) var machine = PictureInPictureMachine()
    let feed: PictureFeed
    let system: PictureInPictureSystem
    weak var model: PlayerModel?
    /// The feed runs whenever the player is up, not only while the picture floats, into a display
    /// layer beneath the picture: iPhone and iPad, where the system starts by itself from it.
    let inline: Bool
    /// Brings the main window back to the front, out of the Dock and out of hiding (Mac).
    var bringBackWindow: () -> Void = {}
    /// The size the floating window renders at, in pixels, as the system last said: mpv draws at
    /// it while the picture floats. A 16:9 window of 640 points until the system says.
    private var renderPixels = CGSize(width: 1280, height: 720)
    /// The layer the feed copies, while it runs.
    private weak var feedLayer: MpvLayer?

    /// While the player is up on iPhone and iPad, frames no larger than this: enough for the
    /// system's window to open from, at a fraction of the full copy's cost.
    static let inlineSize = CGSize(width: 960, height: 540)

    init(model: PlayerModel, feed: PictureFeed, system: PictureInPictureSystem, inline: Bool = false) {
        self.model = model
        self.feed = feed
        self.system = system
        self.inline = inline
        system.events = { [weak self] event in self?.receive(event) }
        system.renderSize = { [weak self] size in self?.rendering(at: size) }
        model.pictureInPictureToggle = { [weak self] in self?.toggle() }
        model.pictureInPictureFrame = { [feed] in feed.latestFrame.flatMap(Self.image) }
        if inline { model.pictureInPictureLayer = feed.displayLayer }
        watchPlayback()
        watchStill()
        if inline { watchInline() }
    }

    /// Picture in Picture for `model` on AVKit.
    public convenience init(model: PlayerModel) {
        let feed = PictureFeed()
        #if os(macOS)
        let inline = false
        #else
        let inline = true
        #endif
        self.init(model: model, feed: feed, system: SystemPictureInPicture(feed: feed, model: model), inline: inline)
    }

    /// Whether the device has Picture in Picture at all.
    public static var isSupported: Bool { AVPictureInPictureController.isPictureInPictureSupported() }

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
            fitFeed()
            startFeed(on: layer)
            // Paused, mpv draws nothing new for the feed to copy: its screenshot instead.
            if model.paused { showStill() }
            system.start()
        case .startFeed:
            fitFeed()
            if let layer = model.layer { startFeed(on: layer) }
        case .stop, .dismiss:
            system.stop()
        case .leaveWindow:
            model.pictureInPicture = true
            // The floating window has no controls of ours for the subtitles to rise above.
            model.liftSubtitles(0, animated: false)
            // All of it from here: the picture renders at the floating window's size and shape.
            fitFeed()
            if let layer = model.layer {
                // No view hosts it while it floats: it renders at the floating window's size,
                // unseen, and the feed copies that.
                layer.removeFromSuperlayer()
                resizeFloating(layer)
            }
        case .returnToWindow:
            model.pictureInPicture = false
            fitFeed()
            bringBackWindow()
        case .endFeed:
            if inline, model.isPresented, model.layer != nil, model.layer === feedLayer {
                // Back in the player: the copying goes on, small, for the next time.
                fitFeed()
            } else {
                stopFeed()
            }
        case .closePlayback:
            model.viewerClosed()
        }
    }

    /// The feed's size and part for where the picture is. In the player, mpv draws the video with
    /// bars to the player's shape, and only the video's part is copied, small on iPhone and
    /// iPad, so the floating window opens at the video's shape. Floating, the picture is that
    /// shape already, and is copied at the window's size; the part still cuts a frame drawn at
    /// the player's shape on the way.
    private func fitFeed() {
        feed.contentAspect = model?.videoAspect
        feed.maximumSize = model?.pictureInPicture == true || !inline ? nil : Self.inlineSize
    }

    private func startFeed(on layer: MpvLayer) {
        if feed.isRunning, feedLayer === layer { return }
        if feed.isRunning { feed.stop() }
        feed.start(layer)
        feedLayer = layer
    }

    private func stopFeed() {
        feed.stop()
        feed.flush()
        feedLayer = nil
    }

    /// iPhone and iPad: the feed runs while the player is up, small, so that the system can
    /// open its window from it by itself; it stops as the player closes.
    private func watchInline() {
        withObservationTracking {
            _ = model?.layer
            _ = model?.isPresented
            _ = model?.videoAspect
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.followPlayer()
                self.watchInline()
            }
        }
        followPlayer()
    }

    private func followPlayer() {
        guard let model else { return }
        if model.isPresented, let layer = model.layer {
            fitFeed()
            guard !feed.isRunning || feedLayer !== layer else { return }
            startFeed(on: layer)
            PlayerModel.note(.info, "picture in picture: copying the player's frames, \(feed.isRunning ? "small" : "but there is no GPU to copy with")")
        } else if machine.phase == .inline, feed.isRunning {
            stopFeed()
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
        // The screen's scale, as the player's host last sized the layer for it.
        let scale = layer.contentsScale > 0 ? layer.contentsScale : 2
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
    private var possible: NSKeyValueObservation?
    private var waiting: Task<Void, Never>?
    #if os(macOS)
    private var stage: NSWindow?
    #endif

    init(feed: PictureFeed, model: PlayerModel) {
        self.feed = feed
        self.model = model
        super.init()
        #if os(iOS)
        // There for the system from the start: it opens its window by itself as the viewer goes
        // home while the player plays.
        _ = make()
        #endif
    }

    func start() {
        let controller = self.controller ?? make()
        #if os(macOS)
        setStage()
        #endif
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
                #if os(macOS)
                self.clearStage()
                #endif
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

    func playbackChanged() {
        controller?.invalidatePlaybackState()
    }

    private func make() -> AVPictureInPictureController {
        let source = AVPictureInPictureController.ContentSource(
            sampleBufferDisplayLayer: feed.displayLayer, playbackDelegate: self)
        let controller = AVPictureInPictureController(contentSource: source)
        controller.delegate = self
        #if os(iOS)
        controller.canStartPictureInPictureAutomaticallyFromInline = true
        #endif
        // In the log: whether the system would open its window now, as that changes.
        possible = controller.observe(\.isPictureInPicturePossible, options: [.initial, .new]) { _, change in
            let now = change.newValue ?? false
            PlayerModel.note(.info, "picture in picture \(now ? "possible" : "not possible")")
        }
        self.controller = controller
        return controller
    }

    #if os(macOS)
    /// The display layer in a window of its own while Picture in Picture runs. AVKit puts a view of
    /// its own beside the layer, into the root view of the layer's window, and AppKit traps when
    /// that root is a SwiftUI hosting view, as every window of the app's is; with the layer in no
    /// window, AVKit starts nothing. So the layer gets a plain window: borderless, transparent,
    /// passing clicks through, behind the main window and at its frame, where the floating window
    /// grows from and shrinks back to.
    private func setStage() {
        let main = NSApp.windows.first { $0.isVisible && !($0 is NSPanel) && $0 !== self.stage }
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
    #else
    func clearStage() {}

    /// Whether the display layer is back in a window: the player full screen again, where the
    /// floating window shrinks into.
    var displayLayerShown: Bool {
        (feed.displayLayer.superlayer?.delegate as? UIView)?.window != nil
    }
    #endif
}

/// AVKit's completion handlers, carried to the main actor and called there.
private struct Completion: @unchecked Sendable {
    let handler: (Bool) -> Void
}

extension SystemPictureInPicture: AVPictureInPictureControllerDelegate {
    nonisolated func pictureInPictureControllerWillStartPictureInPicture(_ controller: AVPictureInPictureController) {
        // After the app's own ask, nothing; begun by the system, the machine follows.
        MainActor.assumeIsolated { events?(.systemStarting) }
    }

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
        #if os(macOS)
        MainActor.assumeIsolated { events?(.restore) }
        completionHandler(true)
        #else
        // The player comes back full screen first, so the floating window shrinks into it: done
        // once the display layer beneath the picture is in a window again, or after 2 s.
        let completion = Completion(handler: completionHandler)
        MainActor.assumeIsolated {
            events?(.restore)
            Task { @MainActor [weak self] in
                let deadline = ContinuousClock.now + .seconds(2)
                while let self, !self.displayLayerShown, ContinuousClock.now < deadline {
                    try? await Task.sleep(for: .milliseconds(20))
                }
                completion.handler(true)
            }
        }
        #endif
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
