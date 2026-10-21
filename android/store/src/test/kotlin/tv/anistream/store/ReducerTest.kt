package tv.anistream.store

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.model.Aired
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.SearchFilters
import tv.anistream.engine.model.TitleAired
import tv.anistream.engine.model.TitleLanguage
import tv.anistream.engine.model.TitleProgress

/** The store fed scripted event lists, as the engine sends them: no engine, no native code. */
@OptIn(ExperimentalCoroutinesApi::class)
class ReducerTest {
    private val main = UnconfinedTestDispatcher()
    private var now = 0L
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        val scope = TestScope(main)
        store = AppStore(
            scope = scope,
            jobs = JobsModel(now = { now }),
            decoder = { _, _ -> null },
            clock = { now },
        )
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun entry(id: UInt, title: String) = Entry(
        id = id, title = title, secondary = null, format = "TV", episodes = 12u, year = 2026u, score = null,
        studio = null, synopsis = "", coverUrl = "https://img.example/cover/$id.jpg", bannerUrl = null,
        genres = listOf("Fantasy"), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = title, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
    )

    @Test
    fun a_list_lands_in_its_own_model_with_its_rows_in_the_title_cache() {
        store.loadSeasonal()
        assertEquals(ListState.Loading, store.seasonal.state.value)
        store.apply(
            listOf(
                Event.Caption(ContentFor.Seasonal, "Fall 2026"),
                Event.Content(ContentFor.Seasonal, ContentList.Entries(listOf(entry(900002u, "Kumo"), entry(900006u, "Natsukaze")))),
            ),
        )
        assertEquals(ListState.Loaded(listOf(900002u, 900006u)), store.seasonal.state.value)
        assertEquals("Fall 2026", store.seasonal.caption.value)
        assertEquals("Natsukaze", store.titles[900006u]?.title)
        // Nothing else moved.
        assertEquals(ListState.Idle, store.continuing.list.state.value)
    }

    @Test
    fun commands_sent_while_the_engine_starts_wait_for_it() {
        store.loadContinue()
        store.loadSeasonal()
        assertEquals(listOf(Command.LoadContinue, Command.LoadSeasonal), store.waiting)
        // Answered within the minute: asking again costs nothing.
        store.apply(Event.Content(ContentFor.Continue, ContentList.Empty))
        store.loadContinue()
        assertEquals(2, store.waiting.size)
        store.loadContinue(force = true)
        assertEquals(3, store.waiting.size)
    }

    @Test
    fun continue_says_what_each_row_plays() {
        val resumed = entry(900001u, "Stardust Wayfarer").copy(
            progress = TitleProgress(done = 6u, next = 7u),
            resume = ResumePoint(position = 606.0, fraction = 606.0 / 1440.0),
        )
        val next = entry(900002u, "The Library Above the Clouds").copy(progress = TitleProgress(done = 3u, next = 4u))
        val fresh = entry(900006u, "Summer Wind Relay").copy(
            progress = TitleProgress(done = 8u, next = 9u),
            lastAired = Aired(episode = 9u, ago = 7_200),
        )
        store.apply(Event.Content(ContentFor.Continue, ContentList.Entries(listOf(resumed, next, fresh))))
        val queue = store.continuing.queue(store.titles.entries.value)
        assertEquals(listOf("Resume E7 · 10:06", "Start E4"), queue.inProgress.map { it.action })
        assertEquals(14, queue.inProgress.first().minutesLeft)
        assertEquals(listOf("Start E9 · out 2h ago"), queue.newThisWeek.map { it.action })

        store.hideFromContinue(900002u)
        assertEquals(listOf(900001u), store.continuing.queue(store.titles.entries.value).inProgress.map { it.id })
    }

    @Test
    fun a_progress_or_a_broadcast_moves_the_title_everywhere() {
        store.apply(Event.Content(ContentFor.Seasonal, ContentList.Entries(listOf(entry(900002u, "Kumo")))))
        store.apply(
            listOf(
                Event.LastAired(listOf(TitleAired(900002u, Aired(episode = 5u, ago = 60)))),
                Event.Progress(900002u, TitleProgress(done = 5u, next = 6u), null, emptyList()),
            ),
        )
        val title = store.titles[900002u]!!
        assertEquals(5u, title.lastAired?.episode)
        assertEquals(6u, title.progress?.next)
        // A list arriving again keeps the broadcast its follow-up brought.
        store.apply(Event.Content(ContentFor.Seasonal, ContentList.Entries(listOf(entry(900002u, "Kumo")))))
        assertEquals(5u, store.titles[900002u]?.lastAired?.episode)
    }

    @Test
    fun a_search_takes_only_the_answer_to_what_was_asked_last() {
        store.search("hoshi")
        store.search("kumo")
        val early = ContentFor.Search("hoshi", SearchFilters())
        store.apply(Event.Content(early, ContentList.Entries(listOf(entry(900001u, "Hoshi")))))
        assertEquals(ListState.Loading, store.search.list.state.value)
        store.apply(Event.Content(ContentFor.Search("kumo", SearchFilters()), ContentList.Empty))
        assertEquals(ListState.Empty, store.search.list.state.value)
    }

    @Test
    fun a_new_title_language_asks_again_for_what_is_on_screen() {
        store.apply(Event.Content(ContentFor.Continue, ContentList.Empty))
        store.apply(Event.Content(ContentFor.Seasonal, ContentList.Entries(listOf(entry(900002u, "Kumo")))))
        store.search("kumo")
        assertEquals(listOf(Command.Search("kumo", SearchFilters())), store.waiting)
        store.apply(Event.TitlesChanged(TitleLanguage.ENGLISH))
        // What was answered is asked for again; the calendar and the library never were.
        assertEquals(
            listOf(Command.Search("kumo", SearchFilters()), Command.LoadContinue, Command.LoadSeasonal, Command.Search("kumo", SearchFilters())),
            store.waiting,
        )
    }

    @Test
    fun the_library_fills_by_segment() {
        store.apply(Event.Content(ContentFor.Library(LibrarySegment.PLANNING), ContentList.Failed("signed out")))
        assertEquals(ListState.Failed("signed out"), store.library.segment(LibrarySegment.PLANNING).state.value)
        assertEquals(ListState.Idle, store.library.segment(LibrarySegment.WATCHING).state.value)
    }

    @Test
    fun a_job_shows_only_once_it_has_run_600_ms() {
        store.apply(Event.JobStarted(1u, "loading this season"))
        store.jobs.refresh()
        assertTrue(store.jobs.shown.value.isEmpty())
        assertFalse(store.isSettled)
        now += 600
        store.jobs.refresh()
        assertEquals(listOf("loading this season"), store.jobs.shown.value.map { it.activity })
        store.apply(Event.JobNote(1u, "waiting on AniList", java.time.Duration.ofMillis(1_500)))
        assertEquals(now + 1_500, store.jobs.running.value.single().waitUntil)
        store.apply(Event.JobDone(1u))
        assertTrue(store.jobs.shown.value.isEmpty())
    }

    @Test
    fun a_notice_shows_until_dismissed_and_stays_in_the_log() {
        val notice = Notice(kind = NoticeKind.INFO, text = "Update ready: Hotaru Open Archive", hint = null)
        store.apply(listOf(Event.Toast(notice), Event.Status("2 changes waiting to sync")))
        val item = store.notices.current.value!!
        assertEquals("Update ready: Hotaru Open Archive", item.notice.text)
        assertEquals("2 changes waiting to sync", store.notices.status.value)
        store.notices.dismiss(item)
        assertNull(store.notices.current.value)
        assertEquals(1, store.notices.log.value.size)
    }

    @Test
    fun art_is_asked_for_once_and_a_failure_waits_before_it_is_asked_again() {
        // No engine yet: nothing to ask, so a later ask tries again.
        store.artwork.request("https://img.example/cover/900001.jpg", 300)
        assertFalse(store.artwork.isPending("https://img.example/cover/900001.jpg"))
        store.apply(Event.ImageFailed("https://img.example/cover/900001.jpg", offline = true))
        assertFalse(store.artwork.hasPending)
    }
}
