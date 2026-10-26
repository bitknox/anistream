// Now Playing and the media keys for the TV this Mac drives: while the TV plays something and the
// Mac's own player is down, the system's Now Playing shows the TV's episode, and play, pause,
// the playhead and the next and previous episode go to the TV, as the card's controls do.

#if os(macOS)
import AnistreamKit
import AnistreamStore
import Foundation
import MediaPlayer
import Observation

@MainActor
public final class TVNowPlaying {
    private let tv: TVRemoteModel
    private weak var player: PlayerModel?
    private var targets: [(MPRemoteCommand, Any)] = []

    public init(tv: TVRemoteModel, player: PlayerModel) {
        self.tv = tv
        self.player = player
        watch()
    }

    /// Follows the card and the player, here rather than in a view: the media keys work with the
    /// window closed.
    private func watch() {
        withObservationTracking {
            update()
        } onChange: { [weak self] in
            Task { @MainActor in self?.watch() }
        }
    }

    private func update() {
        let playerUp = player?.isPresented ?? false
        guard !playerUp, tv.connected != nil, let card = tv.card, let now = tv.now else {
            release(playerUp: playerUp)
            return
        }
        if targets.isEmpty { bind() }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = Self.info(now, tvName: card.tvName, playhead: tv.playhead() ?? now.position, live: tv.live)
        center.playbackState = Self.playing(now, live: tv.live) ? .playing : .paused
    }

    /// Whether the TV's playhead runs: it plays, and the link is up to say so.
    static func playing(_ now: FfiNowCard, live: Bool) -> Bool {
        live && !now.paused && !now.loading
    }

    /// What Now Playing shows of the TV's episode. The system runs the clock from the playhead
    /// at the rate given, so it is written as the card changes, never on a timer.
    static func info(_ now: FfiNowCard, tvName: String, playhead: Double, live: Bool) -> [String: Any] {
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: now.episodeLine,
            MPMediaItemPropertyArtist: now.title,
            MPMediaItemPropertyAlbumTitle: "Playing on \(tvName)",
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.video.rawValue,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: playhead,
            MPNowPlayingInfoPropertyPlaybackRate: playing(now, live: live) ? now.speed : 0.0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if let duration = now.duration { info[MPMediaItemPropertyPlaybackDuration] = duration }
        return info
    }

    private func bind() {
        let center = MPRemoteCommandCenter.shared()
        func on(_ command: MPRemoteCommand, _ action: @escaping @MainActor (TVRemoteModel, FfiNowCard, MPRemoteCommandEvent) -> Void) {
            command.isEnabled = true
            let target = command.addTarget { [weak self] event in
                MainActor.assumeIsolated {
                    guard let tv = self?.tv, let now = tv.now else { return .noActionableNowPlayingItem }
                    guard tv.live else { return .commandFailed }
                    action(tv, now, event)
                    return .success
                }
            }
            targets.append((command, target))
        }
        on(center.playCommand) { tv, now, _ in if now.paused { tv.send(.togglePause) } }
        on(center.pauseCommand) { tv, now, _ in if !now.paused { tv.send(.togglePause) } }
        on(center.togglePlayPauseCommand) { tv, _, _ in tv.send(.togglePause) }
        on(center.changePlaybackPositionCommand) { tv, _, event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return }
            tv.send(.seekTo(position: event.positionTime))
        }
        on(center.nextTrackCommand) { tv, _, _ in tv.send(.next) }
        on(center.previousTrackCommand) { tv, _, _ in tv.send(.previous) }
    }

    /// Let Now Playing go. With the Mac's player up, Now Playing and the commands are its own:
    /// only this one's targets go, and the rest is left to it.
    private func release(playerUp: Bool) {
        guard !targets.isEmpty else { return }
        for (command, target) in targets {
            command.removeTarget(target)
            if !playerUp { command.isEnabled = false }
        }
        targets = []
        guard !playerUp else { return }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = nil
        center.playbackState = .stopped
    }
}
#endif
