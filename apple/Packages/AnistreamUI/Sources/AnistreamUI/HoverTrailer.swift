// The hover card's trailer: one at a time, for whichever card is open, through the trailer layer.

#if os(macOS)
import AnistreamKit
import AnistreamStore
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
    /// A cue that came while a closing card's trailer was still fading, for once it has gone.
    @ObservationIgnored private var queued: (id: AnilistId, start: () -> Void)?

    /// Whether the video is over the art.
    var shown: Bool { visit.shown }
    var muted: Bool { visit.muted }
    /// Whether the player is drawing captions: they follow the sound.
    var captions: Bool { visit.captions }

    /// The warm page while it is primed: hosted unseen by the hover layer until it plays.
    private(set) var priming: TrailerPage?
    @ObservationIgnored private var primed = false
    @ObservationIgnored private var primeTimeout: Task<Void, Never>?

    /// Load the player once a run, with no title, then prime it with `first`, the first cover on
    /// screen with a trailer: its video loads and plays a moment, hidden and muted, and the page
    /// goes back to the pool paused. The first hover then starts as fast as later ones, which
    /// it does not on a page that has played nothing. The caller picks a still moment.
    func warmUp(subtitles: String, first: Trailer?) async {
        await TrailerPool.shared.prewarm(subtitles: subtitles)
        guard !primed, id == nil, priming == nil, let first else { return }
        primed = true
        guard let page = TrailerPool.shared.takeWarm(first, subtitles: subtitles) else { return }
        let began = ContinuousClock.now
        page.hear = { [weak self] text in
            let word = text.trimmingCharacters(in: .whitespaces)
            if word == "playing" || word.hasPrefix("error:") { // i18n-skip: the page's words
                AppLog.debug("trailer", "primed with \(first.id): \(word) after \(AppLog.ms(since: began)) ms")
                self?.endPrime()
            }
        }
        priming = page
        primeTimeout = Task { [weak self] in
            try? await Task.sleep(for: .seconds(8))
            guard !Task.isCancelled else { return }
            AppLog.debug("trailer", "priming with \(first.id) did not play in 8 s")
            self?.endPrime()
        }
    }

    /// The priming is over, or a hover wants the page: it goes back to the pool, paused, after
    /// saying what it fetched.
    private func endPrime() {
        primeTimeout?.cancel()
        guard let page = priming else { return }
        priming = nil
        page.hear = nil
        page.run(trailerBytesScript())
        TrailerPool.shared.give(page)
    }

    /// The pointer has slowed on `entry`'s cover, 300 ms before its card opens: the warm page
    /// starts loading its trailer then, silent and unseen, so the card opens on a playing video.
    /// The pointer moving on lets it go (`uncue`); a pass across a row cues nothing.
    func cue(_ entry: Entry, subtitles: String) {
        guard entry.id != id || !visit.live, let trailer = entry.trailer else { return }
        if pending != nil {
            // The last card's trailer is still fading with it: this one starts once it has gone.
            queued = (entry.id, { [weak self] in self?.cue(entry, subtitles: subtitles) })
            return
        }
        start(entry.id, trailer, subtitles: subtitles, held: true)
    }

    /// The pointer left the cued cover before its card opened.
    func uncue(_ id: AnilistId) {
        if queued?.id == id { queued = nil }
        guard id == self.id, visit.held, pending == nil else { return }
        stop()
    }

    /// The card for `entry` opened or was handed to it: play its trailer under the art, with the
    /// sound as the person last left it. A cued trailer carries on where it is.
    func play(_ entry: Entry, subtitles: String) {
        pending?.cancel()
        pending = nil
        queued = nil
        if entry.id == id, visit.live {
            visit.unhold()
            return
        }
        guard let trailer = entry.trailer else {
            stop()
            return
        }
        start(entry.id, trailer, subtitles: subtitles, held: false)
    }

    private func start(_ id: AnilistId, _ trailer: Trailer, subtitles: String, held: Bool) {
        stop()
        endPrime()
        self.id = id
        let visit = TrailerVisit(loadTimeout: Self.giveUp)
        self.visit = visit
        visit.begin(held: held) { signal in
            TrailerPlayer(trailer: trailer, style: .banner, subtitles: subtitles, onSignal: signal)
        }
    }

    /// The card is going: the player stops once the video has faded with it, so its paused
    /// screen never shows.
    func stopSoon() {
        pending?.cancel()
        pending = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(240))
            guard !Task.isCancelled, let self else { return }
            let next = self.queued
            self.stop()
            next?.start()
        }
    }

    /// Stop now: nothing shows it any more.
    func stop() {
        pending?.cancel()
        pending = nil
        queued = nil
        visit.release()
        id = nil
    }

    func toggleSound() { visit.toggleSound() }
}
#endif
