package tv.anistream.app

import android.app.Application
import android.app.Notification
import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.store.LiveDownloads
import tv.anistream.ui.AppLink

/** The download job's notifications: a Live Update on Android 16, plain progress on 15, and the finished one. */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class DownloadNotificationsTest {
    private val context: Context = RuntimeEnvironment.getApplication()

    private val one = LiveDownloads(
        title = "Stardust Wayfarer", text = "Episode 8 · 5 min left", segments = listOf(LiveDownloads.BAR), progress = 374,
        indeterminate = false, chip = "5 min", several = false, paused = false,
    )
    private val three = one.copy(title = "Downloading 3 episodes", text = "Stardust Wayfarer E8 · 2.7 GB left", segments = listOf(333, 333, 334), progress = 100, several = true)

    private fun actions(notification: Notification) = notification.actions.map { it.title.toString() to shadowOf(it.actionIntent).savedIntent.action }

    @Test
    @Config(sdk = [36])
    fun on_android_16_it_is_a_live_update_with_a_segment_an_episode_and_the_chip() {
        val notification = DownloadNotifier(context, sdk = 36).ongoing(three)
        assertEquals("android.app.Notification\$ProgressStyle", notification.extras.getString(Notification.EXTRA_TEMPLATE))
        assertEquals("5 min", notification.shortCriticalText)
        assertTrue(notification.flags and Notification.FLAG_ONGOING_EVENT != 0)
        assertEquals("Downloading 3 episodes", notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals(listOf("Pause all" to DownloadNotifier.ACTION_PAUSE, "Cancel" to DownloadNotifier.ACTION_CANCEL), actions(notification))
        val style = Notification.Builder.recoverBuilder(context, notification).style as Notification.ProgressStyle
        assertEquals(listOf(333, 333, 334), style.progressSegments.map { it.length })
        assertEquals(listOf(333, 666), style.progressPoints.map { it.position })
        assertEquals(100, style.progress)
    }

    @Test
    @Config(sdk = [35])
    fun on_android_15_it_is_the_plain_progress_notification() {
        val notification = DownloadNotifier(context, sdk = 35).ongoing(one)
        assertFalse(notification.extras.getString(Notification.EXTRA_TEMPLATE).orEmpty().contains("ProgressStyle"))
        assertEquals(LiveDownloads.BAR, notification.extras.getInt(Notification.EXTRA_PROGRESS_MAX))
        assertEquals(374, notification.extras.getInt(Notification.EXTRA_PROGRESS))
        assertEquals(listOf("Pause" to DownloadNotifier.ACTION_PAUSE, "Cancel" to DownloadNotifier.ACTION_CANCEL), actions(notification))
    }

    @Test
    @Config(sdk = [36])
    fun paused_it_offers_resume() {
        val notifier = DownloadNotifier(context, sdk = 36)
        assertEquals("Resume", notifier.ongoing(one.copy(paused = true, chip = null)).actions[0].title.toString())
        assertEquals("Resume all", notifier.ongoing(three.copy(paused = true, chip = null)).actions[0].title.toString())
    }

    @Test
    @Config(sdk = [36])
    fun a_finished_episode_says_so_and_plays_its_file_through_the_app() {
        val row = DownloadRow(
            id = 42, anilistId = 900001u, title = "Stardust Wayfarer", episode = "8", state = DownloadState.DONE, fraction = 1.0,
            downloaded = 1_100_000_000uL, total = 1_100_000_000uL, error = null, path = "/files/42.mkv",
        )
        val notification = DownloadNotifier(context, sdk = 36).finished(row)
        assertEquals("Episode 8 downloaded", notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals("Stardust Wayfarer · ready to watch offline", notification.extras.getCharSequence(Notification.EXTRA_TEXT).toString())
        assertTrue(notification.flags and Notification.FLAG_ONGOING_EVENT == 0)
        val play = notification.actions.single()
        assertEquals("Play", play.title.toString())
        assertEquals(AppLink.Download(42), shadowOf(play.actionIntent).savedIntent.appLink())
        assertEquals(AppLink.Downloads, shadowOf(notification.contentIntent).savedIntent.appLink())
    }
}
