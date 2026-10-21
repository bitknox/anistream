// The downloads' Live Activity on iPhone, kept in step with the download queue: one activity for
// every download, started when one starts, updated every few seconds and at each change of state,
// and ended a few minutes after the last one finishes.
//
// It shows only while the app itself runs the downloads. When a continued-processing task takes
// them over in the background, the system shows its own progress, and the activity ends, so the two
// never show together: whoever runs that task sets `systemShowsProgress`. If the app is suspended
// with neither, the activity goes stale and says the download continues when anistream is opened.

import AnistreamKit
import AnistreamWidgetData
import CoreGraphics
import Foundation
#if os(iOS) && canImport(ActivityKit)
import ActivityKit
#endif

/// Where the activity is shown: ActivityKit on iPhone, a fake in the tests.
@MainActor
public protocol DownloadsActivityHost: AnyObject {
    /// Whether an activity is up.
    var isShowing: Bool { get }
    /// Start one; false when the system does not allow it.
    func start(_ state: DownloadsActivityState, staleAt: Date) -> Bool
    func update(_ state: DownloadsActivityState, staleAt: Date)
    /// End it: at once, or showing `state` until `dismissAt`.
    func end(_ state: DownloadsActivityState?, dismissAt: Date?)
}

@MainActor
public final class DownloadsActivityFeed {
    /// How often progress is shown, at most; a change of state shows at once.
    public static let interval: Duration = .seconds(3)
    /// How long the finished state stays.
    public static let finishedFor: TimeInterval = 4 * 60
    /// How long an update stands before the activity reads as stale: the app was suspended.
    public static let staleAfter: TimeInterval = 30

    private let host: (any DownloadsActivityHost)?
    private let files: WidgetFiles?
    private let now: () -> Date
    private var shown: DownloadsActivityState?
    private var lastUpdate: Date?
    private var pending: Task<Void, Never>?
    /// The downloads in progress last time, by id, to tell which one just finished.
    private var inProgress: [Int64: DownloadRow] = [:]
    /// Covers fetched, by title.
    private var covers: [AnilistId: String] = [:]
    private var coverAsked: Set<AnilistId> = []

    /// Whether the system shows the downloads' progress itself, from a continued-processing task.
    /// The activity ends while it does.
    public var systemShowsProgress = false {
        didSet {
            guard systemShowsProgress != oldValue, systemShowsProgress else { return }
            endNow()
        }
    }

    public init(
        host: (any DownloadsActivityHost)? = DownloadsActivityFeed.systemHost(),
        files: WidgetFiles? = WidgetFiles.shared(),
        now: @escaping () -> Date = Date.init
    ) {
        self.host = host
        self.files = files
        self.now = now
    }

    /// The ActivityKit host on iPhone; none elsewhere.
    public static func systemHost() -> (any DownloadsActivityHost)? {
        #if os(iOS) && canImport(ActivityKit)
        return SystemDownloadsActivity()
        #else
        return nil
        #endif
    }

    /// What the activity's Pause and Resume do, as they run in the app.
    public func takeControls(pause: @escaping @MainActor () -> Void, resume: @escaping @MainActor () -> Void) {
        DownloadControls.shared.pause = pause
        DownloadControls.shared.resume = resume
    }

    /// The queue changed: show it.
    func downloadsChanged(_ model: DownloadsModel, titles: TitleCache, artwork: ArtworkStore?) {
        guard let host else { return }
        let rows = model.rows
        let current = rows.filter { $0.state == .active || $0.state == .queued || $0.state == .paused }
        let finished = rows.first { $0.state == .done && inProgress[$0.id] != nil }
        let started = !inProgress.isEmpty || rows.contains { $0.state == .active }
        inProgress = Dictionary(uniqueKeysWithValues: current.map { ($0.id, $0) })

        if let first = current.first(where: { $0.state == .active }) ?? current.first {
            guard !systemShowsProgress else { return }
            let state = Self.state(current: current, first: first, model: model, cover: cover(for: first, titles: titles, artwork: artwork))
            show(state, on: host, start: started)
        } else if let finished, host.isShowing {
            var state = Self.state(current: [finished], first: finished, model: model, cover: covers[finished.anilistId])
            state.finished = true
            state.downloadID = finished.id
            pending?.cancel()
            host.end(state, dismissAt: now().addingTimeInterval(Self.finishedFor))
            shown = nil
        } else if host.isShowing {
            endNow()
        }
    }

    /// What the activity shows for the downloads in `current`, about `first`.
    static func state(
        current: [DownloadRow], first: DownloadRow, model: DownloadsModel, cover: String?
    ) -> DownloadsActivityState {
        let speed = current.compactMap { model.rates[$0.id] }.reduce(0, +)
        let left = current.reduce(Int64(0)) { $0 + Int64($1.total > $1.downloaded ? $1.total - $1.downloaded : 0) }
        let paused = current.allSatisfy { $0.state == .paused }
        let several = current.count > 1
        let done = several ? current.reduce(Int64(0)) { $0 + Int64($1.downloaded) } : Int64(first.downloaded)
        let total = several ? current.reduce(Int64(0)) { $0 + Int64($1.total) } : Int64(first.total)
        let seconds: Int?
        if paused {
            seconds = nil
        } else if several {
            seconds = speed > 0 ? Int((Double(left) / speed).rounded(.up)) : nil
        } else {
            seconds = model.secondsLeft(first).map(Int.init)
        }
        return DownloadsActivityState(
            titleID: Int(first.anilistId), title: first.title, episode: first.episode, coverFile: cover,
            bytesDone: done, bytesTotal: total, speed: speed > 0 ? speed : nil, secondsLeft: seconds,
            count: current.count, bytesLeft: left, paused: paused)
    }

    private func show(_ state: DownloadsActivityState, on host: any DownloadsActivityHost, start: Bool) {
        let at = now()
        if !host.isShowing {
            guard start, host.start(state, staleAt: at.addingTimeInterval(Self.staleAfter)) else { return }
            shown = state
            lastUpdate = at
            return
        }
        let due = lastUpdate.map { at.timeIntervalSince($0) >= Self.seconds(Self.interval) } ?? true
        if shown.map(state.differsInKind) ?? true || due {
            pending?.cancel()
            pending = nil
            host.update(state, staleAt: at.addingTimeInterval(Self.staleAfter))
            shown = state
            lastUpdate = at
        } else {
            // Shown once the interval is up, unless something newer comes first.
            pending?.cancel()
            let wait = Self.seconds(Self.interval) - at.timeIntervalSince(lastUpdate ?? at)
            pending = Task { [weak self] in
                try? await Task.sleep(for: .seconds(max(0, wait)))
                guard !Task.isCancelled, let self, host.isShowing else { return }
                host.update(state, staleAt: self.now().addingTimeInterval(Self.staleAfter))
                self.shown = state
                self.lastUpdate = self.now()
            }
        }
    }

    private func endNow() {
        pending?.cancel()
        pending = nil
        shown = nil
        if host?.isShowing == true { host?.end(nil, dismissAt: nil) }
    }

    private static func seconds(_ duration: Duration) -> TimeInterval {
        Double(duration.components.seconds) + Double(duration.components.attoseconds) / 1e18
    }

    // MARK: The cover

    /// The cover's file for `row`'s title, fetching it the first time: nil until it is there.
    private func cover(for row: DownloadRow, titles: TitleCache, artwork: ArtworkStore?) -> String? {
        let id = row.anilistId
        if let name = covers[id] { return name }
        guard let files, let artwork, let url = titles[id]?.coverUrl, coverAsked.insert(id).inserted else { return nil }
        Task { [weak self] in
            let images = await artwork.images(for: [url], maxPixelSize: 400)
            guard let image = images[url], let jpeg = ArtThumbnail.jpeg(from: image, kind: .poster) else { return }
            let name = WidgetFiles.artName(for: url, kind: .poster)
            do {
                try FileManager.default.createDirectory(at: files.liveDirectory, withIntermediateDirectories: true)
                try jpeg.write(to: files.liveDirectory.appending(path: name), options: .atomic)
                self?.covers[id] = name
            } catch {}
        }
        return nil
    }
}

#if os(iOS) && canImport(ActivityKit)
/// ActivityKit's activity, with local updates only. It keeps the activity's id, and finds the
/// activity by it for each change, off the main actor.
@MainActor
final class SystemDownloadsActivity: DownloadsActivityHost {
    private var id: String?

    init() {
        // An activity a previous run left up says nothing true any more.
        Task.detached { await Self.endAll() }
    }

    var isShowing: Bool { id != nil }

    func start(_ state: DownloadsActivityState, staleAt: Date) -> Bool {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return false }
        do {
            let activity = try Activity.request(
                attributes: DownloadsAttributes(), content: ActivityContent(state: state, staleDate: staleAt),
                pushType: nil)
            id = activity.id
            return true
        } catch {
            AppLog.info("downloads", "no Live Activity: \(error.localizedDescription)")
            return false
        }
    }

    func update(_ state: DownloadsActivityState, staleAt: Date) {
        guard let id else { return }
        let content = ActivityContent(state: state, staleDate: staleAt)
        Task.detached { await Self.update(id, content) }
    }

    func end(_ state: DownloadsActivityState?, dismissAt: Date?) {
        guard let id else { return }
        self.id = nil
        let content = state.map { ActivityContent(state: $0, staleDate: nil) }
        Task.detached { await Self.end(id, content, dismissAt: dismissAt) }
    }

    nonisolated private static func activity(_ id: String) -> Activity<DownloadsAttributes>? {
        Activity<DownloadsAttributes>.activities.first { $0.id == id }
    }

    nonisolated private static func update(_ id: String, _ content: ActivityContent<DownloadsActivityState>) async {
        await activity(id)?.update(content)
    }

    nonisolated private static func end(
        _ id: String, _ content: ActivityContent<DownloadsActivityState>?, dismissAt: Date?
    ) async {
        await activity(id)?.end(content, dismissalPolicy: dismissAt.map { .after($0) } ?? .immediate)
    }

    nonisolated private static func endAll() async {
        for activity in Activity<DownloadsAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
}
#endif
