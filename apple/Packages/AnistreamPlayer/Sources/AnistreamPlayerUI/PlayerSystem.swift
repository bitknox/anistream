// What the player tells the system: Now Playing and the remote commands, the Live Activity, the
// display kept awake, the audio session, and the power the device runs on.

import AnistreamStore
import Foundation
import MediaPlayer

#if os(macOS)
import IOKit.ps
#else
import AVFAudio
import UIKit
#endif

/// `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`, set while a playback exists and cleared
/// when it stops. Timing is written on play, pause, seek, speed and episode changes, never on a
/// timer: the system runs the clock between. The Live Activity is told at the same moments, and
/// runs on its dates in the same way.
@MainActor
final class NowPlaying {
    weak var model: PlayerModel?
    private var targets: [(MPRemoteCommand, Any)] = []

    func update() {
        guard let model, model.isPresented else { return }
        if targets.isEmpty { bind() }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: model.episodeLine,
            MPMediaItemPropertyArtist: model.showTitle,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.video.rawValue,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: model.playhead,
            MPNowPlayingInfoPropertyPlaybackRate: model.paused ? 0.0 : model.speed,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if let duration = model.duration { info[MPMediaItemPropertyPlaybackDuration] = duration }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = info
        #if os(macOS)
        center.playbackState = model.paused ? .paused : .playing
        #endif
        model.onLiveActivity?(Self.playing(model))
    }

    /// What the Live Activity shows: once the episode's first picture is up and its length known.
    static func playing(_ model: PlayerModel) -> PlaybackActivityFeed.Playing? {
        guard model.hasPicture, let duration = model.duration, duration > 0,
              let id = model.staged?.id ?? model.starting?.id
        else { return nil }
        let episode = model.episode?.number ?? model.staged?.episode ?? model.starting?.episode ?? ""
        return PlaybackActivityFeed.Playing(
            id: id, title: model.showTitle, episode: episode, position: model.playhead,
            duration: duration, rate: model.speed, paused: model.paused)
    }

    func clear() {
        model?.onLiveActivity?(nil)
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = nil
        #if os(macOS)
        center.playbackState = .stopped
        #endif
        for (command, target) in targets {
            command.removeTarget(target)
            command.isEnabled = false
        }
        targets = []
    }

    private func bind() {
        let center = MPRemoteCommandCenter.shared()
        func on(_ command: MPRemoteCommand, _ action: @escaping @MainActor (PlayerModel, MPRemoteCommandEvent) -> Void) {
            command.isEnabled = true
            let target = command.addTarget { [weak self] event in
                MainActor.assumeIsolated {
                    guard let model = self?.model else { return .noActionableNowPlayingItem }
                    action(model, event)
                    return .success
                }
            }
            targets.append((command, target))
        }
        on(center.playCommand) { model, _ in if model.paused { model.togglePause() } }
        on(center.pauseCommand) { model, _ in if !model.paused { model.togglePause() } }
        on(center.togglePlayPauseCommand) { model, _ in model.togglePause() }
        on(center.changePlaybackPositionCommand) { model, event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return }
            model.seek(to: event.positionTime)
        }
        #if os(macOS)
        on(center.nextTrackCommand) { model, _ in model.step(1) }
        on(center.previousTrackCommand) { model, _ in model.step(-1) }
        #else
        center.skipForwardCommand.preferredIntervals = [10]
        center.skipBackwardCommand.preferredIntervals = [10]
        on(center.skipForwardCommand) { model, _ in model.seek(by: 10) }
        on(center.skipBackwardCommand) { model, _ in model.seek(by: -10) }
        #endif
    }
}

/// The display kept awake while playing, and let sleep when paused or closed.
@MainActor
final class KeepAwake {
    #if os(macOS)
    private var activity: NSObjectProtocol?
    #endif

    func update(playing: Bool) {
        #if os(macOS)
        if playing, activity == nil {
            activity = ProcessInfo.processInfo.beginActivity(
                options: [.idleDisplaySleepDisabled, .userInitiated], reason: "Playing an episode")
        } else if !playing, let activity {
            ProcessInfo.processInfo.endActivity(activity)
            self.activity = nil
        }
        #else
        UIApplication.shared.isIdleTimerDisabled = playing
        #endif
    }
}

/// The audio session on iPhone and iPad: movie playback, which plays on in the background.
@MainActor
enum PlaybackSession {
    static func begin() {
        #if os(iOS)
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .moviePlayback)
        try? session.setActive(true)
        #endif
    }

    static func end() {
        #if os(iOS)
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        #endif
    }
}

/// What the device runs on, for the engine's upscaling: a step lighter on battery and in low
/// power mode.
enum PowerState {
    @MainActor
    static func now() -> (onBattery: Bool, lowPower: Bool) {
        (onBattery(), ProcessInfo.processInfo.isLowPowerModeEnabled)
    }

    @MainActor
    private static func onBattery() -> Bool {
        #if os(macOS)
        guard let info = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
            let type = IOPSGetProvidingPowerSourceType(info)?.takeUnretainedValue()
        else { return false }
        return (type as String) == kIOPSBatteryPowerValue
        #else
        let device = UIDevice.current
        device.isBatteryMonitoringEnabled = true
        return device.batteryState == .unplugged
        #endif
    }

    /// Where the system says the power changed.
    static var notifications: [Notification.Name] {
        #if os(macOS)
        [Notification.Name.NSProcessInfoPowerStateDidChange]
        #else
        [Notification.Name.NSProcessInfoPowerStateDidChange, UIDevice.batteryStateDidChangeNotification]
        #endif
    }
}
