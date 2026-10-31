// What a hover preview's trailer does during one visit: wait, load, play, give way to the art when
// the trailer ends, and let go. Also what lasts the whole run: the sound preference, and whether
// YouTube is asked at all.

import AnistreamKit
import Foundation
import Observation

/// What the trailer's page said, as the screens read it.
enum TrailerSignal: Equatable {
    /// The player plays and its clock has moved: the art may fade away over it.
    case playing
    /// The player is up, or paused; nothing changes on screen.
    case quiet
    /// A second before the end: the art comes back now.
    case ending
    case ended
    /// The player refused it, with YouTube's code; 0 is the page's own, the player API not loading.
    case failed(Int)
    /// Captions came on or went off.
    case captions(Bool)
}

/// The web view a visit plays in, or a stand-in for it in a test.
@MainActor
protocol TrailerSurface: AnyObject {
    func control(_ control: TrailerControl)
    func tearDown()
    /// The visit stopped waiting `after` the start with nothing played: say so in the log.
    func gaveUp(after: Duration)
}

extension TrailerSurface {
    func gaveUp(after: Duration) {}
}

/// Whether trailers start with their sound on: the person's last choice, kept on this device.
@MainActor
final class TrailerSound {
    static let shared = TrailerSound(defaults: .standard)
    static let key = "trailerSoundOn" // i18n-skip: a defaults key

    private let defaults: UserDefaults

    init(defaults: UserDefaults) {
        self.defaults = defaults
    }

    var on: Bool {
        get { defaults.bool(forKey: Self.key) }
        set { defaults.set(newValue, forKey: Self.key) }
    }
}

/// Whether the app asks YouTube for trailers, for the rest of the run.
///
/// A video YouTube refuses is never asked for again. Two refusals of different videos in a row,
/// with nothing played between them, stop trailers for the run: YouTube is refusing the network
/// (its sign-in check on a flagged VPN exit, which also says 150) or the app's identity, not one
/// owner's video. One refusal alone does not, because 101 and 150 are also what a single video
/// whose owner turned embedding off says, and that must not end every other trailer.
@MainActor
final class TrailerGate {
    static let shared = TrailerGate()

    /// YouTube's refusals: 101 and 150 embedding not allowed, or the sign-in check; 152 and 153
    /// the embed's identity refused or missing. 2, 5 and 100 are the one video's, and 0 the page's.
    static let refusals: Set<Int> = [101, 150, 152, 153]
    /// Refusals of different videos in a row that stop trailers for the run.
    static let limit = 2

    private(set) var closed = false
    private var refused: Set<String> = []
    private var streak = 0

    /// Whether to ask YouTube for this video.
    func mayAsk(_ id: String) -> Bool { !closed && !refused.contains(id) }

    /// A trailer played: the network and the app are fine.
    func played() { streak = 0 }

    /// The player failed with `code` for video `id`. True when that stops trailers for the run.
    @discardableResult
    func failed(_ id: String, code: Int) -> Bool {
        guard Self.refusals.contains(code), !closed, refused.insert(id).inserted else { return false }
        streak += 1
        closed = streak >= Self.limit
        return closed
    }
}

/// One visit to a hover preview. The preview asks `begin` once it has settled and the engine allows
/// a trailer; from then on the web view exists while `live` and the video is in front of the art
/// while `shown`. The art fades back before anything is released, so the player's own end screen
/// never shows.
///
/// - The art stays, silently, if the video is not playing within `loadTimeout` or the player
///   reports an error. Each is logged.
/// - It ends once: the art returns when the trailer is about to end, and it does not loop.
@MainActor @Observable
final class TrailerVisit {
    private enum Stage { case waiting, loading, playing, done }

    /// The web view, while there is one.
    private(set) var surface: (any TrailerSurface)?
    /// Whether the video is in front of the art.
    private(set) var shown = false
    /// Whether the sound is off: as the person last left it, at every start.
    private(set) var muted = true
    /// The player's last error, for the Play trailer window.
    private(set) var failure: Int?
    /// Whether captions are showing: only with the sound on, in a preview.
    private(set) var captions = false
    /// Whether it loads ahead of being seen, as a cued hover card's does: silent, whatever the
    /// sound preference, until `unhold`.
    private(set) var held = false

    @ObservationIgnored private var stage = Stage.waiting
    @ObservationIgnored private var timer: Task<Void, Never>?
    @ObservationIgnored private let loadTimeout: Duration
    @ObservationIgnored private var begun = ContinuousClock.now
    @ObservationIgnored private let fade: Duration
    @ObservationIgnored private let sound: TrailerSound

    /// Whether a web view exists.
    var live: Bool { surface != nil }

    init(
        loadTimeout: Duration = .seconds(8), fade: Duration = .milliseconds(300),
        sound: TrailerSound = .shared
    ) {
        self.loadTimeout = loadTimeout
        self.fade = fade
        self.sound = sound
    }

    /// Start loading: the preview has settled and trailers may play. Once per visit. `make` builds
    /// the web view and hands it the visit's signal.
    /// With `held`, it stays silent until `unhold`.
    func begin(held: Bool = false, _ make: (@escaping @MainActor (TrailerSignal) -> Void) -> any TrailerSurface) {
        guard stage == .waiting else { return }
        stage = .loading
        begun = .now
        muted = !sound.on
        captions = false
        self.held = held
        let surface = make { [weak self] signal in self?.signal(signal) }
        self.surface = surface
        // The page starts muted, as autoplay needs; with the sound on it comes up as it plays.
        if !muted, !held { surface.control(.unmute) }
        timer = Task { [weak self, loadTimeout] in
            try? await Task.sleep(for: loadTimeout)
            guard !Task.isCancelled, let self, self.stage == .loading else { return }
            self.surface?.gaveUp(after: ContinuousClock.now - self.begun)
            self.finish()
        }
    }

    /// What the page said.
    func signal(_ signal: TrailerSignal) {
        switch signal {
        case .playing:
            guard stage == .loading else { return }
            timer?.cancel()
            stage = .playing
            shown = true
        case .ending, .ended:
            guard stage == .playing || stage == .loading else { return }
            shown = false
            timer?.cancel()
            stage = .done
            timer = Task { [fade] in
                try? await Task.sleep(for: fade)
                guard !Task.isCancelled else { return }
                finish()
            }
        case .failed(let code):
            failure = code
            if stage != .done { finish() }
        case .captions(let on):
            captions = on && surface != nil
        case .quiet:
            break
        }
    }

    /// It is being seen now: the sound comes up if the person left it on.
    func unhold() {
        guard held else { return }
        held = false
        if !muted { surface?.control(.unmute) }
    }

    /// Sound on or off, for this trailer and the ones after it.
    func toggleSound() {
        muted.toggle()
        sound.on = !muted
        if !held { surface?.control(muted ? .mute : .unmute) }
    }

    /// The preview closed, a sheet or the player opened over it, or the app went to the background:
    /// let go at once, if the trailer had started.
    func release() {
        if stage != .waiting { finish() }
    }

    private func finish() {
        timer?.cancel()
        stage = .done
        shown = false
        captions = false
        let surface = surface
        self.surface = nil
        surface?.tearDown()
    }
}
