// The app's state: the engine's events applied, each by the feature model that owns it.

import AnistreamKit
import Foundation
import Observation

/// Where the app's state lives, on the main actor.
///
/// The engine's events arrive in batches from `EngineClient`, and `apply(_:)` hands each one to
/// the model that owns it: the lists, the title cache, jobs, notices, settings, accounts and
/// artwork. Views read the models; commands go out through `send(_:)`.
@Observable @MainActor
public final class AppStore {
    /// Where the engine stands.
    public enum Phase: Equatable, Sendable {
        case starting
        case running
        case stopped
        /// It did not start, and why, in the engine's words.
        case failed(String)
    }

    public private(set) var phase: Phase = .starting

    public let titles = TitleCache()
    public let continuing = ContinueModel()
    public let seasonal = ListModel()
    public let calendar = CalendarModel()
    public let search = SearchModel()
    public let library = LibraryModel()
    public let jobs: JobsModel
    public let notices: NoticesModel
    public let settings = SettingsModel()
    public let accounts = AccountsModel()
    @ObservationIgnored public private(set) var artwork: ArtworkStore!

    @ObservationIgnored private var client: EngineClient?
    /// When a batch was last applied.
    @ObservationIgnored private var lastApplied = ContinuousClock.now
    /// What the screens asked for before the engine was there, sent once it is.
    @ObservationIgnored private var early: [Command] = []
    /// Called after each batch is applied: what a screenshot run waits on.
    @ObservationIgnored public var onApplied: (@MainActor ([Event]) -> Void)?

    public init(jobs: JobsModel = JobsModel(), notices: NoticesModel = NoticesModel()) {
        self.jobs = jobs
        self.notices = notices
        artwork = ArtworkStore { [weak self] url in
            guard let client = self?.client else { return false }
            client.send(.fetchArtwork(url: url))
            return true
        }
    }

    // MARK: The engine

    /// Start the engine `launch` describes with `host`, off the main thread, and read its events.
    public func start(_ launch: EngineLaunch, host: any FfiHost) async {
        phase = .starting
        do {
            let engine = try await Task.detached(priority: .userInitiated) {
                try launch.start(host: host)
            }.value
            attach(engine)
        } catch let error as FfiError {
            phase = .failed(error.reason)
            early = []
        } catch {
            phase = .failed(error.localizedDescription)
            early = []
        }
    }

    /// Read `engine`'s events from now on.
    public func attach(_ engine: FfiEngine) {
        client = EngineClient(
            engine: engine,
            deliver: { [weak self] events in self?.apply(events) },
            stopped: { [weak self] in self?.phase = .stopped }
        )
        phase = .running
        for command in early {
            client?.send(command)
        }
        early = []
    }

    /// Hand the engine a command. `nil` for work nobody waits on, and when there is no engine:
    /// while it starts, the command waits and is sent once it has.
    @discardableResult
    public func send(_ command: Command) -> JobId? {
        guard let client else {
            if phase == .starting { early.append(command) }
            return nil
        }
        return client.send(command)
    }

    /// Stop the engine, waiting while it winds down: at quit.
    public func shutdown() {
        client?.shutdown()
    }

    // MARK: Asking

    public func loadContinue() {
        continuing.list.asked()
        send(.loadContinue)
    }

    public func loadSeasonal() {
        seasonal.asked()
        send(.loadSeasonal)
    }

    public func loadCalendar() {
        calendar.list.asked()
        send(.loadCalendar)
    }

    public func search(_ query: String, filter: SearchFilters = SearchFilters()) {
        search.submit(query: query, filter: filter)
        send(.search(query: query, filter: filter))
    }

    public func loadLibrary(_ segment: LibrarySegment) {
        library.segment(segment).asked()
        send(.loadLibrary(segment))
    }

    public func loadDetail(_ id: AnilistId) {
        send(.loadDetail(id))
    }

    public func loadEpisodes(_ id: AnilistId) {
        send(.loadEpisodes(id))
    }

    /// Take a title off Continue: the row goes at once, and the engine keeps it hidden.
    public func hideFromContinue(_ id: AnilistId) {
        continuing.hide(id)
        send(.hideFromContinue(id: id))
    }

    // MARK: Applying

    public func apply(_ events: [Event]) {
        for event in events {
            apply(event)
        }
        lastApplied = .now
        onApplied?(events)
    }

    // MARK: Settling

    /// Whether the screens have what they asked for: the engine has started or failed, no list
    /// is loading, no job is running, and no art is on its way.
    public var isSettled: Bool {
        let lists = [continuing.list, seasonal, calendar.list, search.list]
            + LibraryModel.order.map(library.segment)
        return phase != .starting
            && !lists.contains { $0.state == .loading }
            && jobs.running.isEmpty
            && !artwork.hasPending
    }

    /// Wait until the store is settled and nothing has been applied for `quiet`: what a
    /// screenshot waits on. False when `timeout` passed first.
    public func settle(
        quiet: Duration = .milliseconds(800),
        timeout: Duration = .seconds(30)
    ) async -> Bool {
        let deadline = ContinuousClock.now + timeout
        while ContinuousClock.now < deadline {
            if isSettled, ContinuousClock.now - lastApplied >= quiet { return true }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return false
    }

    public func apply(_ event: Event) {
        switch event {
        case .jobStarted(let job, let activity):
            jobs.started(job, activity: activity)
        case .jobNote(let job, let note, let wait):
            jobs.note(job, note: note, wait: wait)
        case .jobDone(let job):
            jobs.done(job)

        case .content(let view, let content):
            self.content(content, for: view)
        case .caption(let view, let text):
            caption(text, for: view)
        case .calendarWindow(let anchor, let pages, let morePast):
            calendar.window(anchor: anchor, pages: pages, morePast: morePast)
        case .calendarBackfill(let page, let entries, let morePast):
            calendar.backfill(page: page, entries: entries, morePast: morePast, titles: titles)
        case .detail(let entry):
            titles.merge(detail: entry)
        case .progress(let id, let progress, let resume, let episodes):
            titles.progress(id: id, progress: progress, resume: resume, episodes: episodes)
        case .lastAired(let rows):
            titles.lastAired(rows)
        case .episodes(let id, let rows):
            titles.episodes(id: id, rows: rows)

        case .status(let text):
            notices.status(text)
        case .toast(let notice):
            notices.post(notice)

        case .settings(let view):
            settings.apply(view)

        case .sync(let state):
            accounts.apply(state)
        case .progressQueued:
            accounts.progressQueued()
        case .conflicts(let rows):
            accounts.conflicts(rows)
        case .deviceCode(let prompt):
            accounts.deviceCode(prompt)

        case .artworkReady(let url, let bytes):
            artwork.ready(url: url, bytes: bytes)
        case .imageFailed(let url, _):
            artwork.failed(url: url)

        default:
            // Playback, sources, providers, downloads and plugin stores have no screen yet.
            break
        }
    }

    private func content(_ content: ContentList, for view: ContentFor) {
        switch view {
        case .continue:
            continuing.apply(content, titles: titles)
        case .seasonal:
            seasonal.apply(content, titles: titles)
        case .calendar:
            calendar.apply(content, titles: titles)
        case .search(let query, let filter):
            guard search.takes(query: query, filter: filter) else { return }
            search.list.apply(content, titles: titles)
        case .library(let segment):
            library.segment(segment).apply(content, titles: titles)
        }
    }

    private func caption(_ text: String, for view: ContentFor) {
        switch view {
        case .continue: continuing.list.caption = text
        case .seasonal: seasonal.caption = text
        case .calendar: calendar.list.caption = text
        case .search(let query, let filter):
            if search.takes(query: query, filter: filter) { search.list.caption = text }
        case .library(let segment): library.segment(segment).caption = text
        }
    }
}
