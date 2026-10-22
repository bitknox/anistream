// Picture in Picture on the Mac, iPhone and iPad: the steps between its states, and what they do
// to the player, on a scripted mpv and a stand-in for the system's floating window.

import AnistreamKit
import AnistreamPlayer
import Foundation
#if canImport(AppKit)
import AppKit
#endif
import AVKit
import CoreMedia
import CoreVideo
import QuartzCore
import Testing

@testable import AnistreamPlayerUI

struct PictureInPictureMachineTests {
    @Test func entersAndReturnsByTheAppsOwnToggle() {
        var machine = PictureInPictureMachine()
        #expect(machine.handle(.toggle) == [.start])
        #expect(machine.phase == .starting)
        #expect(machine.handle(.started) == [.leaveWindow])
        #expect(machine.phase == .active)
        // ⌥⌘P again: the system is asked to close, then asks to restore, then says it stopped.
        #expect(machine.handle(.toggle) == [.stop])
        #expect(machine.handle(.restore) == [.returnToWindow])
        #expect(machine.handle(.stopped) == [.endFeed])
        #expect(machine.phase == .inline)
    }

    @Test func theWindowsReturnButtonRestoresThePicture() {
        var machine = PictureInPictureMachine()
        _ = machine.handle(.toggle)
        _ = machine.handle(.started)
        #expect(machine.handle(.restore) == [.returnToWindow])
        #expect(machine.phase == .stopping(restore: true))
        #expect(machine.handle(.stopped) == [.endFeed])
        #expect(machine.phase == .inline)
    }

    @Test func theWindowsCloseButtonStopsThePlayback() {
        var machine = PictureInPictureMachine()
        _ = machine.handle(.toggle)
        _ = machine.handle(.started)
        // Stopped with no restore before it: closed, so the viewer is done.
        #expect(machine.handle(.stopped) == [.endFeed, .closePlayback])
        #expect(machine.phase == .inline)
    }

    @Test func aPlaybackThatClosesTakesTheWindowWithItAndStopsNothingMore() {
        var machine = PictureInPictureMachine()
        _ = machine.handle(.toggle)
        _ = machine.handle(.started)
        #expect(machine.handle(.playbackClosed) == [.dismiss])
        // The system may still ask to restore; there is nothing to restore to.
        #expect(machine.handle(.restore) == [])
        #expect(machine.handle(.stopped) == [.endFeed])
        #expect(machine.phase == .inline)
    }

    @Test func aWindowThatNeverOpensLeavesThePlayerAsItWas() {
        var machine = PictureInPictureMachine()
        _ = machine.handle(.toggle)
        #expect(machine.handle(.failedToStart) == [.endFeed])
        #expect(machine.phase == .inline)
        // Asked twice before it opened: it closes again as soon as it does.
        _ = machine.handle(.toggle)
        #expect(machine.handle(.toggle) == [.stop])
        #expect(machine.handle(.started) == [])
        #expect(machine.handle(.restore) == [.returnToWindow])
        #expect(machine.handle(.stopped) == [.endFeed])
    }

    @Test func aStartCalledOffBeforeTheSystemWasAskedEndsAsAFailedStart() {
        var machine = PictureInPictureMachine()
        // Asked again while the frames were still coming: the system was never asked, so it never
        // says it stopped, and the start says it failed instead.
        _ = machine.handle(.toggle)
        #expect(machine.handle(.toggle) == [.stop])
        #expect(machine.handle(.failedToStart) == [.endFeed])
        #expect(machine.phase == .inline)
        // The player closing while it starts, the same.
        _ = machine.handle(.toggle)
        #expect(machine.handle(.playbackClosed) == [.dismiss])
        #expect(machine.handle(.failedToStart) == [.endFeed])
        #expect(machine.phase == .inline)
        // And it starts again from there.
        #expect(machine.handle(.toggle) == [.start])
    }

    /// iPhone and iPad: going home while it plays, the system opens its window by itself.
    @Test func theSystemStartingByItselfTakesThePictureOut() {
        var machine = PictureInPictureMachine()
        #expect(machine.handle(.systemStarting) == [.startFeed])
        #expect(machine.phase == .starting)
        #expect(machine.handle(.started) == [.leaveWindow])
        #expect(machine.phase == .active)
        #expect(machine.handle(.restore) == [.returnToWindow])
        #expect(machine.handle(.stopped) == [.endFeed])
        #expect(machine.phase == .inline)
    }

    @Test func theSystemSayingItStartsAfterTheAppAskedChangesNothing() {
        var machine = PictureInPictureMachine()
        _ = machine.handle(.toggle)
        #expect(machine.handle(.systemStarting) == [])
        #expect(machine.phase == .starting)
        // Asked to close before it opened, then opening anyway: still closing.
        _ = machine.handle(.toggle)
        #expect(machine.handle(.systemStarting) == [])
        #expect(machine.phase == .stopping(restore: true))
    }

    @Test func eventsThatMeanNothingWhereItIsChangeNothing() {
        var machine = PictureInPictureMachine()
        for event: PictureInPictureEvent in [.started, .restore, .stopped, .playbackClosed, .failedToStart] {
            #expect(machine.handle(event) == [])
            #expect(machine.phase == .inline)
        }
    }
}

/// The system's floating window as a test drives it.
@MainActor
final class StandInSystem: PictureInPictureSystem {
    var events: ((PictureInPictureEvent) -> Void)?
    var renderSize: ((CGSize) -> Void)?
    var starts = 0
    var stops = 0
    var changes = 0

    func start() { starts += 1 }
    func stop() { stops += 1 }
    func playbackChanged() { changes += 1 }

    func say(_ event: PictureInPictureEvent) { events?(event) }
}

@MainActor
struct PictureInPictureTests {
    /// A player playing in the main window, with Picture in Picture on a stand-in system.
    private func playing() async throws -> (Rig, PictureInPicture, StandInSystem) {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.playbackRestart)
        try await until("the first frame") { rig.model.hasPicture }
        let system = StandInSystem()
        let pip = PictureInPicture(model: rig.model, feed: PictureFeed(), system: system)
        return (rig, pip, system)
    }

    @Test func entersWithThePictureLeavingTheMainWindow() async throws {
        let (rig, pip, system) = try await playing()
        let layer = try #require(rig.model.layer)
        let host = CALayer()
        host.addSublayer(layer)
        #expect(rig.model.pictureInPictureAvailable)
        rig.model.togglePictureInPicture()
        #expect(system.starts == 1)
        #expect(pip.feed.isRunning)
        #expect(rig.model.pictureInPicturePhase == .starting)
        // Until the window has opened the picture stays where it is.
        #expect(rig.model.inMainWindow)
        system.say(.started)
        #expect(rig.model.pictureInPicturePhase == .active)
        #expect(rig.model.pictureInPicture)
        #expect(!rig.model.inMainWindow)
        #expect(layer.superlayer == nil)
        // The same playback: nothing reloaded, nothing stopped.
        #expect(rig.model.isPresented)
        #expect(rig.mpv.calls.filter { if case .load = $0 { true } else { false } }.count == 1)
        #expect(!rig.reports.contains { $0.1 == .closed })
    }

    @Test func restoresThePictureIntoTheMainWindow() async throws {
        let (rig, pip, system) = try await playing()
        var broughtBack = 0
        pip.bringBackWindow = { broughtBack += 1 }
        rig.model.togglePictureInPicture()
        system.say(.started)
        // The floating window's own return button.
        system.say(.restore)
        #expect(!rig.model.pictureInPicture)
        #expect(rig.model.inMainWindow)
        #expect(broughtBack == 1)
        system.say(.stopped)
        #expect(rig.model.pictureInPicturePhase == .inline)
        #expect(!pip.feed.isRunning)
        #expect(rig.model.isPresented)
        #expect(!rig.reports.contains { $0.1 == .closed })
    }

    @Test func exitsByTheToggleAsByTheWindowsButton() async throws {
        let (rig, pip, system) = try await playing()
        rig.model.togglePictureInPicture()
        system.say(.started)
        rig.model.togglePictureInPicture()
        #expect(system.stops == 1)
        system.say(.restore)
        system.say(.stopped)
        #expect(rig.model.inMainWindow)
        #expect(rig.model.pictureInPicturePhase == .inline)
        // The app holds it as long as the player's window; so does the test.
        withExtendedLifetime(pip) {}
    }

    @Test func closingTheWindowStopsThePlayback() async throws {
        let (rig, pip, system) = try await playing()
        var broughtBack = 0
        pip.bringBackWindow = { broughtBack += 1 }
        rig.model.togglePictureInPicture()
        system.say(.started)
        system.say(.stopped)
        // As the player's own close: the file ended by quitting, and the player closed.
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
        #expect(!rig.model.isPresented)
        #expect(!rig.model.pictureInPicture)
        // The main window stays on the browsing.
        #expect(broughtBack == 0)
        #expect(!pip.feed.isRunning)
    }

    @Test func aPlayerThatClosesClosesTheWindow() async throws {
        let (rig, pip, system) = try await playing()
        rig.model.togglePictureInPicture()
        system.say(.started)
        // The engine closes the player: the last episode ended, say.
        rig.ops(1, [.close])
        try await until("the window asked to close") { system.stops == 1 }
        #expect(pip.phase == .stopping(restore: false))
        system.say(.stopped)
        #expect(pip.phase == .inline)
        // Closed once, by the engine, not again by the window.
        #expect(rig.reports.filter { $0.1 == .closed }.count <= 1)
    }

    @Test func startsByItselfOnlyWhilePlaying() async throws {
        let (rig, pip, system) = try await playing()
        rig.mpv.emit(.property(name: "pause", value: .flag(true)))
        try await until("paused") { rig.model.paused }
        pip.leaving()
        #expect(system.starts == 0)
        rig.mpv.emit(.property(name: "pause", value: .flag(false)))
        try await until("playing") { !rig.model.paused }
        // The window minimised, or the app hidden.
        pip.leaving()
        #expect(system.starts == 1)
        #expect(pip.phase == .starting)
    }

    /// Paused, mpv draws nothing new: the floating window starts on mpv's screenshot.
    @Test func startsWhilePausedOnAStill() async throws {
        let (rig, pip, system) = try await playing()
        rig.mpv.emit(.property(name: "pause", value: .flag(true)))
        try await until("paused") { rig.model.paused }
        rig.model.togglePictureInPicture()
        try await until("a still shown") { pip.feed.counts.copied == 1 }
        #expect(system.starts == 1)
        let frame = try #require(pip.feed.latestFrame)
        #expect(CVPixelBufferGetWidth(frame) == 16)
    }

    /// AVKit's own controller, asked to stop before it was asked to start: by a second toggle while
    /// the first frames are copied, or by the player closing then. Stuck in stopping before.
    @Test func aStartCalledOffBeforeAVKitWasAskedLeavesNothingWaiting() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.playbackRestart)
        try await until("the first frame") { rig.model.hasPicture }
        _ = NSApplication.shared
        let feed = PictureFeed()
        let system = SystemPictureInPicture(feed: feed, model: rig.model)
        let pip = PictureInPicture(model: rig.model, feed: feed, system: system)
        // The scripted mpv draws nothing, so the frames AVKit waits for never come.
        rig.model.togglePictureInPicture()
        #expect(pip.phase == .starting)
        #expect(feed.isRunning)
        rig.model.togglePictureInPicture()
        #expect(pip.phase == .inline)
        #expect(!feed.isRunning)

        rig.model.togglePictureInPicture()
        #expect(pip.phase == .starting)
        pip.playerClosed()
        #expect(pip.phase == .inline)
        #expect(!feed.isRunning)
        #expect(rig.model.inMainWindow)
    }

    @Test func aWindowThatNeverOpensLeavesThePictureWhereItWas() async throws {
        let (rig, pip, system) = try await playing()
        rig.model.togglePictureInPicture()
        system.say(.failedToStart)
        #expect(rig.model.inMainWindow)
        #expect(rig.model.pictureInPicturePhase == .inline)
        #expect(!pip.feed.isRunning)
    }

    /// The floating window's own buttons, as AVKit calls them: play and pause, and 10 s either way.
    @Test func theWindowsButtonsPlayPauseAndSkip() async throws {
        let (rig, _, _) = try await playing()
        rig.mpv.emit(.property(name: "duration", value: .double(60)))
        try await until("the duration") { rig.model.duration == 60 }
        let feed = PictureFeed()
        let system = SystemPictureInPicture(feed: feed, model: rig.model)
        let controller = AVPictureInPictureController(
            contentSource: .init(sampleBufferDisplayLayer: feed.displayLayer, playbackDelegate: system))
        #expect(!system.pictureInPictureControllerIsPlaybackPaused(controller))
        #expect(system.pictureInPictureControllerTimeRangeForPlayback(controller)
            == CMTimeRange(start: .zero, duration: CMTime(seconds: 60, preferredTimescale: 1000)))
        // To the engine, as the player's own buttons send them.
        system.pictureInPictureController(controller, setPlaying: false)
        system.pictureInPictureController(controller, skipByInterval: CMTime(seconds: 10, preferredTimescale: 1)) {}
        system.pictureInPictureController(controller, skipByInterval: CMTime(seconds: -10, preferredTimescale: 1)) {}
        #expect(rig.controls.suffix(3) == [.togglePause, .seek(10), .seek(-10)])
        // Already playing: asked to play, nothing more is sent.
        let sent = rig.controls.count
        system.pictureInPictureController(controller, setPlaying: true)
        #expect(rig.controls.count == sent)
    }

    @Test func theWindowsButtonsFollowPauseAndPlay() async throws {
        let (rig, pip, system) = try await playing()
        let before = system.changes
        rig.mpv.emit(.property(name: "pause", value: .flag(true)))
        try await until("the window told") { system.changes > before }
        // The app holds it as long as the player's window; so does the test.
        withExtendedLifetime(pip) {}
    }
}

/// Picture in Picture as iPhone and iPad have it: the feed runs while the player is up, into a
/// display layer beneath the picture, so the system can open its window by itself.
@MainActor
struct InlinePictureInPictureTests {
    private func playing() async throws -> (Rig, PictureInPicture, StandInSystem) {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.playbackRestart)
        try await until("the first frame") { rig.model.hasPicture }
        let system = StandInSystem()
        let pip = PictureInPicture(model: rig.model, feed: PictureFeed(), system: system, inline: true)
        return (rig, pip, system)
    }

    /// The values of `vid` the player set on mpv, in order.
    private func videoTracks(_ rig: Rig) -> [String] {
        rig.mpv.calls.compactMap { call in
            if case .set(property: "vid", let value) = call { value.string } else { nil }
        }
    }

    @Test func theFeedRunsSmallBeneathThePictureWhileThePlayerIsUp() async throws {
        let (rig, pip, _) = try await playing()
        #expect(pip.feed.isRunning)
        #expect(pip.feed.maximumSize == PictureInPicture.inlineSize)
        #expect(rig.model.pictureInPictureLayer === pip.feed.displayLayer)
        rig.model.viewerClosed()
        try await until("the feed stopped with the player") { !pip.feed.isRunning }
    }

    @Test func goingHomeWhilePlayingTakesThePictureOutAndRestoreBringsItBack() async throws {
        let (rig, pip, system) = try await playing()
        let layer = try #require(rig.model.layer)
        let host = CALayer()
        host.addSublayer(layer)
        // The system opens its window as the viewer goes home: the app asked for nothing.
        system.say(.systemStarting)
        #expect(system.starts == 0)
        #expect(pip.phase == .starting)
        system.say(.started)
        #expect(rig.model.pictureInPicture)
        #expect(!rig.model.inMainWindow)
        #expect(layer.superlayer == nil)
        // At the floating window's size from here.
        #expect(pip.feed.maximumSize == nil)
        // The window's return button: the player full screen again, the copying on and small.
        system.say(.restore)
        #expect(rig.model.inMainWindow)
        system.say(.stopped)
        #expect(pip.phase == .inline)
        #expect(pip.feed.isRunning)
        #expect(pip.feed.maximumSize == PictureInPicture.inlineSize)
        #expect(rig.model.isPresented)
        #expect(!rig.reports.contains { $0.1 == .closed })
    }

    @Test func closingTheWindowStopsThePlaybackWhereItWas() async throws {
        let (rig, pip, system) = try await playing()
        system.say(.systemStarting)
        system.say(.started)
        system.say(.stopped)
        // As the player's own close: the file ended by quitting, which keeps the place, and the
        // player closed.
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
        #expect(!rig.model.isPresented)
        try await until("the feed stopped") { !pip.feed.isRunning }
    }

    @Test func theVideoTrackIsOffInTheBackgroundUnlessPictureInPictureShowsIt() async throws {
        let (rig, pip, system) = try await playing()
        // Home with no Picture in Picture: the audio plays on, the picture is not decoded.
        rig.model.backgrounded(true)
        try await until("the video off") { videoTracks(rig) == ["no"] }
        rig.model.backgrounded(false)
        try await until("the video on") { videoTracks(rig) == ["no", "auto"] }

        // Home while it plays, the system opening its window after the app said it had gone.
        rig.model.backgrounded(true)
        system.say(.systemStarting)
        try await until("the video on for the window") { videoTracks(rig) == ["no", "auto", "no", "auto"] }
        system.say(.started)
        // In the floating window, still in the background: on.
        system.say(.restore)
        #expect(videoTracks(rig).count == 4)
        // The window gone and the app still in the background: off again.
        system.say(.stopped)
        try await until("the video off") { videoTracks(rig).last == "no" && videoTracks(rig).count == 5 }
        rig.model.backgrounded(false)
        try await until("the video on") { videoTracks(rig).last == "auto" && videoTracks(rig).count == 6 }
        withExtendedLifetime(pip) {}
    }

    @Test func theWindowOpenedBeforeTheAppLeftKeepsTheVideoOn() async throws {
        let (rig, pip, system) = try await playing()
        rig.model.togglePictureInPicture()
        #expect(system.starts == 1)
        system.say(.started)
        rig.model.backgrounded(true)
        rig.model.backgrounded(false)
        // Something sent to mpv after them, so anything they sent is in by the time it is.
        rig.model.liftSubtitles(0.2, animated: false)
        try await until("the lift sent") {
            rig.mpv.calls.contains { if case .set(property: "sub-pos", _) = $0 { true } else { false } }
        }
        #expect(videoTracks(rig).isEmpty)
        withExtendedLifetime(pip) {}
    }
}
