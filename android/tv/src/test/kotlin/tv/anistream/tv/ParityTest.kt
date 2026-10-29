package tv.anistream.tv

import java.time.LocalDate
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.MediaFormat
import tv.anistream.engine.model.Pick
import tv.anistream.engine.model.RememberedSubtitles
import tv.anistream.engine.model.RememberedTrack
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.TitleLanguage
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState
import tv.anistream.tv.screens.SearchFilters
import tv.anistream.tv.screens.TitleChoice
import tv.anistream.tv.screens.updateLine
import tv.anistream.tv.ui.subtitleWords
import tv.anistream.tv.ui.trackWords

/** The words and rules behind Search's filters, the Titles row, the update and the show's tracks. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ParityTest {
    private val context = RuntimeEnvironment.getApplication()
    @Test
    fun a_season_keeps_only_while_the_years_are_one() {
        val one = SearchQuery(years = Bounds(2021u, 2021u), season = Season.SPRING)
        assertEquals(2021u, SearchFilters.singleYear(one))
        assertEquals(Season.SPRING, SearchFilters.settle(one).season)
        val range = one.copy(years = Bounds(2019u, 2021u))
        assertNull(SearchFilters.singleYear(range))
        assertNull(SearchFilters.settle(range).season)
    }

    @Test
    fun the_year_panel_runs_from_next_year_down() {
        val years = SearchFilters.years(LocalDate.of(2026, 10, 25))
        assertEquals(2027, years.first())
        assertEquals(SearchFilters.FIRST_YEAR, years.last())
    }

    @Test
    fun center_steps_a_genre_in_then_out_then_off() {
        assertEquals(Pick.INCLUDE, SearchFilters.next(Pick.OFF))
        assertEquals(Pick.EXCLUDE, SearchFilters.next(Pick.INCLUDE))
        assertEquals(Pick.OFF, SearchFilters.next(Pick.EXCLUDE))
        assertNull(SearchFilters.pickWords(context, Pick.OFF))
    }

    @Test
    fun the_chips_name_what_is_set() {
        assertEquals("Genres", SearchFilters.genresChip(context, SearchQuery()))
        assertEquals("3 genres", SearchFilters.genresChip(context, SearchQuery(genres = listOf("Action", "Drama"), genresExcluded = listOf("Horror"))))
        assertEquals("Format", SearchFilters.formatChip(context, SearchQuery()))
        val three = SearchQuery(formats = listOf(MediaFormat.TV, MediaFormat.MOVIE, MediaFormat.OVA))
        assertEquals("3 formats", SearchFilters.formatChip(context, three))
    }

    @Test
    fun filters_with_nothing_typed_are_a_browse_not_the_start() {
        val browse = SearchQuery(genres = listOf("Action"))
        assertFalse(browse.isBlank())
        assertTrue(browse.isBrowse())
        assertTrue(browse.cleared().copy(sort = null).isBlank())
    }

    @Test
    fun the_titles_row_says_what_each_choice_does() {
        assertEquals("English while signed out of AniList", TitleChoice.line(context, TitleChoice.AniList, signedIn = false, viewer = null))
        assertEquals("Romaji, from your AniList account", TitleChoice.line(context, TitleChoice.AniList, signedIn = true, viewer = TitleLanguage.ROMAJI))
        assertEquals(TitleChoice.AniList, TitleChoice.of(null))
    }

    @Test
    fun an_update_needs_room_for_the_apk_twice_and_some_to_spare() {
        val mb = 1024L * 1024
        assertTrue(roomForUpdate(50 * mb, 200 * mb))
        assertFalse(roomForUpdate(50 * mb, 150 * mb))
    }

    @Test
    fun the_update_row_says_where_the_update_is() {
        val offer = UpdateOffer("2.1.0", "https://updates.example/a.apk", "ab".repeat(32), 0)
        assertEquals("Available", updateLine(context, UpdateState.Available(offer)))
        assertEquals("Downloading 40%", updateLine(context, UpdateState.Downloading(offer, 40, 100)))
        assertEquals("", updateLine(context, UpdateState.None))
    }

    @Test
    fun a_track_is_named_by_its_language_then_its_own_title() {
        assertEquals("English (Signs)", trackWords(context, RememberedTrack("en", "English Signs", false, false)))
        assertEquals("Unnamed track", trackWords(context, RememberedTrack(null, "", false, false)))
        assertEquals("Off", subtitleWords(context, RememberedSubtitles.Off))
    }
}
