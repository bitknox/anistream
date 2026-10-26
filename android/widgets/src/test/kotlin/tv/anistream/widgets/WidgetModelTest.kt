package tv.anistream.widgets

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.widgets.Fixtures.NOW
import tv.anistream.widgets.Fixtures.ZONE
import tv.anistream.widgets.Fixtures.at

/** What the widgets make of the record: each state the boards draw, from either shape of it. */
class WidgetModelTest {
    private fun continuing(data: tv.anistream.engine.model.WidgetData?, now: java.time.Instant = NOW) =
        WidgetModel.continuing(data?.let(Record::of), now, ZONE)

    private fun airing(data: tv.anistream.engine.model.WidgetData?, now: java.time.Instant = NOW) =
        WidgetModel.airing(data?.let(Record::of), now, ZONE)

    @Test
    fun a_second_shape_record_is_read_whole() {
        val record = Record.of(Fixtures.record())
        assertEquals(true, record.signedIn)
        assertEquals(7, record.days)
        assertEquals(14u, record.continuing.first().minutesLeft)
        assertEquals(true, record.continuing.first().resume)
        assertEquals("Hoshikuzu no Tabibito", record.nextAiring?.title)
        // Today and the week after, in the order they air.
        assertEquals(Fixtures.today.size + Fixtures.nextDays.size, record.airing.size)
        assertEquals(record.airing.sortedBy { it.airsAt }, record.airing)
        assertEquals("Dub", record.airing.first { it.airsAt == at(20, 21, 0) }.translation)
        assertEquals(true, record.airing.first().watched)
    }

    @Test
    fun a_first_shape_record_leaves_unknown_what_it_does_not_say() {
        val record = Record.of(Fixtures.firstShape())
        assertNull(record.signedIn)
        assertEquals(1, record.days)
        assertNull(record.continuing.first().resume)
        assertNull(record.airing.first().watched)
        // Progress still comes from how far in it was left.
        val view = WidgetModel.continuing(record, NOW, ZONE) as ContinueView.Shows
        assertEquals("E7 of 12", view.shows.first().line)
        assertEquals(0.42f, view.shows.first().progress!!, 0.001f)
    }

    @Test
    fun continue_says_the_time_left_and_starts_what_was_not_begun() {
        val view = continuing(Fixtures.record()) as ContinueView.Shows
        // Hagane no Niwashi is watched dubbed: the engine's mark follows its episode.
        assertEquals(listOf("E7 · 14 min left", "E2 · Dub · 6 min left", "Start E4"), view.shows.map { it.line })
        assertEquals(0.75f, view.shows[1].progress!!, 0.001f)
        assertNull("no bar for an episode not begun", view.shows[2].progress)
        assertNull(view.updated)
        // The wide art is the banner; the rows' is the poster; a title with no banner uses its poster.
        assertEquals(Fixtures.banner(Fixtures.HOSHIKUZU), view.shows[0].wideUrl)
        assertEquals(Fixtures.poster(Fixtures.KUMO), view.shows[2].wideUrl)
        assertEquals(0xFF1A1F3DL, view.shows[0].plate)
    }

    @Test
    fun continue_says_when_its_record_is_from_once_a_day_old() {
        val sunday = at(18, 20, 15)
        val stale = continuing(Fixtures.record(generatedAt = sunday)) as ContinueView.Shows
        assertEquals("Updated Sunday", stale.updated)
        // The content stays.
        assertEquals("Hoshikuzu no Tabibito", stale.shows.first().title)
        // A day exactly is still fresh, a moment more is not.
        val made = NOW.minusSeconds(WidgetModel.CONTINUE_FRESH_SECS)
        assertNull((continuing(Fixtures.record(generatedAt = made)) as ContinueView.Shows).updated)
        val yesterday = continuing(Fixtures.record(generatedAt = made.minusSeconds(1))) as ContinueView.Shows
        assertEquals("Updated yesterday", yesterday.updated)
        // A week back says the date.
        assertEquals("Updated 10 Oct", (continuing(Fixtures.record(generatedAt = at(10, 9, 0))) as ContinueView.Shows).updated)
    }

    @Test
    fun signed_out_with_nothing_played_asks_to_connect() {
        assertEquals(ContinueView.SignedOut, continuing(Fixtures.record(signedIn = false, continuing = emptyList())))
        // No record yet reads the same: the step that fills the widget.
        assertEquals(ContinueView.SignedOut, continuing(null))
        // A first-shape record cannot say; with nothing of the list airing, signed out.
        val bare = Fixtures.firstShape().copy(continueWatching = emptyList(), airingToday = emptyList())
        assertEquals(ContinueView.SignedOut, continuing(bare))
    }

    @Test
    fun caught_up_names_the_next_episode_due() {
        val view = continuing(Fixtures.record(continuing = emptyList())) as ContinueView.CaughtUp
        assertEquals(10u, view.next?.episode)
        assertEquals(at(22, 16, 30), view.next?.airsAt)
        // The record's word for it once that has aired: the list's next broadcast it holds instead.
        val later = continuing(Fixtures.record(continuing = emptyList()), now = at(22, 17, 0)) as ContinueView.CaughtUp
        assertNull(later.next)
        // A first-shape record with a show of the list still to air today is caught up, with that show next.
        val first = Fixtures.firstShape().copy(continueWatching = emptyList())
        val fromToday = continuing(first) as ContinueView.CaughtUp
        assertEquals("Natsukaze Relay", fromToday.next?.title)
        assertEquals(at(20, 21, 0), fromToday.next?.airsAt)
    }

    @Test
    fun airing_today_marks_the_list_and_says_what_is_out() {
        val view = airing(Fixtures.record()) as AiringView.Today
        assertEquals("Tuesday", view.weekday)
        assertTrue(view.marks)
        val rows = view.rows.associateBy { it.time }
        assertEquals("E9 out", rows.getValue("17:30").state)
        assertTrue("watched, so not out", rows.getValue("13:00").watched)
        assertFalse(rows.getValue("13:00").out)
        // Aired but not in the list: no dot, and no mark.
        assertFalse(rows.getValue("19:00").out)
        assertFalse(rows.getValue("19:00").onList)
        assertEquals("E9 · Dub", rows.getValue("21:00").state)
        assertEquals("E2", rows.getValue("22:30").state)
        // The week after today is held, not shown today.
        assertTrue(view.rows.none { it.episode == 10u })
    }

    @Test
    fun only_a_dub_is_marked_and_a_sub_says_nothing() {
        val record = Fixtures.record()
        val subbed = record.copy(airingToday = record.airingToday.map { if (it.airsAt == at(20, 22, 0).epochSecond) it.copy(translation = tv.anistream.engine.core.Translation.SUB) else it })
        val rows = (airing(subbed) as AiringView.Today).rows.associateBy { it.time }
        assertEquals("E4", rows.getValue("22:00").state)
        assertNull(rows.getValue("22:00").translation)
        assertEquals("E9 · Dub", rows.getValue("21:00").state)
    }

    @Test
    fun continue_marks_a_dub_where_each_line_names_the_episode() {
        val dub = Fixtures.continuing[1].copy(mark = "Dub")
        fun line(row: tv.anistream.engine.model.WidgetContinue) =
            ((continuing(Fixtures.record(continuing = listOf(row))) as ContinueView.Shows).shows.single())
        assertEquals("Start E2 · Dub", line(dub.copy(resume = false, fraction = null)).line)
        assertEquals("E2 of 10 · Dub", line(dub.copy(minutesLeft = null)).line)
        assertEquals("Resume episode 2, Dub, of Hagane no Niwashi", playLabel(line(dub)))
        assertEquals("Resume episode 7 of Hoshikuzu no Tabibito", playLabel(line(Fixtures.continuing[0])))
    }

    @Test
    fun signed_out_marks_nothing_and_says_nothing_is_out() {
        val view = airing(Fixtures.record(signedIn = false, continuing = emptyList())) as AiringView.Today
        assertFalse(view.marks)
        assertTrue(view.rows.none { it.onList || it.out })
    }

    @Test
    fun a_first_shape_record_never_says_out_without_knowing_it_was_not_watched() {
        val view = airing(Fixtures.firstShape()) as AiringView.Today
        assertTrue(view.marks)
        assertTrue(view.rows.none { it.out || it.watched })
    }

    @Test
    fun the_layouts_take_what_is_still_to_come_and_the_episode_out() {
        val rows = (airing(Fixtures.record()) as AiringView.Today).rows
        // 2 × 2: the next four times still to come.
        assertEquals(listOf("21:00", "22:00", "22:30", "23:45"), WidgetModel.pick(rows, 4, withOut = false).map { it.time })
        // 4 × 2: the one out and unwatched, then those to come.
        assertEquals(listOf("17:30", "21:00", "22:00", "22:30", "23:45"), WidgetModel.pick(rows, 5, withOut = true).map { it.time })
        // Late in the evening, the room left goes to the latest that aired.
        val late = (airing(Fixtures.record(), now = at(20, 23, 0)) as AiringView.Today).rows
        assertEquals(listOf("21:00", "22:00", "22:30", "23:45"), WidgetModel.pick(late, 4, withOut = false).map { it.time })
    }

    @Test
    fun a_week_of_schedule_answers_for_a_day_the_app_has_not_run_on() {
        // Made on Sunday, read on Thursday: Thursday's broadcasts, from the week it holds.
        val sunday = Fixtures.record(generatedAt = at(18, 9, 0), today = emptyList())
        val thursday = airing(sunday, now = at(22, 9, 0)) as AiringView.Today
        assertEquals(listOf("16:30"), thursday.rows.map { it.time })
        assertEquals("Thursday", thursday.weekday)
    }

    @Test
    fun nothing_today_names_what_airs_next_and_past_the_week_there_is_no_schedule() {
        val quiet = Fixtures.record(today = emptyList())
        val view = airing(quiet) as AiringView.NothingToday
        assertEquals(at(22, 16, 30), view.next?.airsAt)
        // Seven days on from the record, it says nothing of today.
        assertEquals(AiringView.NoSchedule, airing(Fixtures.record(generatedAt = at(13, 19, 0), today = emptyList(), nextDays = emptyList())))
        // A first-shape record holds its own day alone: the next day, no schedule.
        assertEquals(AiringView.NoSchedule, airing(Fixtures.firstShape(generatedAt = at(19, 19, 0)).copy(airingToday = emptyList())))
        // And before any record.
        assertEquals(AiringView.NoSchedule, airing(null))
    }

    @Test
    fun the_next_redraw_is_the_next_broadcast_the_record_turning_a_day_old_or_midnight() {
        val record = Record.of(Fixtures.record())
        assertEquals(at(20, 21, 0), Widgets.nextRedraw(record, NOW, ZONE))
        assertEquals(at(21, 0, 0), Widgets.nextRedraw(record, at(20, 23, 50), ZONE))
        // Made yesterday at 20:00, with nothing airing: it turns a day old before midnight.
        val yesterday = Record.of(Fixtures.record(generatedAt = at(19, 20, 0), today = emptyList(), nextDays = emptyList()))
        assertEquals(at(19, 20, 0).plusSeconds(WidgetModel.CONTINUE_FRESH_SECS + 1), Widgets.nextRedraw(yesterday, NOW, ZONE))
        assertEquals(at(21, 0, 0), Widgets.nextRedraw(null, NOW, ZONE))
    }

    @Test
    fun a_colour_is_read_as_opaque_argb() {
        assertEquals(0xFF5D9BE4L, WidgetModel.argb("#5d9be4"))
        assertNull(WidgetModel.argb("blue"))
        assertNull(WidgetModel.argb("#fff"))
    }
}
