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
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.Aired
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.BrowseRail
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.SearchSort
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.ShowSettingKey
import tv.anistream.engine.model.ShowSettingRow
import tv.anistream.engine.model.ShowSettingsView
import tv.anistream.engine.model.TitleAired
import tv.anistream.engine.model.TitleDetail
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
        studio = null, synopsis = "", coverUrl = "https://img.example/cover/$id.jpg", coverColor = null, bannerUrl = null,
        genres = listOf("Fantasy"), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = title, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
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
    fun continue_marks_a_dub_and_leaves_a_sub_unmarked() {
        val dub = entry(900001u, "Stardust Wayfarer").copy(
            progress = TitleProgress(done = 6u, next = 7u),
            resume = ResumePoint(position = 606.0, fraction = 606.0 / 1440.0),
            watchedIn = Translation.DUB,
        )
        val sub = entry(900002u, "The Library Above the Clouds").copy(
            progress = TitleProgress(done = 3u, next = 4u),
            watchedIn = Translation.SUB,
        )
        val unknown = entry(900003u, "Summer Wind Relay").copy(progress = TitleProgress(done = 3u, next = 4u))
        val fresh = entry(900004u, "Harbour Lights").copy(
            progress = TitleProgress(done = 8u, next = 9u),
            lastAired = Aired(episode = 9u, ago = 7_200),
            watchedIn = Translation.DUB,
        )
        store.apply(Event.Content(ContentFor.Continue, ContentList.Entries(listOf(dub, sub, unknown, fresh))))
        val queue = store.continuing.queue(store.titles.entries.value)
        assertEquals(
            listOf("Resume E7 · Dub · 10:06", "Start E4", "Start E4"),
            queue.inProgress.map { it.action },
        )
        assertEquals(listOf("Start E9 · Dub · out 2h ago"), queue.newThisWeek.map { it.action })
        assertEquals("Episode 7 · Dub", queue.inProgress[0].longEpisode)
        assertEquals("Episode 4", queue.inProgress[1].longEpisode)
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
    fun a_pages_seasons_stay_when_a_list_brings_its_title_again() {
        fun season(id: UInt) = LinkedTitle(
            id, "Season of $id", null, null, null, null, "TV", coverUrl = null, coverColor = null, season = null,
            year = null, listStatus = null,
        )
        val page = entry(900001u, "Stardust Wayfarer").copy(
            detail = TitleDetail(
                tags = emptyList(), schedule = emptyList(), relations = emptyList(),
                seasons = listOf(season(900001u), season(900004u)), recommendations = emptyList(),
                rankings = emptyList(), characters = emptyList(), staff = emptyList(),
            ),
        )
        store.apply(Event.Detail(page))
        // Continue loads again under the open page, as it does after every watch.
        store.apply(Event.Content(ContentFor.Continue, ContentList.Entries(listOf(entry(900001u, "Stardust Wayfarer")))))
        assertEquals(listOf(900001u, 900004u), store.titles[900001u]?.detail?.seasons?.map { it.id })
    }

    @Test
    fun a_search_takes_only_the_answer_to_what_was_asked_last() {
        store.search("hoshi")
        store.search("kumo")
        store.apply(Event.Content(ContentFor.Search(SearchQuery(text = "hoshi")), ContentList.Entries(listOf(entry(900001u, "Hoshi")))))
        assertEquals(ListState.Loading, store.search.list.state.value)
        store.apply(Event.Content(ContentFor.Search(SearchQuery(text = "kumo")), ContentList.Empty))
        assertEquals(ListState.Empty, store.search.list.state.value)
    }

    @Test
    fun a_search_takes_the_answer_to_its_text_without_the_spaces_around() {
        store.searchLive(SearchQuery(text = " kumo  "))
        assertEquals(listOf(Command.SearchLive(SearchQuery(text = "kumo"))), store.waiting)
        // Only spaces changed: nothing new to ask.
        store.searchLive("kumo ")
        assertEquals(1, store.waiting.size)
        store.apply(Event.Content(ContentFor.Search(SearchQuery(text = "kumo")), ContentList.Empty))
        assertEquals(ListState.Empty, store.search.list.state.value)
    }

    @Test
    fun a_blank_search_is_the_start_and_asks_nothing() {
        store.search("kumo")
        store.searchLive("  ")
        // Told all the same, so the engine drops the search it was waiting to make.
        assertEquals(listOf(Command.Search(SearchQuery(text = "kumo")), Command.SearchLive(SearchQuery())), store.waiting)
        assertTrue(store.search.status.value.atStart)
        assertNull(store.search.asked)
        assertEquals(ListState.Idle, store.search.list.state.value)
    }

    @Test
    fun one_typed_letter_waits_for_the_next_as_the_engine_does() {
        val kumo = SearchQuery(text = "kumo")
        store.searchLive(kumo)
        store.apply(Event.Content(ContentFor.Search(kumo), ContentList.Entries(listOf(entry(900001u, "Kumo")))))
        // Back to one letter: the start again, nothing asked, not the answer to "kumo".
        store.searchLive("k")
        assertTrue(store.search.status.value.atStart)
        assertNull(store.search.asked)
        assertFalse(store.search.status.value.pending)
        // One kana is a word.
        store.searchLive("く")
        assertFalse(store.search.status.value.atStart)
        assertEquals(SearchQuery(text = "く"), store.search.asked)
        // The search key searches for one letter all the same.
        store.search("k")
        assertEquals(SearchQuery(text = "k"), store.search.asked)
        assertFalse(store.search.status.value.atStart)
    }

    @Test
    fun one_letter_with_filters_keeps_the_filters_answer() {
        val mecha = SearchQuery(genres = listOf("Mecha"))
        store.searchLive(mecha)
        store.apply(Event.Content(ContentFor.Search(mecha), ContentList.Entries(listOf(entry(900001u, "Kumo")))))
        store.searchLive("s")
        assertFalse(store.search.status.value.atStart)
        assertEquals(mecha, store.search.asked)
        assertEquals(mecha, store.search.status.value.answered)
        assertFalse(store.search.status.value.pending)
    }

    @Test
    fun the_earlier_answer_is_known_for_what_it_answers() {
        val hoshi = SearchQuery(text = "hoshi")
        val kumo = SearchQuery(text = "kumo")
        store.searchLive(hoshi)
        assertTrue(store.search.status.value.pending)
        store.apply(Event.Content(ContentFor.Search(hoshi), ContentList.Entries(listOf(entry(900001u, "Hoshi")))))
        assertEquals(hoshi, store.search.status.value.answered)
        store.searchLive(kumo)
        // The earlier results stay, known to answer "hoshi", and no page of them is asked for.
        assertTrue(store.search.status.value.pending)
        assertEquals(hoshi, store.search.status.value.answered)
        assertEquals(listOf(900001u), store.search.list.state.value.ids)
        store.apply(Event.Content(ContentFor.Search(kumo), ContentList.Empty))
        assertEquals(kumo, store.search.status.value.answered)
        assertFalse(store.search.status.value.pending)
    }

    @Test
    fun try_again_shows_placeholders_and_asks_the_same_search() {
        store.search("kumo")
        store.apply(Event.Content(ContentFor.Search(SearchQuery(text = "kumo")), ContentList.Failed("AniList is unavailable")))
        store.retrySearch()
        assertEquals(ListState.Loading, store.search.list.state.value)
        assertEquals(Command.Search(SearchQuery(text = "kumo")), store.waiting.last())
    }

    @Test
    fun further_pages_go_after_what_is_there_and_a_failed_one_is_asked_again() {
        val browse = SearchQuery(genres = listOf("Fantasy"))
        store.search(browse)
        store.apply(Event.SearchPaging(browse, hasNext = true))
        store.apply(Event.Content(ContentFor.Search(browse), ContentList.Entries(listOf(entry(900001u, "Hoshi"), entry(900002u, "Kumo")))))
        store.searchMore()
        // Once at a time.
        store.searchMore()
        assertEquals(listOf(Command.Search(browse), Command.SearchMore(browse, 2u)), store.waiting)
        store.apply(Event.SearchMore(browse, 2u, emptyList(), hasNext = true))
        store.searchMore()
        assertEquals(Command.SearchMore(browse, 2u), store.waiting.last())
        store.apply(Event.SearchMore(browse, 2u, listOf(entry(900002u, "Kumo"), entry(900003u, "Tsuki")), hasNext = false))
        assertEquals(listOf(900001u, 900002u, 900003u), store.search.list.state.value.ids)
        assertFalse(store.search.status.value.hasNext)
        store.searchMore()
        assertEquals(3, store.waiting.size)
    }

    @Test
    fun the_rails_bring_their_titles_and_the_start_its_recents() {
        store.loadSearchStart()
        assertEquals(listOf(Command.LoadBrowse, Command.LoadSearchFacets, Command.LoadRecentSearches), store.waiting)
        val rail = BrowseRail("Trending now", null, SearchQuery(sort = SearchSort.TRENDING), listOf(entry(900004u, "Kaze")))
        store.apply(Event.Browse(listOf(rail)))
        store.apply(Event.RecentSearches(listOf(SearchQuery(text = "hoshi"))))
        assertEquals("Kaze", store.titles[900004u]?.title)
        assertEquals(listOf(SearchQuery(text = "hoshi")), store.search.status.value.recents)
        // The rails are here: the start asks only for its recents again.
        store.loadSearchStart()
        assertEquals(Command.LoadRecentSearches, store.waiting.last())
        assertEquals(5, store.waiting.size)
    }

    @Test
    fun seasonal_is_the_season_preset_of_search() {
        val query = seasonQuery(java.time.Instant.parse("2026-10-24T12:00:00Z"))
        assertEquals(Season.FALL, query.season)
        assertEquals(Bounds(2026u, 2026u), query.years)
        assertEquals("2026 · Fall", query.headline())
    }

    @Test
    fun a_new_title_language_asks_again_for_what_is_on_screen() {
        store.apply(Event.Content(ContentFor.Continue, ContentList.Empty))
        store.apply(Event.Content(ContentFor.Seasonal, ContentList.Entries(listOf(entry(900002u, "Kumo")))))
        store.search("kumo")
        assertEquals(listOf(Command.Search(SearchQuery(text = "kumo"))), store.waiting)
        store.apply(Event.TitlesChanged(TitleLanguage.ENGLISH))
        // What was answered is asked for again; the calendar and the library never were.
        assertEquals(
            listOf(Command.Search(SearchQuery(text = "kumo")), Command.LoadContinue, Command.LoadSeasonal, Command.Search(SearchQuery(text = "kumo"))),
            store.waiting,
        )
    }

    @Test
    fun a_shows_sheet_lands_by_show_and_its_changes_go_to_the_engine() {
        val sheet = ShowSettingsView(
            id = 900001u,
            rows = listOf(ShowSettingRow(ShowSettingKey.SKIP_OPENING, own = null, value = "on", default = "on")),
            remembered = emptyList(),
            source = null,
        )
        store.apply(Event.ShowSettings(sheet))
        assertEquals(sheet, store.showSettings.sheets.value[900001u])
        store.setShowSetting(900001u, ShowSettingKey.SKIP_OPENING, SettingValue.Bool(false))
        store.setShowSetting(900001u, ShowSettingKey.SKIP_OPENING, null)
        store.resetShowSettings(900001u)
        assertEquals(
            listOf(
                Command.SetShowSetting(900001u, ShowSettingKey.SKIP_OPENING, SettingValue.Bool(false)),
                Command.SetShowSetting(900001u, ShowSettingKey.SKIP_OPENING, null),
                Command.ResetShowSettings(900001u),
            ),
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
        store.apply(Event.JobNote(1u, Msg.Verbatim("waiting on AniList"), java.time.Duration.ofMillis(1_500)))
        assertEquals(now + 1_500, store.jobs.running.value.single().waitUntil)
        store.apply(Event.JobDone(1u))
        assertTrue(store.jobs.shown.value.isEmpty())
    }

    @Test
    fun a_notice_shows_until_dismissed_and_stays_in_the_log() {
        val notice = Notice(kind = NoticeKind.INFO, text = "Update ready: Hotaru Open Archive", hint = null)
        store.apply(listOf(Event.Toast(notice), Event.Status(Msg.Verbatim("2 changes waiting to sync"))))
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
