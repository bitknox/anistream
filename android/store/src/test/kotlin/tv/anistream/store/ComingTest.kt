package tv.anistream.store

import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.model.ComingEpisode
import tv.anistream.engine.model.Event

class ComingTest {
    private val zone = ZoneId.of("Europe/Copenhagen")

    /** Tuesday 20 October 2026, 19:30 in Copenhagen. */
    private val now = Instant.parse("2026-10-20T17:30:00Z")

    @Test
    fun a_time_coming_up_says_today_tomorrow_or_its_weekday() {
        assertEquals("Today 22:00", AirTime.coming(Instant.parse("2026-10-20T20:00:00Z"), now, zone))
        assertEquals("Tomorrow 16:30", AirTime.coming(Instant.parse("2026-10-21T14:30:00Z"), now, zone))
        assertEquals("Thursday 16:30", AirTime.coming(Instant.parse("2026-10-22T14:30:00Z"), now, zone))
        // Past midnight here, though not yet in UTC.
        assertEquals("Tomorrow 00:30", AirTime.coming(Instant.parse("2026-10-20T22:30:00Z"), now, zone))
    }

    @Test
    fun the_engines_list_lands_in_its_model_and_is_asked_for() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        assertNull(store.coming.episodes.value)
        store.loadComing()
        assertEquals(listOf(tv.anistream.engine.model.Command.ComingEpisodes), store.waiting)
        val listed = listOf(ComingEpisode(900002u, "The Library Above the Clouds", 6u, 1_792_163_730, switchedOn = true))
        store.apply(Event.ComingEpisodes(listed))
        assertEquals(listed, store.coming.episodes.value)
    }
}
