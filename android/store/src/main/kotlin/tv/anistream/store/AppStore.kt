// The app's state: the engine's events applied, each by the feature model that owns it.

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
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
 */
class AppStore(
    private val scope: CoroutineScope,
    val jobs: JobsModel = JobsModel(scope = scope),
    val notices: NoticesModel = NoticesModel(),
    decoder: ArtworkDecoder = BitmapDecoder,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
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
    val artwork = ArtworkStore(scope, decoder) { url ->
        val client = client ?: return@ArtworkStore false
        client.send(Command.FetchArtwork(url))
        true
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

    // The engine

    /** Start the engine [launch] describes with [host], off the main thread, and read its events. */
    fun start(launch: EngineLaunch, host: FfiHost) {
        _phase.value = Phase.Starting
        scope.launch {
            try {
                val engine = withContext(Dispatchers.IO) { launch.start(host) }
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
        send(Command.LoadDetail(id))
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

    /** Take a title off Continue: the row goes at once, and the engine keeps it hidden. */
    fun hideFromContinue(id: AnilistId) {
        continuing.hide(id)
        send(Command.HideFromContinue(id))
    }

    /** Mark [episodes] of [id] watched, or not. */
    fun setWatched(id: AnilistId, episodes: List<String>, watched: Boolean = true) {
        send(Command.SetWatched(id, episodes, watched))
    }

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

    fun apply(events: List<Event>) {
        for (event in events) apply(event)
        lastApplied = clock()
        onApplied?.invoke(events)
    }

    fun apply(event: Event) {
        when (event) {
            is Event.JobStarted -> jobs.started(event.job, event.activity)
            is Event.JobNote -> jobs.note(event.job, event.note, event.wait?.toMillis())
            is Event.JobDone -> jobs.done(event.v1)

            is Event.Content -> content(event.content, event.view)
            is Event.Caption -> caption(event.text, event.view)
            is Event.CalendarWindow -> calendar.window(event.anchor, event.pages, event.morePast)
            is Event.CalendarBackfill -> calendar.backfill(event.page, event.entries, event.morePast, titles)
            is Event.Detail -> titles.merge(detail = event.v1)
            is Event.TitlesChanged -> titlesChanged()
            is Event.Progress -> titles.progress(event.id, event.progress, event.resume, event.episodes)
            is Event.LastAired -> titles.lastAired(event.v1)
            is Event.Episodes -> titles.episodes(event.id, event.rows)

            is Event.Status -> notices.status(event.v1)
            is Event.Toast -> notices.post(event.v1)

            is Event.Sync -> accounts.apply(event.v1)
            is Event.Stores -> stores.apply(event.v1)
            is Event.Downloads -> downloads.apply(event.v1)

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
