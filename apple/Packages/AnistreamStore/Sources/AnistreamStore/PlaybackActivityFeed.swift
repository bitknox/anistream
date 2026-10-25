// The playback's Live Activity on iPhone, kept in step with the player: started once the episode
// shows its first picture, updated on play, pause, seek, speed and episode changes, and ended
// when the player closes. Never on a timer: the activity runs its progress and time left on the
// dates it is given. It stands beside the downloads' activity and leads it (see
// `LiveActivityRelevance`).

import AnistreamKit
import AnistreamWidgetData
import Foundation
#if os(iOS) && canImport(ActivityKit)
import ActivityKit
#endif

@MainActor
public final class PlaybackActivityFeed {
    /// What plays, as the player says it.
    public struct Playing: Equatable, Sendable {
        public var id: AnilistId
        public var title: String
        public var episode: String
        public var position: Double
        public var duration: Double
        public var rate: Double
        public var paused: Bool

        public init(id: AnilistId, title: String, episode: String, position: Double, duration: Double, rate: Double, paused: Bool) {
            self.id = id
            self.title = title
            self.episode = episode
            self.position = position
            self.duration = duration
            self.rate = rate
            self.paused = paused
        }
    }

    /// How long past the episode's end, at its speed, the activity reads as stale: the app was
    /// not there to say what came next.
    public static let staleAfterEnd: TimeInterval = 60

    private let host: (any LiveActivityHost<PlaybackActivityState>)?
    private let covers: LiveActivityCovers
    private let now: () -> Date
    private var shown: PlaybackActivityState?

    public init(
        host: (any LiveActivityHost<PlaybackActivityState>)? = PlaybackActivityFeed.systemHost(),
        files: WidgetFiles? = WidgetFiles.shared(),
        now: @escaping () -> Date = Date.init
    ) {
        self.host = host
        covers = LiveActivityCovers(files: files)
        self.now = now
    }

    /// The ActivityKit host on iPhone; none elsewhere.
    public static func systemHost() -> (any LiveActivityHost<PlaybackActivityState>)? {
        #if os(iOS) && canImport(ActivityKit)
        return SystemLiveActivity(PlaybackAttributes(), relevance: LiveActivityRelevance.playback, name: "player")
        #else
        return nil
        #endif
    }

    /// What the activity's Play and Pause do, as they run in the app.
    public func takeControls(play: @escaping @MainActor () -> Void, pause: @escaping @MainActor () -> Void) {
        PlaybackControls.shared.play = play
        PlaybackControls.shared.pause = pause
    }

    /// The player changed: show what plays, or end the activity when `playing` is nil.
    func show(_ playing: Playing?, titles: TitleCache, artwork: (any WidgetArtSource)?) {
        guard let host else { return }
        guard let playing, playing.duration > 0 else {
            shown = nil
            if host.isShowing { host.end(nil, dismissAt: nil) }
            return
        }
        let cover = covers.file(for: playing.id, url: titles[playing.id]?.coverUrl, artwork: artwork) { [weak self] name in
            // The cover came after the activity started: shown as it stands, with the cover.
            guard let self, let host = self.host, host.isShowing, var state = self.shown,
                  state.titleID == Int(playing.id), state.coverFile == nil else { return }
            state.coverFile = name
            host.update(state, staleAt: Self.staleAt(state))
            self.shown = state
        }
        // The title as the app's screens show it, in the form Settings › Title language picks.
        let title = titles[playing.id]?.title ?? playing.title
        let state = PlaybackActivityState(
            titleID: Int(playing.id), title: title, episode: playing.episode, coverFile: cover,
            position: playing.position, duration: playing.duration, at: now(), rate: playing.rate,
            paused: playing.paused)
        let staleAt = Self.staleAt(state)
        if !host.isShowing {
            if host.start(state, staleAt: staleAt) { shown = state }
            return
        }
        guard shown.map({ state.differs(from: $0) }) ?? true else { return }
        host.update(state, staleAt: staleAt)
        shown = state
    }

    /// When `state` reads as stale: a while after the episode ends while it plays; never while
    /// paused.
    static func staleAt(_ state: PlaybackActivityState) -> Date? {
        state.paused ? nil : state.timeline.upperBound.addingTimeInterval(staleAfterEnd)
    }
}

extension AppStore {
    /// The player changed: the playback's Live Activity shows what plays, or ends with `nil`.
    public func playbackChanged(_ playing: PlaybackActivityFeed.Playing?) {
        playbackActivity.show(playing, titles: titles, artwork: artwork)
    }
}
