// What a title page's trailer does during one visit: wait, load, play, give way to the banner when
// the page scrolls away or the trailer ends, and let go.

import AnistreamKit
import Foundation
import Observation

/// What the trailer's page said, as the screens read it.
enum TrailerSignal: Equatable {
    /// A frame is on screen: the banner may fade away over it.
    case playing
    /// The player is up, or paused; nothing changes on screen.
    case quiet
    /// Half a second before the end: the banner comes back now.
    case ending
    case ended
    /// The player refused it, with YouTube's code; 0 is the page's own, the player API not loading.
    case failed(Int)
}

/// The web view a visit plays in, or a stand-in for it in a test.
@MainActor
protocol TrailerSurface: AnyObject {
    func control(_ control: TrailerControl)
    func tearDown()
}

/// One visit to a title page, or to a hover preview. The page asks `begin` once it has settled and
/// the engine allows a trailer; from then on the web view exists while `live` and the video is in
/// front of the banner while `shown`. The banner fades back before anything is paused or released,
/// so the player's own paused and end screens never show.
///
/// - The banner stays, silently, if nothing plays within `loadTimeout` or the player reports an
///   error.
/// - Scrolling away fades the banner back, pauses the video once it is covered, and releases the
///   view after `awayLimit`; scrolling back before that plays it again and fades it in once it
///   reports a picture.
/// - It ends once: the banner returns when the trailer is about to end, and it does not loop.
@MainActor @Observable
final class TrailerVisit {
    private enum Stage { case waiting, loading, playing, held, done }

    /// The web view, while there is one.
    private(set) var surface: (any TrailerSurface)?
    /// Whether the video is in front of the banner art.
    private(set) var shown = false
    /// Whether the sound is off, which it is at every start.
    private(set) var muted = true
    /// The player's last error, for the Play trailer window.
    private(set) var failure: Int?

    @ObservationIgnored private var stage = Stage.waiting
    @ObservationIgnored private var away = false
    @ObservationIgnored private var timer: Task<Void, Never>?
    @ObservationIgnored private var pause: Task<Void, Never>?
    /// Whether the banner is fading back and the pause has not landed.
    @ObservationIgnored private var pausing = false
    @ObservationIgnored private let loadTimeout: Duration
    @ObservationIgnored private let awayLimit: Duration
    @ObservationIgnored private let fade: Duration

    /// Whether a web view exists.
    var live: Bool { surface != nil }

    init(
        loadTimeout: Duration = .seconds(8), awayLimit: Duration = .seconds(30), fade: Duration = .milliseconds(300)
    ) {
        self.loadTimeout = loadTimeout
        self.awayLimit = awayLimit
        self.fade = fade
    }

    /// Start loading: the page has settled and trailers may play. Once per visit. `make` builds the
    /// web view and hands it the visit's signal.
    func begin(_ make: (@escaping @MainActor (TrailerSignal) -> Void) -> any TrailerSurface) {
        guard stage == .waiting else { return }
        stage = .loading
        muted = true
        surface = make { [weak self] signal in self?.signal(signal) }
        startTimeout()
    }

    private func startTimeout() {
        timer?.cancel()
        timer = Task { [loadTimeout] in
            try? await Task.sleep(for: loadTimeout)
            guard !Task.isCancelled else { return }
            if stage == .loading { finish() }
        }
    }

    /// The page scrolled past the point where the video would be under the page, or back to the top.
    func scrolledAway(_ value: Bool) {
        guard away != value else { return }
        away = value
        switch stage {
        case .playing where value:
            shown = false
            pausing = true
            pause = Task { [fade, awayLimit] in
                try? await Task.sleep(for: fade)
                guard !Task.isCancelled else { return }
                pausing = false
                surface?.control(.pause)
                stage = .held
                timer?.cancel()
                timer = Task {
                    try? await Task.sleep(for: awayLimit)
                    guard !Task.isCancelled else { return }
                    if stage == .held { finish() }
                }
            }
        case .playing where !value:
            // Back before the pause landed: the video is still playing, so it comes forward.
            if pausing {
                pause?.cancel()
                pausing = false
                shown = true
            }
        case .held where !value:
            timer?.cancel()
            stage = .loading
            surface?.control(.play)
            startTimeout()
        default: break
        }
    }

    /// What the page said.
    func signal(_ signal: TrailerSignal) {
        switch signal {
        case .playing:
            guard stage == .loading else { return }
            timer?.cancel()
            stage = .playing
            if away {
                // Scrolled away while it loaded: it plays under the page for a moment, then waits.
                away = false
                scrolledAway(true)
            } else {
                shown = true
            }
        case .ending, .ended:
            guard stage == .playing || stage == .loading else { return }
            shown = false
            pause?.cancel()
            pausing = false
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
        case .quiet:
            break
        }
    }

    /// Sound on or off for this trailer.
    func toggleSound() {
        muted.toggle()
        surface?.control(muted ? .mute : .unmute)
    }

    /// The page was left, a sheet or the player opened over it, the window lost key status, or the
    /// app went to the background: let go at once, if the trailer had started.
    func release() {
        if stage != .waiting { finish() }
    }

    private func finish() {
        timer?.cancel()
        pause?.cancel()
        stage = .done
        shown = false
        let surface = surface
        self.surface = nil
        surface?.tearDown()
    }
}
