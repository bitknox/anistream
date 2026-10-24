package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.TitleProgress
import tv.anistream.testkit.Scene

/** Which watched episodes a title page folds, and how the row over them reads. */
class EpisodeFoldTest {
    private val scene = Scene.named("watching")

    /** 900001's page and its twelve episodes as the engine sends them. */
    private val entry: Entry = scene.events.filterIsInstance<Event.Detail>().first { it.v1.id == 900001u }.v1
    private val rows: List<EpisodeRow> = scene.events.filterIsInstance<Event.Episodes>().first { it.id == 900001u }.rows

    /** The first [done] watched, the next one [done] + 1. */
    private fun watched(done: Int): Pair<Entry, List<EpisodeRow>> =
        entry.copy(progress = TitleProgress(done = done.toUInt(), next = (done + 1).toUInt())) to
            rows.map { it.copy(completed = (it.number.toIntOrNull() ?: 0) <= done) }

    private fun folded(done: Int, columns: Int = 1): List<String> {
        val (title, episodes) = watched(done)
        return title.foldedEpisodes(episodes, columns).map { it.number }
    }

    @Test
    fun the_list_starts_at_the_last_episode_watched() {
        assertEquals((1..5).map { "$it" }, folded(6))
        // In two columns, an even number, so the last watched keeps its place in its pair.
        assertEquals((1..4).map { "$it" }, folded(6, columns = 2))
        assertEquals((1..6).map { "$it" }, folded(7, columns = 2))
    }

    @Test
    fun nothing_folds_with_nothing_watched_or_only_the_first_episode_before_the_last() {
        assertEquals(emptyList<String>(), folded(0))
        assertEquals(emptyList<String>(), folded(1))
        assertEquals(emptyList<String>(), folded(2))
        assertEquals(listOf("1", "2"), folded(3))
        assertEquals(listOf("1", "2"), folded(3, columns = 2))
        // Three before the last watched fold as two in columns, which start at the third.
        assertEquals(listOf("1", "2"), folded(4, columns = 2))
    }

    @Test
    fun an_episode_skipped_stops_the_fold() {
        val (title, episodes) = watched(6)
        val skipped = episodes.map { if (it.number == "3") it.copy(completed = false) else it }
        assertEquals(listOf("1", "2"), title.foldedEpisodes(skipped).map { it.number })
    }

    @Test
    fun the_row_names_the_range_it_folds() {
        val (title, episodes) = watched(6)
        assertEquals("Episodes 1 to 5 watched", watchedRangeWords(title.foldedEpisodes(episodes)))
        assertEquals("Episodes 1 to 2 watched", watchedRangeWords(episodes.take(2)))
    }
}
