package tv.anistream.store

import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.NoticeReason
import tv.anistream.engine.model.UpcomingNotice
import tv.anistream.engine.model.UpcomingNotices
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
    fun the_engines_week_lands_in_its_model_and_is_asked_for() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        assertNull(store.coming.week.value)
        store.loadComing()
        assertEquals(listOf(tv.anistream.engine.model.Command.UpcomingNotices), store.waiting)
        val week = UpcomingNotices(
            enabled = false,
            generatedAt = 1_792_000_000,
            airings = listOf(UpcomingNotice(900002u, "The Library Above the Clouds", 6u, 1_792_163_730, NoticeReason.SWITCHED_ON, 6u, notifies = true)),
        )
        store.apply(Event.UpcomingNotices(week))
        assertEquals(week, store.coming.week.value)
    }

    @Test
    fun the_footer_names_what_waits_for_its_dub_and_what_is_switched_off() {
        fun row(id: UInt, title: String, reason: NoticeReason, translation: Translation = Translation.SUB) =
            UpcomingNotice(id, title, 1u, 1_792_163_730, reason, 1u, notifies = reason != NoticeReason.SWITCHED_OFF, translation = translation)
        val week = UpcomingNotices(
            enabled = true,
            generatedAt = 0,
            airings = listOf(
                row(1u, "Summer Wind Relay", NoticeReason.WATCHING, Translation.DUB),
                row(1u, "Summer Wind Relay", NoticeReason.WATCHING, Translation.DUB),
                row(2u, "Amber Forest", NoticeReason.SWITCHED_OFF),
                row(3u, "The Library Above the Clouds", NoticeReason.SWITCHED_ON),
            ),
        )
        assertEquals(listOf(1u, 1u, 3u), ComingModel.listed(week).map { it.id })
        assertEquals(
            "Summer Wind Relay notifies when its dub is out. Amber Forest is off in its own settings. A show opens its settings.",
            ComingModel.footer(week),
        )
        assertEquals("Nothing airs this week for the shows followed.", ComingModel.footer(UpcomingNotices(enabled = true, generatedAt = 0)))
    }
}
