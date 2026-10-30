package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Test
import tv.anistream.player.SeekSegment
import tv.anistream.tv.player.ScrubHead
import tv.anistream.tv.player.SeekRun
import tv.anistream.tv.player.clockTime
import tv.anistream.tv.player.remainingTime
import tv.anistream.tv.player.signedSeconds

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PlayerKeysTest {
    private val context = RuntimeEnvironment.getApplication()
    /** The board's episode: the opening 2:10 to 3:40, the ending 21:31 to 23:01, of 24:02. */
    private val segments = listOf(
        SeekSegment(SeekSegment.Kind.Plain, 0.0, 130.0),
        SeekSegment(SeekSegment.Kind.Opening, 130.0, 220.0),
        SeekSegment(SeekSegment.Kind.Plain, 220.0, 1291.0),
        SeekSegment(SeekSegment.Kind.Ending, 1291.0, 1381.0),
        SeekSegment(SeekSegment.Kind.Plain, 1381.0, 1442.0),
    )

    @Test
    fun presses_count_up_and_seek_once() {
        val run = SeekRun()
        repeat(3) { run.press(forward = true) }
        assertEquals("+30 s", run.label(context))
        run.press(forward = false)
        assertEquals("+20 s", run.label(context))
        assertEquals(20.0, run.take(), 0.0)
        assertEquals(0, run.presses)
        run.press(forward = false)
        assertEquals("−10 s", run.label(context))
    }

    @Test
    fun the_scrub_head_stops_once_on_the_opening_edges() {
        val head = ScrubHead(start = 125.0, duration = 1442.0, segments = segments)
        head.move(forward = true, heldMillis = 0)
        assertEquals(130.0, head.at, 0.0)
        head.move(forward = true, heldMillis = 0)
        assertEquals(140.0, head.at, 0.0)
        // Held: 30 s steps after a second, and it still stops on the opening's end.
        head.move(forward = true, heldMillis = 1_200)
        assertEquals(170.0, head.at, 0.0)
        head.move(forward = true, heldMillis = 1_400)
        assertEquals(200.0, head.at, 0.0)
        head.move(forward = true, heldMillis = 1_600)
        assertEquals(220.0, head.at, 0.0)
        head.move(forward = true, heldMillis = 1_800)
        assertEquals(250.0, head.at, 0.0)
    }

    @Test
    fun steps_grow_while_a_key_is_held() {
        assertEquals(10.0, ScrubHead.stepFor(0), 0.0)
        assertEquals(30.0, ScrubHead.stepFor(1_000), 0.0)
        assertEquals(60.0, ScrubHead.stepFor(3_000), 0.0)
    }

    @Test
    fun the_head_stays_inside_the_episode() {
        val head = ScrubHead(start = 5.0, duration = 1442.0, segments = emptyList())
        head.move(forward = false, heldMillis = 0)
        assertEquals(0.0, head.at, 0.0)
    }

    @Test
    fun times_read_as_the_chrome_writes_them() {
        assertEquals("10:29", clockTime(629.0))
        assertEquals("1:02:03", clockTime(3723.0))
        assertEquals("−13:31", remainingTime(631.0, 1442.0))
        assertEquals("0 s", signedSeconds(context, 0.0))
    }
}
