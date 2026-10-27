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
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.SourceNote
import tv.anistream.engine.model.SourceOption
import tv.anistream.engine.model.TitleProgress

/** The source picker's state, the rows it lists, and the next episode that a source lists. */
@OptIn(ExperimentalCoroutinesApi::class)
class SourcePickerTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), jobs = JobsModel(now = { 0L }), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun release(id: String) = SourceCandidate(
        id = id, providerId = "torrents", title = "Wayfarer $id", quality = 1080u, seeders = 4u, size = "1 GiB",
        dualAudio = false, dubbed = false, autoPick = false,
    )

    private fun option(
        id: String,
        covered: UInt = 6u,
        pinned: Boolean = false,
        note: SourceNote? = null,
        releases: List<SourceCandidate> = emptyList(),
        pinnedRelease: String? = null,
    ) = SourceOption(id, id, covered, 13u, pinned, note, releases, pinnedRelease)

    private fun row(number: String, unsourced: Boolean = false) = EpisodeRow(
        number = number, title = null, thumbnail = null, description = null, durationSecs = null, watched = 0.0,
        completed = false, kind = null, skippable = false, airsAt = null, unsourced = unsourced,
    )

    private fun watching(next: UInt, resume: Boolean = false): Entry = TestEntries.entry(900001u).copy(
        progress = TitleProgress(done = next - 1u, next = next),
        resume = if (resume) ResumePoint(position = 300.0, fraction = 0.3) else null,
    )

    @Test
    fun loading_lasts_until_the_options_come_and_the_old_ones_stay_after() {
        store.loadSourceOptions(900001u)
        assertEquals(listOf<Command>(Command.LoadSourceOptions(900001u)), store.waiting)
        assertTrue(store.sourceChoices.isLoading(900001u))
        store.apply(Event.SourceOptions(900001u, listOf(option("a"), option("b"))))
        assertFalse(store.sourceChoices.isLoading(900001u))
        assertEquals(listOf("a", "b"), store.sourceChoices.options.value[900001u]?.map { it.providerId })
        store.loadSourceOptions(900001u)
        assertFalse("the last listing stays up while the next comes", store.sourceChoices.isLoading(900001u))
    }

    @Test
    fun a_closed_picker_stops_waiting() {
        store.loadSourceOptions(900001u)
        store.sourceChoices.giveUp(900001u)
        assertFalse(store.sourceChoices.isLoading(900001u))
    }

    @Test
    fun a_pick_is_shown_at_once_and_sent_to_the_engine() {
        store.apply(Event.SourceOptions(900001u, listOf(option("a"), option("b", pinned = true))))
        store.chooseSource(900001u, "a")
        assertEquals(listOf<Command>(Command.SetProviderPreference(900001u, "a")), store.waiting)
        assertEquals(listOf(true, false), store.sourceChoices.options.value[900001u]?.map { it.pinned })
        store.chooseSource(900001u, null)
        assertEquals(listOf(false, false), store.sourceChoices.options.value[900001u]?.map { it.pinned })
        assertEquals(Command.SetProviderPreference(900001u, null), store.waiting.last())
    }

    @Test
    fun a_release_is_pinned_with_its_source() {
        val batch = release("batch")
        store.apply(Event.SourceOptions(900001u, listOf(option("a"), option("torrents", releases = listOf(batch, release("one"))))))
        store.chooseRelease(900001u, "torrents", batch)
        assertEquals(Command.PinRelease(900001u, "torrents", "batch", "Wayfarer batch"), store.waiting.last())
        val torrents = store.sourceChoices.options.value[900001u]!!.last()
        assertTrue(torrents.pinned)
        assertEquals("batch", torrents.pinnedRelease)
    }

    @Test
    fun a_pin_the_engine_could_not_use_is_said_once() {
        store.apply(Event.PinUnavailable(900001u, "a", "Hotaru"))
        val first = store.sourceChoices.pinNotice.value
        assertEquals("Hotaru", first?.name)
        // The same pin in the same visit is not said again.
        store.apply(Event.PinUnavailable(900001u, "a", "Hotaru"))
        assertEquals(first, store.sourceChoices.pinNotice.value)
        store.sourceChoices.told(first!!)
        assertNull(store.sourceChoices.pinNotice.value)
        // A new choice may meet it again, and says so again.
        store.chooseSource(900001u, "a")
        store.apply(Event.PinUnavailable(900001u, "a", "Hotaru"))
        assertNotEquals(first, store.sourceChoices.pinNotice.value)
        assertEquals("Hotaru", store.sourceChoices.pinNotice.value?.name)
    }

    @Test
    fun the_rows_are_automatic_then_each_source_with_releases_under_a_source_that_has_several() {
        val rows = pickerRows(
            listOf(
                option("a", covered = 13u),
                option("torrents", releases = listOf(release("batch"), release("one")), pinned = true, pinnedRelease = "one"),
                option("old", covered = 0u, note = SourceNote.Off),
            ),
        )
        assertEquals(
            listOf("automatic", "source/a", "source/torrents", "releases/torrents", "release/torrents/batch", "release/torrents/one", "source/old"),
            rows.map { it.key },
        )
        assertFalse((rows[0] as PickerRow.Automatic).chosen)
        // The pinned release holds the check, not its source.
        assertFalse((rows[2] as PickerRow.Source).chosen)
        assertTrue((rows[5] as PickerRow.Release).chosen)
        assertTrue((rows[6] as PickerRow.Source).dimmed)
    }

    @Test
    fun automatic_is_chosen_when_nothing_is_pinned_and_a_removed_source_cannot_be_picked() {
        val rows = pickerRows(listOf(option("a"), option("gone", note = SourceNote.Gone, pinned = true)))
        assertFalse((rows[0] as PickerRow.Automatic).chosen)
        assertFalse((rows[2] as PickerRow.Source).selectable)
        assertTrue((pickerRows(listOf(option("a")))[0] as PickerRow.Automatic).chosen)
        // Nothing has come yet: only Automatic, and not yet chosen.
        assertEquals(1, pickerRows(null).size)
        assertFalse((pickerRows(null)[0] as PickerRow.Automatic).chosen)
    }

    @Test
    fun the_main_button_skips_episodes_no_source_lists() {
        val rows = listOf(row("6"), row("7", unsourced = true), row("8", unsourced = true), row("9"))
        assertEquals(PlayableNext(9u, false), watching(next = 7u).playableNext(rows))
        assertEquals(PlayableNext(6u, false), watching(next = 6u).playableNext(rows))
    }

    @Test
    fun with_nothing_left_that_plays_it_says_so() {
        val rows = listOf(row("6"), row("7", unsourced = true), row("8", unsourced = true))
        assertEquals(PlayableNext(7u, true), watching(next = 7u).playableNext(rows))
    }

    @Test
    fun a_part_watched_episode_is_resumed_and_an_unloaded_table_changes_nothing() {
        val rows = listOf(row("7", unsourced = true), row("8"))
        assertEquals(7u, watching(next = 7u, resume = true).playableNext(rows).episode)
        assertEquals(PlayableNext(7u, false), watching(next = 7u).playableNext(null))
    }

    @Test
    fun continue_rows_follow_it() {
        val rows = listOf(row("7", unsourced = true), row("8"))
        val item = ContinueItem.of(watching(next = 7u), rows)
        assertEquals(8u, item.episode)
        assertTrue(item.playable)
        val none = ContinueItem.of(watching(next = 7u), listOf(row("7", unsourced = true)))
        assertTrue(none.unsourced)
        assertFalse(none.playable)
        // The launcher's shortcut skips a title with nothing a source lists.
        val entry = watching(next = 7u)
        val queue = ContinueQueue.of(listOf(entry.id), mapOf(entry.id to entry), mapOf(entry.id to listOf(row("7", unsourced = true))))
        assertNull(queue.nextUp)
    }

    @Test
    fun an_episode_no_source_lists_is_not_downloaded() {
        store.apply(Event.Episodes(900001u, listOf(row("7", unsourced = true), row("8"))))
        store.downloadEpisode(900001u, "7")
        assertTrue(store.waiting.none { it is Command.DownloadEpisode })
        store.downloadEpisode(900001u, "8")
        assertEquals(Command.DownloadEpisode(900001u, "8"), store.waiting.last())
    }
}
