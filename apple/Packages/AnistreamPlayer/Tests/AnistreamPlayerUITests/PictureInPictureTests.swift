// Picture in Picture on the Mac: the steps between its states, and what they do to the player,
// on a scripted mpv and a stand-in for the system's floating window.

import AnistreamKit
import AnistreamPlayer
import Foundation
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

    @Test func eventsThatMeanNothingWhereItIsChangeNothing() {
        var machine = PictureInPictureMachine()
        for event: PictureInPictureEvent in [.started, .restore, .stopped, .playbackClosed, .failedToStart] {
            #expect(machine.handle(event) == [])
            #expect(machine.phase == .inline)
        }
    }
}

#if os(macOS)
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

    @Test func takesOverFromTheMiniPlayer() async throws {
        let (rig, pip, system) = try await playing()
        rig.model.miniPlayer = true
        rig.model.togglePictureInPicture()
        system.say(.started)
        #expect(!rig.model.miniPlayer)
        #expect(rig.model.pictureInPicture)
        #expect(!rig.model.inMainWindow)
        // The app holds it as long as the player's window; so does the test.
        withExtendedLifetime(pip) {}
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
#endif
