// The app's state: the engine's events applied, each by the feature model that owns it.

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiException
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.JobId
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SearchFilters
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.ShowSettingKey

/**
 * Where the app's state lives, on the main thread.
 *
 * The engine's events arrive in batches from [EngineClient], and [apply] hands each one to the
 * model that owns it: the lists, the title cache, jobs, notices, accounts, stores and artwork.
 * Screens collect the models' flows; commands go out through [send]. Every call is made on the
 * main thread.
 *
 * @param scope where the engine starts and its events are read: the application's, for as long as
 *   the process.
 * @param artworkWithoutEngine where art comes from when there is no engine behind the store, as
 *   in the screenshot tests: a URL's bytes, answered as the engine would answer them, or `null`
 *   for none. Unset, art waits for the engine.
 */
class AppStore(
    private val scope: CoroutineScope,
    val jobs: JobsModel = JobsModel(scope = scope),
    val notices: NoticesModel = NoticesModel(),
    decoder: ArtworkDecoder = BitmapDecoder,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
    private val artworkWithoutEngine: ((String) -> ByteArray?)? = null,
) {
    /** Where the engine stands. */
    sealed interface Phase {
        data object Starting : Phase
        data object Running : Phase
        data object Stopped : Phase

        /** It did not start, and why, in the engine's words. */
        data class Failed(val reason: String) : Phase
    }

    private val _phase = MutableStateFlow<Phase>(Phase.Starting)
    val phase: StateFlow<Phase> = _phase.asStateFlow()

    val titles = TitleCache()
    val continuing = ContinueModel()
    val seasonal = ListModel()
    val calendar = CalendarModel()
    val search = SearchModel()
    val library = LibraryModel()
    val accounts = AccountsModel()
    val stores = StoresModel()
    val downloads = DownloadsModel()
    val settings = SettingsModel()
    val providers = ProvidersModel()
    val showSettings = ShowSettingsModel()
    val episodeFolds = EpisodeFolds()
    val coming = ComingModel()

    /** The app's own update: the newer release, and the download and install the viewer started. */
    val updates = UpdateModel(notices)
    val artwork = ArtworkStore(scope, decoder) { url ->
        val client = client
        val files = artworkWithoutEngine
        when {
            client != null -> {
                client.send(Command.FetchArtwork(url))
                true
            }
            files != null -> {
                val bytes = files(url)
                scope.launch { apply(listOf(if (bytes != null) Event.ArtworkReady(url, bytes) else Event.ImageFailed(url, offline = false))) }
                true
            }
            else -> false
        }
    }

    private var client: EngineClient? = null

    /** What the screens asked for before the engine was there, sent once it is. */
    private val early = mutableListOf<Command>()

    /** When a batch was last applied, on [clock]. */
    private var lastApplied = clock()

    /** Called after each batch is applied: what a screenshot run waits on. */
    var onApplied: ((List<Event>) -> Unit)? = null

    /** The playback's events, for the player the app hosts: its ops and interface. */
    var onPlayback: ((Event) -> Unit)? = null

    private val _answers = MutableSharedFlow<Event>(extraBufferCapacity = 8)

    /**
     * The events that answer a command once and with no job: the new-episode check's and the
     * widgets' record, for the background work that asked, as [ask] waits for them.
     */
    val answers: SharedFlow<Event> = _answers.asSharedFlow()

    /**
     * Send [command] and wait for the event that answers it, the first [answer] takes, at most
     * [timeoutMillis]; `null` when none came. Listening starts before the command goes, so an
     * answer that comes at once is not missed.
     */
    suspend fun <T : Any> ask(command: Command, timeoutMillis: Long, answer: (Event) -> T?): T? = coroutineScope {
        val answered = async(start = CoroutineStart.UNDISPATCHED) { answers.mapNotNull(answer).first() }
        send(command)
        withTimeoutOrNull(timeoutMillis) { answered.await() }.also { answered.cancel() }
    }

    // The engine

    /**
     * Start the engine [launch] describes with [host], off the main thread, and read its events.
     * [prepare] runs first, on the same thread: the first call into the engine's library, which
     * loads it and JNA, so that never happens on the main thread.
     */
    fun start(launch: EngineLaunch, host: FfiHost, prepare: () -> Unit = {}) {
        _phase.value = Phase.Starting
        scope.launch {
            try {
                val engine = withContext(Dispatchers.IO) {
                    traced("prepare") { prepare() }
                    traced("engine start") { launch.start(host) }
                }
                attach(engine)
            } catch (e: FfiException) {
                _phase.value = Phase.Failed(e.reason)
                early.clear()
            }
        }
    }

    /** Read [engine]'s events from now on. */
    fun attach(engine: FfiEngine) {
        val client = EngineClient(
            engine,
            scope,
            deliver = ::apply,
            stopped = { _phase.value = Phase.Stopped },
        )
        this.client = client
        _phase.value = Phase.Running
        for (command in early) client.send(command)
        early.clear()
    }

    /**
     * Hand the engine a command. `null` for work nobody waits on, and when there is no engine:
     * while it starts, the command waits and is sent once it has.
     */
    fun send(command: Command): JobId? {
        val client = client
        if (client == null) {
            if (_phase.value == Phase.Starting) early += command
            return null
        }
        return client.send(command)
    }

    /** The commands waiting for the engine to start, for a test to look at. */
    val waiting: List<Command> get() = early.toList()

    /** Stop the engine, waiting while it winds down. */
    fun shutdown() {
        client?.shutdown()
    }

    // Asking

    /** What you were watching, unless it was answered within [LIST_FRESH_MILLIS]; [force] asks anyway. */
    fun loadContinue(force: Boolean = false) {
        if (!force && continuing.list.answered(LIST_FRESH_MILLIS)) return
        continuing.list.asked()
        send(Command.LoadContinue)
    }

    fun loadSeasonal(force: Boolean = false) {
        if (!force && seasonal.answered(LIST_FRESH_MILLIS)) return
        seasonal.asked()
        send(Command.LoadSeasonal)
    }

    fun loadCalendar(force: Boolean = false) {
        if (!force && calendar.list.answered(LIST_FRESH_MILLIS)) return
        calendar.list.asked()
        send(Command.LoadCalendar)
    }

    fun search(query: String, filter: SearchFilters = SearchFilters()) {
        search.submit(query, filter)
        send(Command.Search(query, filter))
    }

    fun loadLibrary(segment: LibrarySegment) {
        library.segment(segment).asked()
        send(Command.LoadLibrary(segment))
    }

    fun loadDetail(id: AnilistId) {
        titles.detailAsked(id)
        detailPending += id
        send(Command.LoadDetail(id))?.let { detailJobs[it] = id }
    }

    /** Each title page's fetch by the job that carries it, to tell an answer from none. */
    private val detailJobs = mutableMapOf<JobId, AnilistId>()
    private val detailPending = mutableSetOf<AnilistId>()

    /** A page's fetch ended: without the title, AniList did not answer. */
    private fun detailDone(job: JobId) {
        val id = detailJobs.remove(job) ?: return
        if (detailPending.remove(id)) titles.detailFailed(id)
    }

    /**
     * The form titles are named in changed: what is on screen names them the old way, so every
     * list answered, the search and the pages last opened are asked for again.
     */
    private fun titlesChanged() {
        if (continuing.list.answeredAt != null) loadContinue(force = true)
        if (seasonal.answeredAt != null) loadSeasonal(force = true)
        if (calendar.list.answeredAt != null) loadCalendar(force = true)
        for (segment in LibraryModel.order) {
            if (library.segment(segment).answeredAt != null) loadLibrary(segment)
        }
        search.submitted?.let { (query, filter) -> search(query, filter) }
        for (id in titles.detailed.toList().takeLast(5)) loadDetail(id)
    }

    /** A title's episodes, once a session per title unless [force]: for the names of what plays. */
    fun loadEpisodes(id: AnilistId, force: Boolean = false) {
        if (!force && (id in episodesAsked || titles.episodes.value[id] != null)) return
        episodesAsked += id
        send(Command.LoadEpisodes(id))
    }

    private val episodesAsked = mutableSetOf<AnilistId>()

    /**
     * Sources are ready where none were: each title whose episodes are AniList's alone is asked for
     * them again, so its list takes its source's numbering.
     */
    private fun sourcesArrived() {
        for (id in titles.unlistedTitles()) loadEpisodes(id, force = true)
    }

    /** [id]'s own settings sheet, answered with `ShowSettings`. */
    fun loadShowSettings(id: AnilistId) {
        send(Command.LoadShowSettings(id))
    }

    /** Set [key] for [id] alone, or with `null` have it follow Settings again. */
    fun setShowSetting(id: AnilistId, key: ShowSettingKey, value: SettingValue?) {
        send(Command.SetShowSetting(id, key, value))
    }

    /** Put [id] back on Settings: its own settings, the source it keeps and the remembered tracks. */
    fun resetShowSettings(id: AnilistId) {
        send(Command.ResetShowSettings(id))
    }

    /** Put [id] on the tracker's [segment] list: shown at once, ahead of the tracker's word. */
    fun setStatus(id: AnilistId, segment: LibrarySegment) {
        titles.setStatus(id, segment)
        send(Command.SetStatus(id, segment))
    }

    /** Open [url] in the browser, through the engine, which says so if it cannot. */
    fun openExternal(url: String) {
        send(Command.OpenExternal(url))
    }

    /** The downloads, once a session unless [force]. */
    fun loadDownloads(force: Boolean = false) {
        if (!force && downloads.loaded.value) return
        send(Command.LoadDownloads)
    }

    /** Queue [episode] of [id] for download. */
    fun downloadEpisode(id: AnilistId, episode: String) {
        send(Command.DownloadEpisode(id, episode))
    }

    /** Pause a download, or resume it. */
    fun toggleDownload(id: Long) {
        send(Command.DownloadPause(id))
    }

    /** Take a download off the queue, keeping a finished file. */
    fun cancelDownload(id: Long) {
        send(Command.DownloadCancel(id))
    }

    /** Take a download off the queue and delete its file. */
    fun deleteDownload(id: Long) {
        send(Command.DownloadDelete(id))
    }

    fun clearCompletedDownloads() {
        send(Command.DownloadClearCompleted)
    }

    /** Pause every download that runs or waits. */
    fun pauseAllDownloads() {
        for (row in downloads.rows.value) {
            if (row.state == tv.anistream.engine.model.DownloadState.ACTIVE || row.state == tv.anistream.engine.model.DownloadState.QUEUED) toggleDownload(row.id)
        }
    }

    /** Resume every paused download. */
    fun resumeAllDownloads() {
        for (row in downloads.rows.value) if (row.state == tv.anistream.engine.model.DownloadState.PAUSED) toggleDownload(row.id)
    }

    /** Play a finished download from its file, with the history, resume and sync a stream has. */
    fun playDownload(row: tv.anistream.engine.model.DownloadRow) {
        val path = row.path ?: return
        send(Command.PlayLocal(row.anilistId, row.episode, row.title, path))
    }

    /** The device's network, as the system says it: cellular, and Data Saver as constrained. */
    fun networkPath(cellular: Boolean, constrained: Boolean) {
        send(Command.NetworkPath(cellular, constrained))
    }

    /** Take a title off Continue: the row goes at once, and the engine keeps it hidden. */
    fun hideFromContinue(id: AnilistId) {
        continuing.hide(id)
        send(Command.HideFromContinue(id))
    }

    /** Mark [episodes] of [id] watched, or not. */
    fun setWatched(id: AnilistId, episodes: List<String>, watched: Boolean = true) {
        send(Command.SetWatched(id, episodes, watched))
    }

    // Settings

    /** Step a setting one rung, or over to its other state. */
    fun stepSetting(key: SettingKey, delta: Long) {
        send(Command.StepSetting(key, delta))
    }

    /** Set a setting to [value]: a menu's choice, a typed value, the source order. */
    fun setSetting(key: SettingKey, value: SettingValue) {
        send(Command.SetSetting(key, value))
    }

    /** Turn a switch on or off, when it is not already. */
    fun setSwitch(key: SettingKey, on: Boolean) {
        if (settings.isOn(key) == on) return
        setSetting(key, SettingValue.Bool(on))
    }

    /**
     * The week's broadcasts for the shows notified of, and those a show's own switch leaves out:
     * Notifications' "Coming up".
     */
    fun loadComing() {
        send(Command.UpcomingNotices)
    }

    /** The title language the signed-in AniList viewer reads in, for AniList's choice. */
    fun loadTitleLanguage() {
        send(Command.LoadTitleLanguage)
    }

    // The app's update

    /**
     * Update, as the notice's button and Settings' row press it: the download and install where
     * this build updates itself and the release has an APK, else the release's page.
     */
    fun takeUpdate() {
        val offer = updates.state.value.offer ?: return
        if (updates.installs(offer)) updates.press() else if (offer.page.isNotEmpty()) openExternal(offer.page)
    }

    /** Settings' Check now: GitHub asked whatever the daily check's switch says. */
    fun checkForUpdates() {
        send(Command.CheckForUpdates)
    }

    // Accounts

    /** Send what waits and pull the library now, rather than at the next interval. */
    fun syncNow() {
        accounts.started("sync", send(Command.SyncNow))
    }

    /**
     * Start [tracker]'s own sign-in: the browser for AniList and MyAnimeList, which the engine asks
     * for with `Authorize`; a code to type elsewhere for Simkl and Trakt.
     */
    fun connect(tracker: String) {
        accounts.started(tracker, send(Command.Connect(tracker)))
    }

    /**
     * The browser came back from [request] with [callback], the redirect that carries the code or
     * the token itself, so the engine finishes signing in. Handed over as it came, and kept
     * nowhere else.
     */
    fun finishSignIn(request: SignInRequest, callback: String) {
        accounts.signInEnded(request)
        accounts.started(request.tracker, send(Command.FinishSignIn(request.tracker, callback)))
    }

    /**
     * The browser came back to the app for [tracker] with [callback], the redirect that carries
     * the code or the token: handed to the engine as it came, which checks it against the sign-in
     * it waits on. It ends the open sign-in only when it is that one's own, for its tracker and
     * with its state, as the engine goes on waiting for any other: the two stay in step, and the
     * tab's own redirect, still to come, finishes it. Says whether it ended the open one.
     */
    fun redirected(tracker: String, callback: String): Boolean {
        accounts.leftForTab = false
        val request = accounts.signIn.value?.takeIf { it.answeredBy(tracker, callback) }
        if (request != null) accounts.signInEnded(request)
        accounts.started(tracker, send(Command.FinishSignIn(tracker, callback)))
        return request != null
    }

    /** The browser for [request] was closed without signing in. */
    fun cancelSignIn(request: SignInRequest) {
        accounts.signInEnded(request)
        send(Command.CancelSignIn(request.tracker))
    }

    /** Kitsu's sign-in: its email or user name and password, exchanged for a token by the engine. */
    fun connectKitsu(username: String, password: String) {
        accounts.started("kitsu", send(Command.ConnectKitsu(username, password)))
    }

    /** Settle what a pull found different on [id], keeping this device's side or AniList's. */
    fun resolveConflict(id: AnilistId, keepLocal: Boolean) {
        send(Command.ResolveConflict(id, keepLocal))
    }

    /** Sign out of [tracker]: the engine forgets its token. */
    fun disconnect(tracker: String) {
        send(Command.Disconnect(tracker))
    }

    // Sources

    /**
     * Protect torrent traffic with [mode], through [proxy] when it is one, and have a failed
     * connection check do [onFailure]: written as one change, the proxy's password to the
     * keystore-sealed secrets and never to the file. No protection and warn only need
     * [acknowledged], the viewer having confirmed it.
     */
    fun setTorrentProtection(
        mode: tv.anistream.engine.model.TorrentProtection,
        proxy: tv.anistream.engine.model.TorrentProxy?,
        onFailure: tv.anistream.engine.model.FailedCheck,
        acknowledged: Boolean,
    ) {
        send(Command.SetTorrentProtection(mode, proxy, onFailure, acknowledged))
    }

    /** Check every provider again. */
    fun checkProviders() {
        send(Command.CheckProviders)
    }

    /**
     * Switch the plugin [id] off, or back on, without removing it: it keeps its place and its row,
     * and is not tried while off.
     */
    fun disablePlugin(id: String, disabled: Boolean) {
        send(Command.DisablePlugin(id, disabled))
    }

    /** Read the joined stores, then refresh each one's plugin list. */
    fun loadStores() {
        send(Command.LoadStores)
    }

    /** Install [plugin] from [store], or update it. */
    fun installPlugin(store: String, plugin: String) {
        stores.started(StoresModel.Work.Install(store, plugin), send(Command.InstallPlugin(store, plugin)))
    }

    /** Delete an installed plugin from this device. */
    fun removePlugin(plugin: String) {
        stores.started(StoresModel.Work.Remove(plugin), send(Command.RemovePlugin(plugin)))
    }

    /** Check a pasted invite link against the store it names, before spending it. */
    fun previewStore(link: String) {
        val trimmed = link.trim()
        stores.checking(trimmed)
        send(Command.PreviewStore(trimmed))
    }

    /** Spend an invite link for a token and join, calling this device what the join sheet's field says. */
    fun joinStore(link: String) {
        stores.started(StoresModel.Work.Join, send(stores.joinCommand(link)))
    }

    /** Forget [store] and revoke this device's token there. */
    fun leaveStore(store: String) {
        stores.started(StoresModel.Work.Leave(store), send(Command.LeaveStore(store)))
    }

    /** An invite link: for someone new with a [name], for another device of yours without. */
    fun createInvite(store: String, name: String?) {
        stores.started(StoresModel.Work.Invite(store), send(Command.CreateInvite(store, name)))
    }

    /** Pin [provider] as the source for title [id], or give the title back to automatic with `null`. */
    fun setProviderPreference(id: AnilistId, provider: String?) {
        send(Command.SetProviderPreference(id, provider))
    }

    /** The providers in the order failover tries them, as the config now has it. */
    fun orderedProviders(): List<tv.anistream.engine.model.ProviderRow> =
        SourceOrder.sorted(providers.rows.value, settings.view.value?.sources?.order.orEmpty())

    // Settling

    /**
     * Whether the screens have what they asked for: the engine has started or failed, no list is
     * loading, no job is running, and no art is on its way.
     */
    val isSettled: Boolean
        get() {
            val lists = listOf(continuing.list, seasonal, calendar.list, search.list) +
                LibraryModel.order.map(library::segment)
            return _phase.value != Phase.Starting &&
                lists.none { it.state.value == ListState.Loading } &&
                jobs.running.value.isEmpty() &&
                !artwork.hasPending
        }

    /**
     * Wait until the store is settled and nothing has been applied for [quietMillis]: what a
     * screenshot waits on. False when [timeoutMillis] passed first.
     */
    suspend fun settle(quietMillis: Long = 800, timeoutMillis: Long = 30_000): Boolean {
        val deadline = clock() + timeoutMillis
        while (clock() < deadline) {
            if (isSettled && clock() - lastApplied >= quietMillis) return true
            delay(50)
        }
        return false
    }

    // Applying

    fun apply(events: List<Event>) = traced("apply ${events.size} events") {
        for (event in events) apply(event)
        lastApplied = clock()
        onApplied?.invoke(events)
    }

    fun apply(event: Event) {
        when (event) {
            is Event.JobStarted -> jobs.started(event.job, event.activity)
            is Event.JobNote -> jobs.note(event.job, event.note, event.wait?.toMillis())
            is Event.JobDone -> {
                jobs.done(event.v1)
                accounts.done(event.v1)
                stores.done(event.v1)
                detailDone(event.v1)
            }

            is Event.Content -> content(event.content, event.view)
            is Event.Caption -> caption(event.text, event.view)
            is Event.CalendarWindow -> calendar.window(event.anchor, event.pages, event.morePast)
            is Event.CalendarBackfill -> calendar.backfill(event.page, event.entries, event.morePast, titles)
            is Event.Detail -> {
                detailPending -= event.v1.id
                titles.merge(detail = event.v1)
            }
            is Event.TitlesChanged -> titlesChanged()
            is Event.Progress -> titles.progress(event.id, event.progress, event.resume, event.episodes)
            is Event.LastAired -> titles.lastAired(event.v1)
            is Event.Episodes -> titles.episodes(event.id, event.rows)

            is Event.Status -> notices.status(event.v1)
            is Event.Toast -> notices.post(event.v1)

            is Event.Sync -> accounts.apply(event.v1)
            is Event.ViewerTitleLanguage -> accounts.viewerTitleLanguage(event.v1)
            is Event.Authorize -> accounts.authorize(SignInRequest(event.tracker, event.url, event.callbackScheme))
            is Event.DeviceCode -> accounts.deviceCode(event.v1)
            is Event.Conflicts -> accounts.conflicts(event.v1)
            is Event.Settings -> settings.apply(event.v1)
            is Event.Providers -> {
                val had = providers.rows.value.any { it.usable }
                providers.apply(event.v1)
                if (!had && event.v1.any { it.usable }) sourcesArrived()
            }
            is Event.ProviderNote -> providers.note(event.v1)
            Event.SourcesChanged -> providers.sourcesChanged()
            is Event.Vpn -> providers.guardSaid(event.v1)
            is Event.ProviderPreference -> providers.preference(event.v1)
            is Event.Stores -> stores.apply(event.v1)
            is Event.StorePreview -> stores.previewed(event.v1)
            is Event.StoreJoined -> stores.joined(event.store)
            is Event.InviteLink -> stores.invited(event.v1)
            is Event.Downloads -> downloads.apply(event.v1)
            is Event.Seeding -> downloads.seeding(event.v1)
            is Event.ShowSettings -> showSettings.apply(event.v1)
            is Event.NewEpisodes, is Event.WidgetSnapshot -> _answers.tryEmit(event)
            is Event.UpcomingNotices -> coming.apply(event.v1)
            is Event.Update -> updates.apply(event.v1)

            is Event.ArtworkReady -> artwork.ready(event.url, event.bytes)
            is Event.ImageFailed -> artwork.failed(event.url)

            is Event.PlayerOps, is Event.PlayerUi, is Event.PlaybackStaged, is Event.PlaybackEnded,
            Event.PlaybackKept -> onPlayback?.invoke(event)

            // Everything else is said in the events above, or has no screen yet.
            else -> {}
        }
    }

    private fun content(content: ContentList, view: ContentFor) {
        when (view) {
            ContentFor.Continue -> continuing.apply(content, titles)
            ContentFor.Seasonal -> seasonal.apply(content, titles)
            ContentFor.Calendar -> calendar.apply(content, titles)
            is ContentFor.Search ->
                if (search.takes(view.query, view.filter)) search.list.apply(content, titles)
            is ContentFor.Library -> library.segment(view.v1).apply(content, titles)
        }
    }

    private fun caption(text: String, view: ContentFor) {
        when (view) {
            ContentFor.Continue -> continuing.list.caption(text)
            ContentFor.Seasonal -> seasonal.caption(text)
            ContentFor.Calendar -> calendar.list.caption(text)
            is ContentFor.Search ->
                if (search.takes(view.query, view.filter)) search.list.caption(text)
            is ContentFor.Library -> library.segment(view.v1).caption(text)
        }
    }

    companion object {
        /** How long a list's answer stands: showing it again within this asks for nothing. */
        const val LIST_FRESH_MILLIS = 60_000L
    }
}

/** What failed, in the engine's words, which read whole on screen. */
val FfiException.reason: String
    get() = when (this) {
        is FfiException.Dirs -> reason
        is FfiException.Config -> reason
        is FfiException.Store -> reason
        is FfiException.Client -> reason
        is FfiException.Runtime -> reason
        is FfiException.Engine -> reason
    }
