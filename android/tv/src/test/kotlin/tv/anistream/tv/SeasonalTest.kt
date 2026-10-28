package tv.anistream.tv

import java.time.LocalDate
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.MediaFormat
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.SearchSort
import tv.anistream.tv.screens.SeasonSort
import tv.anistream.tv.screens.SeasonTab
import tv.anistream.tv.screens.seasonFilter

/** Seasonal's tabs and what each asks the engine for. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SeasonalTest {
    private val context = RuntimeEnvironment.getApplication()
    @Test
    fun the_tabs_are_two_seasons_back_the_current_and_the_next() {
        val labels = SeasonTab.around(LocalDate.of(2026, 10, 20)).map { it.label(context) }
        assertEquals(listOf("Spring 2026", "Summer 2026", "Autumn 2026", "Winter 2027"), labels)
        val winter = SeasonTab.around(LocalDate.of(2027, 1, 5)).map { it.label(context) }
        assertEquals(listOf("Summer 2026", "Autumn 2026", "Winter 2027", "Spring 2027"), winter)
    }

    @Test
    fun the_current_season_popular_in_every_format_is_the_engines_own_list() {
        val now = SeasonTab(Season.FALL, 2026)
        assertNull(seasonFilter(now, now, SeasonSort.Popular, null))
        assertEquals(
            SearchQuery(sort = SearchSort.SCORE, season = Season.FALL, years = Bounds(min = 2026u, max = 2026u)),
            seasonFilter(now, now, SeasonSort.Score, null),
        )
        assertEquals(
            SearchQuery(
                sort = SearchSort.POPULARITY,
                formats = listOf(MediaFormat.MOVIE),
                season = Season.SUMMER,
                years = Bounds(min = 2026u, max = 2026u),
            ),
            seasonFilter(now.shift(-1), now, SeasonSort.Popular, MediaFormat.MOVIE),
        )
    }
}
