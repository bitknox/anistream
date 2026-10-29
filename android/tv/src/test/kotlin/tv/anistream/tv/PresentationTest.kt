package tv.anistream.tv

import java.time.Clock
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.EpisodeAiring
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.TitleDetail
import tv.anistream.engine.model.TitleProgress
import tv.anistream.store.AppClock
import tv.anistream.tv.screens.SearchKeys
import tv.anistream.tv.screens.dayLabel
import tv.anistream.tv.screens.seasonLabel

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PresentationTest {
    private val context = RuntimeEnvironment.getApplication()
    /** The brief's now: Tuesday 20 October 2026, 19:30 in Copenhagen. */
    private val now = Instant.parse("2026-10-20T17:30:00Z")

    @Before
    fun setUp() {
        AppClock.clock = Clock.fixed(now, ZoneId.of("Europe/Copenhagen"))
    }

    @After
    fun tearDown() {
        AppClock.clock = Clock.systemDefaultZone()
    }

    private fun entry(
        progress: TitleProgress? = null,
        resume: ResumePoint? = null,
        detail: TitleDetail? = null,
    ) = Entry(id = 900001u, title = "Stardust Wayfarer", progress = progress, resume = resume, detail = detail)

    @Test
    fun the_amber_action_resumes_an_episode_left_part_way() {
        val action = PlayAction.of(entry(TitleProgress(6u, 7u), ResumePoint(606.0, 0.42)))
        assertEquals(7u, action.episode)
        assertTrue(action.playable)
        assertEquals("Resume episode 7", action.label(context))
    }

    @Test
    fun a_dub_is_marked_on_the_action_and_a_sub_is_not() {
        val progress = TitleProgress(6u, 7u)
        val resume = ResumePoint(606.0, 0.42)
        assertEquals("Resume episode 7 · Dubbed", PlayAction.of(entry(progress, resume).copy(watchedIn = Translation.DUB)).label(context))
        assertEquals("Resume episode 7", PlayAction.of(entry(progress, resume).copy(watchedIn = Translation.SUB)).label(context))
        assertEquals("Play episode 7 · Dubbed", PlayAction.of(entry(progress).copy(watchedIn = Translation.DUB)).label(context))
    }

    @Test
    fun a_finished_show_says_watch_again_and_plays_episode_one() {
        val action = PlayAction.of(entry(TitleProgress(12u, 1u)).copy(episodes = 12u))
        assertEquals("Watch again", action.label(context))
        assertEquals(1u, action.episode)
        assertTrue(action.playable)
    }

    @Test
    fun a_title_never_started_plays_its_first_episode() {
        assertEquals("Play episode 1", PlayAction.of(entry()).label(context))
    }

    @Test
    fun an_episode_still_to_air_says_the_day_and_waits() {
        val thursday = Instant.parse("2026-10-22T15:00:00Z").epochSecond
        val detail = TitleDetail(schedule = listOf(EpisodeAiring(episode = 10u, airsAt = thursday)))
        val action = PlayAction.of(entry(TitleProgress(9u, 10u), detail = detail), now.epochSecond)
        assertFalse(action.playable)
        assertEquals("Play episode 10 (out Thursday)", action.label(context, now))
    }

    @Test
    fun the_metadata_line_is_in_sentence_case() {
        val entry = Entry(
            id = 900001u,
            title = "Stardust Wayfarer",
            format = "TV",
            season = Season.SUMMER,
            year = 2026u,
            episodes = 24u,
            score = 79u,
            studio = "Atelier Hotaru",
        )
        assertEquals("TV · Summer 2026 · 24 episodes · 79% · Atelier Hotaru", entry.metadataLine(context))
    }

    @Test
    fun spoilers_leave_the_synopsis() {
        assertEquals("A traveller and a lamp.", synopsisWithoutSpoilers("A traveller and a lamp.~!The lamp is her sister.!~"))
    }

    @Test
    fun seasons_and_days_read_as_the_rows_have_them() {
        assertEquals("Autumn 2026", seasonLabel(context, LocalDate.of(2026, 10, 20)))
        assertEquals("Winter 2027", seasonLabel(context, LocalDate.of(2026, 12, 3)))
        val today = LocalDate.of(2026, 10, 20)
        assertEquals("Today", dayLabel(context, today, today))
        assertEquals("Wed 21", dayLabel(context, today.plusDays(1), today))
    }

    @Test
    fun our_keyboard_types_deletes_and_clears() {
        var text = ""
        "hoshi".forEach { text = SearchKeys.type(text, SearchKeys.Key.Character(it)) }
        assertEquals("hoshi", text)
        text = SearchKeys.type(text, SearchKeys.Key.Space)
        text = SearchKeys.type(text, SearchKeys.Key.Space)
        assertEquals("hoshi ", text)
        assertEquals("hoshi", SearchKeys.type(text, SearchKeys.Key.Delete))
        assertEquals("", SearchKeys.type(text, SearchKeys.Key.Clear))
        assertEquals("", SearchKeys.type("", SearchKeys.Key.Space))
        assertEquals(36, SearchKeys.characters.size)
        assertEquals('0', SearchKeys.characters.last())
    }

    @Test
    fun the_synopsis_keeps_the_engines_line_and_paragraph_breaks() {
        assertEquals(
            "First line.\nSecond line.\n\nNew paragraph.",
            synopsisWithoutSpoilers("First line. \nSecond  line.\n\n\n~!Hidden.!~New paragraph. "),
        )
    }
}
