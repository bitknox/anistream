// The hover card's trailer: one at a time, for whichever card is open, through the trailer layer.

#if os(macOS)
import AnistreamKit
import Foundation
import Observation

/// What the hover card asks of the trailer layer: warm up, cue a title while the pointer settles,
/// play the open card's, and stop once the card has faded.
@MainActor @Observable
final class HoverTrailer {
    static let shared = HoverTrailer()

    /// The title whose trailer is playing, or loading.
    private(set) var id: AnilistId?
    /// The visit it plays in.
    private(set) var visit = TrailerVisit(loadTimeout: HoverTrailer.giveUp)

    /// With no frame 4 s after play, it stops: the art stays, with no spinner and no toggle.
    static let giveUp: Duration = .seconds(4)

    @ObservationIgnored private var pending: Task<Void, Never>?

    /// Whether the video is over the art.
    var shown: Bool { visit.shown }
    var muted: Bool { visit.muted }
    /// Whether the player is drawing captions: they follow the sound.
    var captions: Bool { visit.captions }

    /// Load the player once, with no title, as a grid that can show cards appears: the pool's
    /// warm page, which then only switches video.
    func prewarm(subtitles: String) {
        TrailerPool.shared.prewarm(subtitles: subtitles)
    }

    /// The pointer has slowed on `entry`'s cover. The warm page switches video in place once the
    /// card opens, so nothing is asked of YouTube before then.
    func cue(_ entry: Entry) {}

    /// The card for `entry` opened or was handed to it: play its trailer under the art, with the
    /// sound as the person last left it.
    func play(_ entry: Entry, subtitles: String) {
        pending?.cancel()
        guard entry.id != id || !visit.live else { return }
        stop()
        guard let trailer = entry.trailer else { return }
        id = entry.id
        let visit = TrailerVisit(loadTimeout: Self.giveUp)
        self.visit = visit
        visit.begin { signal in
            TrailerPlayer(trailer: trailer, style: .banner, subtitles: subtitles, onSignal: signal)
        }
    }

    /// The card is going: the player stops once the video has faded with it, so its paused
    /// screen never shows.
    func stopSoon() {
        pending?.cancel()
        pending = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(240))
            guard !Task.isCancelled else { return }
            self?.stop()
        }
    }

    /// Stop now: nothing shows it any more.
    func stop() {
        pending?.cancel()
        visit.release()
        id = nil
    }

    func toggleSound() { visit.toggleSound() }
}
#endif
