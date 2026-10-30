package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.Entry

/** The calendar's window: how far it reaches, and the pages that come in at either end. */
class CalendarWindowTest {
    private val titles = TitleCache()
    private fun row(id: Long, at: Long) = Entry(id = id.toUInt(), title = "Stardust $id", airingIn = at, airingEpisode = 1u)
    private fun model() = CalendarModel().also { it.window(1_000L, 3u, true, 100L, 5_000L, 3u, true) }

    @Test
    fun the_window_carries_the_floor_and_the_ceiling_the_engine_says() {
        val w = model().window.value
        assertEquals(100L, w.floor)
        assertEquals(5_000L, w.ceiling)
        assertTrue(w.reaches(100L) && w.reaches(5_000L))
        assertFalse(w.reaches(99L))
        assertFalse(w.reaches(5_001L))
    }

    @Test
    fun an_earlier_page_is_asked_for_once_until_it_answers() {
        val m = model()
        assertNotNull(m.askPast())
        assertNull(m.askPast())
        m.backfill(3u, listOf(row(1, -10)), false, titles)
        assertEquals(1, m.window.value.rows.size)
        assertFalse(m.window.value.morePast)
        assertNull(m.askPast())
    }

    @Test
    fun a_failed_earlier_page_keeps_what_is_there_and_waits_for_try_again() {
        val m = model()
        m.askPast()
        m.backfill(null, emptyList(), true, titles)
        assertTrue(m.window.value.pastFailed)
        assertNull(m.askPast())
        assertNotNull(m.retryPast())
    }

    @Test
    fun a_later_page_follows_the_ones_loaded_and_the_page_after_it_follows_that() {
        val m = model()
        assertEquals(1_000L to 4u, m.askAhead())
        m.ahead(4u, listOf(row(2, 50)), true, titles)
        assertEquals(1, m.window.value.rows.size)
        assertEquals(1_000L to 5u, m.askAhead())
        m.ahead(5u, emptyList(), false, titles)
        assertNull(m.askAhead())
    }

    @Test
    fun a_failed_later_page_is_asked_again_only_on_try_again() {
        val m = model()
        m.askAhead()
        m.ahead(null, emptyList(), true, titles)
        assertTrue(m.window.value.aheadFailed)
        assertNull(m.askAhead())
        assertEquals(1_000L to 4u, m.askAhead(retry = true))
        assertFalse(m.window.value.aheadFailed)
    }

    @Test
    fun a_refreshed_window_clears_what_was_loading_or_failed() {
        val m = model()
        m.askAhead()
        m.ahead(null, emptyList(), true, titles)
        m.window(2_000L, 3u, true, 200L, 6_000L, 3u, true)
        val w = m.window.value
        assertFalse(w.aheadFailed || w.loadingAhead || w.loadingPast || w.pastFailed)
        assertEquals(6_000L, w.ceiling)
    }
}
