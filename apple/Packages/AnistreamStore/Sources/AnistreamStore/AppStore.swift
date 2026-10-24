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
    public let showSettings = ShowSettingsModel()
    public let accounts = AccountsModel()
    public let providers = ProvidersModel()
    public let stores = StoresModel()
    public let downloads = DownloadsModel()
    /// New-episode notifications: the week coming up, permission, and each show's switch.
    public let notifications = NotificationsModel()
    /// The engine's answers to questions asked of it, for the notifications and the widgets.
    public let answers = EngineAnswers()
    /// Downloads that go on after the app leaves, in the system's window for them (iOS).
    public let continued = ContinuedDownloads()
    /// The update check's word, and an update the viewer started.
    public let updates = UpdatesModel()
    /// Play on TV: the TVs, pairing, and the card of the one driven.
    public let tv = TVRemoteModel()
    /// The week's ask, waiting a moment in case more ask at once.
    @ObservationIgnored var noticeRefresh: Task<Void, Never>?
    /// The home-screen widgets' snapshot, kept current as what they show changes.
    @ObservationIgnored public let widgets: WidgetFeed
    /// The downloads' Live Activity on iPhone.
    @ObservationIgnored public let downloadsActivity: DownloadsActivityFeed
    @ObservationIgnored public private(set) var artwork: ArtworkStore!

    @ObservationIgnored private var client: EngineClient?
    /// When a batch was last applied.
    @ObservationIgnored private var lastApplied = ContinuousClock.now
    /// What the screens asked for before the engine was there, sent once it is.
    @ObservationIgnored private var early: [Command] = []
    /// The titles whose episodes were asked for their stills.
    @ObservationIgnored private var stillsAsked: Set<AnilistId> = []
    /// When each question about a title last went out (see `ask`).
    @ObservationIgnored private var asked: [TitleQuestion: ContinuousClock.Instant] = [:]
    /// The kinds of the engine's events a showcase set aside, so its rows stay on screen.
    @ObservationIgnored private var held: Set<String> = []
    /// Whether the engine is the testing one, which a showcase needs.
    @ObservationIgnored private var testing = false
    /// Called after each batch is applied: what a screenshot run waits on.
    @ObservationIgnored public var onApplied: (@MainActor ([Event]) -> Void)?
    /// Where the engine started keeps its files: the cache is where the player keeps mpv's.
    public private(set) var home: EngineHome?
    /// When the last play was sent, for the log's timings.
    @ObservationIgnored private var playSent: ContinuousClock.Instant?
    /// A play that starts at a place of its own, handed over from another device.
    @ObservationIgnored private var startAt: StartAt?
    /// The playback's events, for the player the app hosts: its ops and interface, and the
    /// playback around them (AnistreamPlayerUI's `PlayerModel.apply(_:)`).
    @ObservationIgnored public var onPlayback: (@MainActor (Event) -> Void)?

    /// - Parameter artworkCostLimit: how many bytes of decoded art the cache keeps beside what is
    ///   on screen; smaller in the dev harness, to see the page with the cache full.
    public init(
        jobs: JobsModel = JobsModel(), notices: NoticesModel = NoticesModel(), widgets: WidgetFeed = WidgetFeed(),
        downloadsActivity: DownloadsActivityFeed = DownloadsActivityFeed(),
        artworkCostLimit: Int = ArtworkStore.costLimit
    ) {
        self.jobs = jobs
        self.notices = notices
        self.widgets = widgets
        self.downloadsActivity = downloadsActivity
        artwork = ArtworkStore(costLimit: artworkCostLimit) { [weak self] url in
            guard let client = self?.client else { return false }
            client.send(.fetchArtwork(url: url))
            return true
        }
        continued.send = { [weak self] command in self?.send(command) }
        updates.send = { [weak self] command in self?.send(command) }
        // The downloads' Live Activity and the system's own progress for them never show at once.
        continued.runningChanged = { [weak self] running in self?.downloadsActivity.systemShowsProgress = running }
    }

    // MARK: The engine

    /// Start the engine `launch` describes with `host`, off the main thread, and read its events.
    public func start(_ launch: EngineLaunch, host: any FfiHost) async {
        phase = .starting
        #if FIXTURES
        if case .fixtures = launch.engine { testing = true }
        #endif
        // The Live Activity's Pause and Resume act on the whole queue.
        downloadsActivity.takeControls(
            pause: { [weak self] in self?.pauseAllDownloads() },
            resume: { [weak self] in self?.resumeAllDownloads() })
        home = launch.home
        AppLog.start(cache: launch.home.cache)
        #if os(macOS)
        AppLog.info("store", "helper tools are looked for on \(HelperPath.extend())")
        #endif
        let began = ContinuousClock.now
        do {
            let engine = try await Task.detached(priority: .userInitiated) {
                try launch.start(host: host)
            }.value
            AppLog.info("store", "engine started in \(AppLog.ms(since: began)) ms")
            attach(engine)
        } catch let error as FfiError {
            AppLog.error("store", "engine did not start: \(error.reason)")
            phase = .failed(error.reason)
            early = []
        } catch {
            AppLog.error("store", "engine did not start: \(error.localizedDescription)")
            phase = .failed(error.localizedDescription)
            early = []
        }
    }

    #if FIXTURES
    /// Every command sent, by its case's name, for counting what an action costs. The Fixtures
    /// app only.
    @ObservationIgnored public private(set) var sentLog: [String] = []

    /// What the fake AniList has been asked so far, an operation's name a request, in arrival
    /// order.
    public var anilistRequests: [String] { client?.engine.testingAnilistRequests() ?? [] }

    /// Serve the fixtures' art badly from now on, as a busy CDN would: every second image fails
    /// `failFirst` times before it is served, and each waits `delay` (the testing engine's
    /// `testingTroubleArt`). The Fixtures app only.
    public func troubleArt(failFirst: Int, delay: Duration) {
        let ms = delay.components.seconds * 1000 + delay.components.attoseconds / 1_000_000_000_000_000
        client?.engine.testingTroubleArt(failFirst: UInt32(max(0, failFirst)), delayMs: UInt32(max(0, ms)))
    }
    #endif

    /// Read `engine`'s events from now on.
    public func attach(_ engine: FfiEngine) {
        client = EngineClient(
            engine: engine,
            deliver: { [weak self] events in self?.apply(events) },
            stopped: { [weak self] in self?.phase = .stopped }
        )
        phase = .running
        attachTV(engine)
        for command in early {
            client?.send(command)
        }
        early = []
        artwork.sendUnsent()
    }

    /// The engine's sender for Play on TV, browsing the network with Bonjour; in the Fixtures
    /// app, two made-up TVs on the loopback interface instead, set up as `-AnistreamTV` says.
    private func attachTV(_ engine: FfiEngine) {
        let sender: FfiTvSender
        do {
            sender = try engine.tvSender(deviceName: TVRemoteModel.deviceName, app: TVRemoteModel.appName)
        } catch {
            AppLog.error("tv", "no Play on TV: \(error)")
            return
        }
        #if FIXTURES
        if testing, let fakes = TVRemoteModel.fixtures(engine) {
            tv.attach(sender, browser: fakes)
            if let scene = TVScene.named() {
                // The finished fixture title, its third episode, five minutes in.
                let play = TVPlayRequest(id: 900_001, title: "", episode: "3", from: 312)
                Task { await tv.stage(scene, fakes: fakes, play: play) }
            }
            return
        }
        #endif
        tv.attach(sender, browser: BonjourTVBrowser())
    }

    /// Play on TV from a title page or an episode row: the episode, from where it was left when
    /// it is the one being resumed.
    public func playOnTV(_ id: AnilistId, episode: String, from: Double? = nil) {
        tv.offer(TVPlayRequest(id: id, title: titles[id]?.title ?? "", episode: episode, from: from))
    }

    /// Hand the engine a command. `nil` for work nobody waits on, and when there is no engine:
    /// while it starts, the command waits and is sent once it has.
    @discardableResult
    public func send(_ command: Command) -> JobId? {
        #if FIXTURES
        sentLog.append(String(String(describing: command).prefix { $0 != "(" }))
        #endif
        switch command {
        case .play(let id, let episode, _):
            playSent = .now
            startAt = nil
            AppLog.info("store", "play \(id) episode \(episode)")
        case .playSource(let id, let episode, _, _):
            playSent = .now
            startAt = nil
            AppLog.info("store", "play \(id) episode \(episode) from a chosen source")
        case .playLocal, .playParty:
            startAt = nil
        default:
            break
        }
        guard let client else {
            if phase == .starting { early.append(command) }
            return nil
        }
        let job = client.send(command)
        if let then = startAt?.after(command) {
            startAt = nil
            send(then)
        }
        return job
    }

    /// Play `episode` of `id` from `position`, as a Handoff from another device asks: played as
    /// any other, then moved there once its file is open.
    /// Any other play sent after it gives the place up.
    public func play(_ id: AnilistId, episode: String, at position: Double) {
        send(.play(id: id, episode: episode))
        startAt = StartAt(id: id, episode: episode, position: position)
    }

    /// Stop the engine, waiting while it winds down: at quit.
    public func shutdown() {
        client?.shutdown()
    }

    // MARK: Asking

    /// How long a list's answer stands: showing it again within this asks for nothing, so going
    /// back and forth between sections costs AniList nothing.
    public static let listFresh: Duration = .seconds(60)

    /// How long a question about a title stands once asked: the same one is not put again while
    /// its answer is on its way, which a selection moving back and forth would otherwise do.
    public static let askAgainAfter: Duration = .seconds(20)

    /// What you were watching, unless it was answered within `listFresh`; `force` asks anyway.
    public func loadContinue(force: Bool = false) {
        guard force || !continuing.list.answered(within: Self.listFresh) else { return }
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

    /// Search now, as Return, a recent search or a rail's See All does: always asked, and kept
    /// among the recent searches.
    public func search(_ query: SearchQuery) {
        _ = search.edit(query)
        guard !query.normalized.isBlank() else { return }
        send(.search(query: query.normalized))
    }

    /// Search for these words with the filters already set.
    public func search(text: String) {
        var query = search.query
        query.text = text
        search(query)
    }

    /// The search changed, a keystroke or a filter: the engine searches once the changes stop.
    public func searchLive(_ query: SearchQuery) {
        guard search.edit(query) else { return }
        send(.searchLive(query: query.normalized))
    }

    /// The text changed, with the filters as they are.
    public func searchLive(text: String) {
        guard text != search.query.text else { return }
        var query = search.query
        query.text = text
        searchLive(query)
    }

    /// The next page of the results, as the end of them comes near.
    public func searchMore() {
        guard let (query, page) = search.nextPage() else { return }
        send(.searchMore(query: query, page: page))
    }

    /// Search's start: the rails, the recent searches and what the filters choose from.
    public func loadSearchStart() {
        if search.rails == nil { send(.loadBrowse) }
        if search.facets == nil { send(.loadSearchFacets) }
        send(.loadRecentSearches)
    }

    /// Keep what is searched for among the recent searches, as opening one of its titles does.
    public func rememberSearch() {
        guard let asked = search.asked else { return }
        send(.rememberSearch(asked))
    }

    public func forgetRecentSearch(_ query: SearchQuery) {
        send(.forgetRecentSearch(query))
    }

    public func clearRecentSearches() {
        send(.clearRecentSearches)
    }

    public func loadLibrary(_ segment: LibrarySegment) {
        library.segment(segment).asked()
        send(.loadLibrary(segment))
    }

    /// A title's page. `force` asks again however recently it was asked: Try Again.
    public func loadDetail(_ id: AnilistId, force: Bool = false) {
        guard ask(.detail(id)) || force else { return }
        send(.loadDetail(id))
    }

    public func loadEpisodes(_ id: AnilistId) {
        guard ask(.episodes(id)) else { return }
        send(.loadEpisodes(id))
    }

    /// Sources are ready where none were: each title whose episodes are AniList's alone is asked
    /// for them again, however lately it was, so its list takes its source's numbering.
    private func sourcesArrived() {
        for id in titles.unlistedTitles {
            asked[.episodes(id)] = nil
            loadEpisodes(id)
        }
    }

    /// A question about one title.
    private enum TitleQuestion: Hashable {
        case detail(AnilistId)
        case episodes(AnilistId)
    }

    /// Whether `question` may go out now: not asked within `askAgainAfter`. Noted as asked when it
    /// may.
    private func ask(_ question: TitleQuestion) -> Bool {
        let now = ContinuousClock.now
        if let at = asked[question], now - at < Self.askAgainAfter { return false }
        asked[question] = now
        return true
    }

    /// Ask for a title's episodes for the stills that stand in for a missing banner. Once a
    /// session per title, and not when they are here already, so a selection moving back and
    /// forth costs one request. Whether it asked.
    @discardableResult
    public func loadStills(_ id: AnilistId) -> Bool {
        guard titles[id]?.bannerUrl == nil, titles.episodes[id] == nil,
              stillsAsked.insert(id).inserted, ask(.episodes(id))
        else { return false }
        send(.loadEpisodes(id))
        return true
    }

    /// Every title is named another way now: the lists that have been shown are asked for again,
    /// so they name their titles the new way, and a title page asks again when it next opens.
    private func titlesChanged() {
        asked.removeAll()
        if continuing.list.answeredAt != nil { loadContinue(force: true) }
        if seasonal.answeredAt != nil { loadSeasonal() }
        if calendar.list.answeredAt != nil { loadCalendar() }
        for segment in LibraryModel.order where library.segment(segment).answeredAt != nil {
            loadLibrary(segment)
        }
        if let query = search.again() { send(.search(query: query)) }
        if search.rails != nil { send(.loadBrowse) }
    }

    /// Adult titles were let in or kept out: Search and its start are asked for again, so they
    /// show them or leave them out at once.
    private func adultChanged() {
        if let query = search.again() { send(.search(query: query)) }
        if search.rails != nil { send(.loadBrowse) }
        if search.facets != nil { send(.loadSearchFacets) }
        if seasonal.answeredAt != nil { loadSeasonal() }
    }

    /// Take a title off Continue: the row goes at once, and the engine keeps it hidden.
    public func hideFromContinue(_ id: AnilistId) {
        continuing.hide(id)
        send(.hideFromContinue(id: id))
    }

    // MARK: Applying

    public func apply(_ events: [Event]) {
        var events = events
        #if FIXTURES
        if let delay = episodesDelay {
            // Episode tables held back, as a slow source answers them.
            let tables = events.filter { if case .episodes = $0 { true } else { false } }
            events.removeAll { if case .episodes = $0 { true } else { false } }
            if !tables.isEmpty {
                Task { [weak self] in
                    try? await Task.sleep(for: delay)
                    guard let self else { return }
                    for event in tables { apply(event) }
                    lastApplied = .now
                }
            }
        }
        #endif
        for event in events {
            apply(event)
        }
        lastApplied = .now
        onApplied?(events)
    }

    #if FIXTURES
    /// How long each episode table waits before the screens see it: the dev harness's slow
    /// source (`--episodes-delay`). The testing engine only.
    public var episodesDelay: Duration?
    #endif

    /// Show the invented rows the launch arguments ask for, on the testing engine only, and keep
    /// the engine's own of those kinds off screen from then on. For screenshots.
    public func applyShowcase(arguments: [String] = ProcessInfo.processInfo.arguments) {
        #if FIXTURES
        guard testing, let showcase = Showcase.from(arguments: arguments) else { return }
        let (events, held) = showcase.events(settings: settings.view) { [titles] in titles[$0] }
        self.held = []
        for event in events { apply(event) }
        self.held = held
        #endif
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
        if !held.isEmpty, let kind = event.showcaseKind, held.contains(kind) { return }
        notifications.observe(event)
        answers.deliver([event])
        if case .downloads(let rows) = event { continued.update(rows) }
        if notifications.schedules, NotificationsModel.reschedules(after: event) { refreshNotices() }
        switch event {
        case .jobStarted(let job, let activity):
            jobs.started(job, activity: activity)
        case .jobNote(let job, let note, let wait):
            jobs.note(job, note: note, wait: wait)
        case .jobDone(let job):
            jobs.done(job)
            stores.done(job)
            accounts.done(job)

        case .content(let view, let content):
            self.content(content, for: view)
            if case .continue = view { widgetsChanged() }
            if case .calendar = view { widgetsChanged() }
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
            widgetsChanged()
        case .lastAired(let rows):
            titles.lastAired(rows)
        case .episodes(let id, let rows):
            titles.episodes(id: id, rows: rows)

        case .status(let text):
            notices.status(text)
        case .toast(let notice):
            notices.post(notice)

        case .settings(let view):
            let adult = settings.view.map { _ in settings.isOn(.adultTitles) }
            settings.apply(view)
            if let adult, adult != settings.isOn(.adultTitles) { adultChanged() }
        case .showSettings(let view):
            showSettings.apply(view)

        case .sync(let state):
            accounts.apply(state)
            widgetsChanged()
        case .progressQueued:
            accounts.progressQueued()
        case .conflicts(let rows):
            accounts.conflicts(rows)
        case .deviceCode(let prompt):
            accounts.deviceCode(prompt)
        case .authorize(let tracker, let url, let callbackScheme):
            if let url = URL(string: url) {
                accounts.authorize(SignInRequest(tracker: tracker, url: url, callbackScheme: callbackScheme))
            } else {
                // Nothing a sheet could open: the sign-in ends here.
                send(.cancelSignIn(tracker: tracker))
            }
        case .metaStatus(let rows):
            accounts.metaStatus(rows)
            providers.meta(rows)

        case .providers(let rows):
            let had = providers.rows.contains { $0.usable }
            providers.apply(rows)
            if !had, rows.contains(where: { $0.usable }) { sourcesArrived() }
        case .providerNote(let text):
            providers.note(text)
        case .sourcesChanged:
            providers.sourcesChanged()
        case .providerPreference(let provider):
            providers.preference(provider)
        case .vpn(let view):
            providers.guardSaid(view)

        case .stores(let rows):
            stores.apply(rows)
        case .storePreview(let answer):
            stores.previewed(answer)
        case .storeJoined(let store):
            stores.joined(store)
        case .inviteLink(let link):
            stores.invited(link)
        case .storeMembers(let store, let members, let invites):
            stores.members(store: store, members: members, invites: invites)
        case .localPlugins(let store, let plugins):
            stores.localPlugins(store: store, plugins: plugins)
        case .openJoin(let store, let view):
            stores.openJoin(store: store, view: view)

        case .downloads(let rows):
            downloads.apply(rows)
            downloadsActivity.downloadsChanged(downloads, titles: titles, artwork: artwork)
        case .seeding(let rows):
            downloads.seeding(rows)

        case .artworkReady(let url, let bytes):
            artwork.ready(url: url, bytes: bytes)
        case .imageFailed(let url, _):
            artwork.failed(url: url)

        case .playbackStaged(let id, _, let episode, _, _):
            if startAt?.staged(id: id, episode: episode) == false { startAt = nil }
            let after = playSent.map { " \(AppLog.ms(since: $0)) ms after play" } ?? ""
            AppLog.info("store", "staged \(id) episode \(episode)\(after)")
            onPlayback?(event)
        case .playbackEnded(let watched, let ending, let rollingOn):
            AppLog.info("store", "playback ended: \(ending), watched \(watched), rolling on \(rollingOn)")
            onPlayback?(event)
            widgetsChanged()
        case .playerOps, .playerUi, .playbackKept, .sources:
            onPlayback?(event)

        case .widgetSnapshot(let data):
            widgets.received(data, art: artwork)

        case .update(let status):
            updates.apply(status)

        case .searchPaging(let query, let hasNext):
            search.paging(query, hasNext: hasNext)
        case .searchMore(let query, let page, let entries, let hasNext):
            search.more(query, page: page, entries: entries, hasNext: hasNext, titles: titles)
        case .liveSearch(let paused):
            search.live(paused: paused)
        case .browse(let rails):
            search.rails(rails, titles: titles)
        case .facets(let facets):
            search.facets(facets)
        case .recentSearches(let queries):
            search.recents(queries)

        case .viewerTitleLanguage(let language):
            accounts.viewerTitleLanguage = language
        case .titlesChanged:
            titlesChanged()

        default:
            // Everything else is said in the events above, or has no screen.
            break
        }
    }

    /// What the widgets show may have changed: ask for their snapshot once things settle.
    private func widgetsChanged() {
        widgets.changed { [weak self] in self?.send(.widgetSnapshot) }
    }

    private func content(_ content: ContentList, for view: ContentFor) {
        switch view {
        case .continue:
            continuing.apply(content, titles: titles)
        case .seasonal:
            seasonal.apply(content, titles: titles)
        case .calendar:
            calendar.apply(content, titles: titles)
        case .search(let query):
            guard search.takes(query) else { return }
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
        case .search(let query):
            if search.takes(query) { search.list.caption = text.isEmpty ? nil : text }
        case .library(let segment): library.segment(segment).caption = text
        }
    }
}
