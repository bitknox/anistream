package tv.anistream.store

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState

/** The download notification's words and bar, as NativeLiveActivity draws them. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class LiveDownloadsTest {
    private val context = RuntimeEnvironment.getApplication()
    private val mb = 1_000_000uL

    private fun row(id: Long, title: String, episode: String, state: DownloadState, downloaded: ULong, total: ULong) = DownloadRow(
        id = id, anilistId = 900000u + id.toUInt(), title = title, episode = episode, state = state,
        fraction = if (total > 0uL) downloaded.toDouble() / total.toDouble() else 0.0,
        downloaded = downloaded, total = total, error = null, path = if (state == DownloadState.DONE) "/files/$id.mkv" else null,
    )

    @Test
    fun one_episode_says_its_show_its_number_and_the_time_left() {
        val rows = listOf(row(1, "Stardust Wayfarer", "8", DownloadState.ACTIVE, 412u * mb, 1_100u * mb))
        // 688 MB left at 2.4 MB/s: 287 s, so 5 min.
        val live = LiveDownloads.of(context, rows, mapOf(1L to 2_400_000.0))!!
        assertEquals("Stardust Wayfarer", live.title)
        assertEquals("Episode 8 · 5 min left", live.text)
        assertEquals("5 min", live.chip)
        assertFalse(live.several)
        assertFalse(live.paused)
        assertEquals(listOf(LiveDownloads.BAR), live.segments)
        assertEquals(374, live.progress)
    }

    @Test
    fun without_a_speed_yet_it_says_the_bytes_and_a_waiting_one_says_so() {
        val running = listOf(row(1, "Stardust Wayfarer", "8", DownloadState.ACTIVE, 412u * mb, 1_100u * mb))
        assertEquals("Episode 8 · 412 MB of 1.1 GB", LiveDownloads.of(context, running, emptyMap())!!.text)
        assertNull(LiveDownloads.of(context, running, emptyMap())!!.chip)
        val waiting = listOf(row(1, "Stardust Wayfarer", "8", DownloadState.QUEUED, 0u, 0u))
        val live = LiveDownloads.of(context, waiting, emptyMap())!!
        assertEquals("Episode 8 · Waiting", live.text)
        assertTrue(live.indeterminate)
    }

    @Test
    fun several_say_how_many_the_one_running_and_the_bytes_left() {
        val rows = listOf(
            row(1, "Stardust Wayfarer", "8", DownloadState.ACTIVE, 300u * mb, 1_000u * mb),
            row(2, "Stardust Wayfarer", "9", DownloadState.QUEUED, 0u, 1_000u * mb),
            row(3, "Summer Wind Relay", "3", DownloadState.QUEUED, 0u, 1_000u * mb),
        )
        val live = LiveDownloads.of(context, rows, mapOf(1L to 2_400_000.0))!!
        assertEquals("Downloading 3 episodes", live.title)
        assertEquals("Stardust Wayfarer E8 · 2.7 GB left", live.text)
        assertTrue(live.several)
        // A segment an episode, filling the bar, and the tracker at the bytes done.
        assertEquals(listOf(333, 333, 334), live.segments)
        assertEquals(100, live.progress)
        // 2.7 GB at 2.4 MB/s: 18 min 45 s, said as 19.
        assertEquals("19 min", live.chip)
    }

    @Test
    fun an_episode_finished_while_the_job_ran_keeps_its_full_segment() {
        val rows = listOf(
            row(1, "Stardust Wayfarer", "8", DownloadState.DONE, 1_000u * mb, 1_000u * mb),
            row(2, "Stardust Wayfarer", "9", DownloadState.ACTIVE, 500u * mb, 1_000u * mb),
            // Down before the job began: not on the bar.
            row(3, "Summer Wind Relay", "1", DownloadState.DONE, 1_000u * mb, 1_000u * mb),
        )
        val live = LiveDownloads.of(context, rows, emptyMap(), finished = setOf(1L))!!
        assertEquals(listOf(500, 500), live.segments)
        assertEquals(750, live.progress)
        assertEquals("Stardust Wayfarer", live.title)
        assertFalse("one left", live.several)
    }

    @Test
    fun paused_it_says_so_and_has_no_time_left() {
        val one = listOf(row(1, "Stardust Wayfarer", "8", DownloadState.PAUSED, 412u * mb, 1_100u * mb))
        val live = LiveDownloads.of(context, one, mapOf(1L to 2_400_000.0))!!
        assertTrue(live.paused)
        assertEquals("Episode 8 · Paused · 412 MB of 1.1 GB", live.text)
        assertNull(live.chip)
        val two = one + row(2, "Summer Wind Relay", "3", DownloadState.PAUSED, 0u, 1_000u * mb)
        assertEquals("2 episodes paused", LiveDownloads.of(context, two, emptyMap())!!.title)
    }

    @Test
    fun nothing_left_is_no_notification() {
        assertNull(LiveDownloads.of(context, emptyList(), emptyMap()))
        assertNull(LiveDownloads.of(context, listOf(row(1, "Kumo", "1", DownloadState.DONE, mb, mb), row(2, "Kumo", "2", DownloadState.FAILED, 0u, mb)), emptyMap()))
    }

    @Test
    fun the_chip_says_minutes_then_hours() {
        assertEquals("1 min", LiveDownloads.chipOf(context, 20))
        assertEquals("59 min", LiveDownloads.chipOf(context, 59 * 60))
        assertEquals("1h 20m", LiveDownloads.chipOf(context, 80 * 60))
        assertEquals("12h", LiveDownloads.chipOf(context, 12 * 3600 + 120))
    }

    @Test
    fun the_session_says_each_episode_finished_while_it_ran_once() {
        val session = DownloadSession()
        val before = row(9, "Kumo", "1", DownloadState.DONE, mb, mb)
        val running = row(1, "Kumo", "2", DownloadState.ACTIVE, 0u, mb)
        assertEquals(emptyList<DownloadRow>(), session.saw(listOf(before, running)))
        val done = running.copy(state = DownloadState.DONE, downloaded = mb)
        assertEquals(listOf(done), session.saw(listOf(before, done)))
        assertEquals(emptyList<DownloadRow>(), session.saw(listOf(before, done)))
        assertEquals(setOf(1L), session.finished)
    }
}
