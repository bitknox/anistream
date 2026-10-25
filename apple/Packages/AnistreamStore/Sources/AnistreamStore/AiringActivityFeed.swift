// The airing Live Activity on iPhone: an episode of a show you follow that airs within the hour,
// counting down, then "Out now" for half an hour. It comes from the week the engine says for
// notifications (`UpcomingNotices`): a show being watched or switched on for itself, not one
// switched off, and not one watched dubbed, whose broadcast time is not the dub's.
//
// The countdown runs on the air time, and the activity's stale date is that time, so it turns to
// "Out now" while the app is away. While the app runs, it says so itself at the air time, and
// ends the activity half an hour on. Starting one needs the app in front, which is when the week
// is asked for.

import AnistreamKit
import AnistreamWidgetData
import Foundation
#if os(iOS) && canImport(ActivityKit)
import ActivityKit
#endif

@MainActor
public final class AiringActivityFeed {
    /// How soon an episode airs for the activity to start.
    public nonisolated static let ahead: TimeInterval = 60 * 60
    /// How long it says "Out now".
    public nonisolated static let outFor: TimeInterval = 30 * 60

    private let host: (any LiveActivityHost<AiringActivityState>)?
    private let covers: LiveActivityCovers
    private let now: () -> Date
    private var shown: AiringActivityState?
    private var week: UpcomingNotices?
    private var artwork: (any WidgetArtSource)?
    private var titles: TitleCache?
    /// The next look, at the next time something changes: an air time, or the end of "Out now".
    private var wake: Task<Void, Never>?

    public init(
        host: (any LiveActivityHost<AiringActivityState>)? = AiringActivityFeed.systemHost(),
        files: WidgetFiles? = WidgetFiles.shared(),
        now: @escaping () -> Date = Date.init
    ) {
        self.host = host
        covers = LiveActivityCovers(files: files)
        self.now = now
    }

    /// The ActivityKit host on iPhone; none elsewhere.
    public static func systemHost() -> (any LiveActivityHost<AiringActivityState>)? {
        #if os(iOS) && canImport(ActivityKit)
        return SystemLiveActivity(AiringAttributes(), relevance: LiveActivityRelevance.airing, name: "airing")
        #else
        return nil
        #endif
    }

    /// Whether the activity can show here, so the week is worth asking for.
    public var wantsWeek: Bool { host?.isEnabled ?? false }

    /// The week the engine said.
    func weekChanged(_ week: UpcomingNotices, titles: TitleCache? = nil, artwork: (any WidgetArtSource)?) {
        self.week = week
        self.titles = titles
        self.artwork = artwork
        look()
    }

    /// The episode to show at `now`: of a show followed, not dubbed, airing within the hour or
    /// out within the half hour, the soonest.
    nonisolated static func pick(_ week: UpcomingNotices, now: Date) -> UpcomingNotice? {
        week.airings
            .filter { $0.reason != .switchedOff && $0.translation != .dub }
            .filter {
                let airs = Date(timeIntervalSince1970: TimeInterval($0.airsAt))
                return airs > now.addingTimeInterval(-outFor) && airs <= now.addingTimeInterval(ahead)
            }
            .min { ($0.airsAt, $0.id, $0.episode) < ($1.airsAt, $1.id, $1.episode) }
    }

    /// Show what the week has now, and look again when that changes.
    func look() {
        wake?.cancel()
        wake = nil
        guard let host else { return }
        let at = now()
        var target = week.flatMap { Self.pick($0, now: at) }.map { row in
            AiringActivityState(
                // The title as the app's screens show it; the week's is in the same form, but
                // may be older than a change to Settings › Title language.
                titleID: Int(row.id), title: titles?[row.id]?.title ?? row.title, episode: Int(row.episode),
                airsAt: Date(timeIntervalSince1970: TimeInterval(row.airsAt)), since: at)
        }
        // A week asked for after the air time no longer has the episode: the one up stays out
        // for its half hour.
        if target == nil, let shown, shown.airsAt <= at, at < shown.airsAt.addingTimeInterval(Self.outFor) {
            target = shown
        }
        guard var target else {
            if host.isShowing { host.end(nil, dismissAt: nil) }
            shown = nil
            schedule(at)
            return
        }
        if let shown, shown.isEpisode(of: target) {
            target.since = shown.since
            target.coverFile = shown.coverFile
        } else if host.isShowing {
            host.end(nil, dismissAt: nil)
            shown = nil
        }
        if target.coverFile == nil {
            let url = week?.airings.first { Int($0.id) == target.titleID }?.coverUrl
            target.coverFile = covers.file(for: AnilistId(target.titleID), url: url, artwork: artwork) { [weak self] name in
                guard let self, let host = self.host, host.isShowing, var state = self.shown,
                      state.titleID == target.titleID, state.coverFile == nil else { return }
                state.coverFile = name
                host.update(state, staleAt: state.out ? nil : state.airsAt)
                self.shown = state
            }
        }
        target.out = target.airsAt <= at
        let staleAt = target.out ? nil : target.airsAt
        if !host.isShowing {
            if host.start(target, staleAt: staleAt) { shown = target }
        } else if shown != target {
            host.update(target, staleAt: staleAt)
            shown = target
        }
        schedule(at)
    }

    /// Look again at the next time something changes, while the app runs: the episode shown
    /// airs, its half hour ends, or the next one comes within the hour.
    private func schedule(_ at: Date) {
        var times: [Date] = []
        if let shown {
            times.append(shown.out ? shown.airsAt.addingTimeInterval(Self.outFor) : shown.airsAt)
        }
        let next = (week?.airings ?? [])
            .filter { $0.reason != .switchedOff && $0.translation != .dub }
            .map { Date(timeIntervalSince1970: TimeInterval($0.airsAt)).addingTimeInterval(-Self.ahead) }
            .filter { $0 > at }
            .min()
        if let next { times.append(next) }
        guard let when = times.filter({ $0 > at }).min() else { return }
        let wait = when.timeIntervalSince(at)
        wake = Task { [weak self] in
            try? await Task.sleep(for: .seconds(wait))
            guard !Task.isCancelled else { return }
            self?.look()
        }
    }
}
