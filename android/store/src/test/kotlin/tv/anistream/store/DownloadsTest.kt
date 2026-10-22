package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState

/** The queue as Downloads shows it: by title, each running one's speed, and what keeps the job on. */
class DownloadsTest {
    private fun row(id: Long, title: UInt, episode: String, state: DownloadState, downloaded: ULong, total: ULong = 1_000_000uL) = DownloadRow(
        id = id, anilistId = title, title = "Title $title", episode = episode, state = state,
        fraction = if (total > 0uL) downloaded.toDouble() / total.toDouble() else 0.0,
        downloaded = downloaded, total = total, error = null, path = if (state == DownloadState.DONE) "/files/$id.mkv" else null,
    )

    @Test
    fun a_running_download_s_speed_comes_from_how_far_it_moved_and_its_time_left_from_that() {
        var now = 0L
        val model = DownloadsModel(clock = { now })
        model.apply(listOf(row(1, 900001u, "8", DownloadState.ACTIVE, downloaded = 0uL)))
        assertNull(model.rates.value[1])
        now = 2_000L
        model.apply(listOf(row(1, 900001u, "8", DownloadState.ACTIVE, downloaded = 200_000uL)))
        assertEquals(100_000.0, model.rates.value.getValue(1), 0.1)
        // Half the last reading and half this one.
        now = 4_000L
        model.apply(listOf(row(1, 900001u, "8", DownloadState.ACTIVE, downloaded = 600_000uL)))
        assertEquals(150_000.0, model.rates.value.getValue(1), 0.1)
        assertEquals(3L, model.secondsLeft(model.rows.value.single(), model.rates.value))
    }

    @Test
    fun the_queue_groups_by_title_in_its_order_and_counts_what_is_on_disk() {
        val rows = listOf(
            row(1, 900001u, "8", DownloadState.DONE, downloaded = 1_400_000uL, total = 1_400_000uL),
            row(2, 900002u, "3", DownloadState.QUEUED, downloaded = 0uL),
            row(3, 900001u, "9", DownloadState.PAUSED, downloaded = 300_000uL),
        )
        val groups = DownloadsModel.groups(rows)
        assertEquals(listOf(900001u, 900002u), groups.map { it.id })
        assertEquals(1_700_000L, groups.first().bytesOnDisk)
        assertTrue(DownloadsModel.busy(rows))
        assertFalse(DownloadsModel.busy(rows.filter { it.state != DownloadState.QUEUED }))
    }
}
