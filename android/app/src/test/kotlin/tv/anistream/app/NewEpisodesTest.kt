package tv.anistream.app

import android.Manifest
import android.app.Application
import android.app.Notification
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.os.Looper
import androidx.work.Configuration
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.testing.SynchronousExecutor
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.testing.WorkManagerTestInitHelper
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.EpisodeCheck
import tv.anistream.store.NewEpisode
import tv.anistream.ui.AppLink

/** The application the work and the action reach, with a check the test sets and the marks it was asked for. */
class FakeHost : Application(), NewEpisodesHost {
    override var episodeCheck: EpisodeCheck? = null

    override val episodeNotifier: EpisodeNotifier by lazy { EpisodeNotifier(this) }

    val marked = mutableListOf<Pair<AnilistId, UInt>>()

    override suspend fun markWatched(id: AnilistId, episode: UInt) {
        marked += id to episode
    }
}

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = FakeHost::class)
class NewEpisodesTest {
    private val host = RuntimeEnvironment.getApplication() as FakeHost
    private val context: Context = host
    private val manager = context.getSystemService(NotificationManager::class.java)

    private val wayfarer = NewEpisode(900001u, "Stardust Wayfarer", 8u, airedAt = 1_792_600_000)
    private val relay = NewEpisode(900006u, "Summer Wind Relay", 3u, airedAt = 1_792_603_600)
    private val library = NewEpisode(900002u, "The Library Above the Clouds", 11u, airedAt = 1_792_607_200)

    @Before
    fun setUp() {
        shadowOf(host).grantPermissions(Manifest.permission.POST_NOTIFICATIONS)
    }

    // The schedule

    @Test
    fun the_check_runs_about_every_six_hours_on_a_network_with_the_battery_not_low() {
        val spec = NewEpisodesSchedule.request().workSpec
        assertEquals(TimeUnit.HOURS.toMillis(6), spec.intervalDuration)
        assertEquals(TimeUnit.HOURS.toMillis(1), spec.flexDuration)
        assertEquals(NetworkType.CONNECTED, spec.constraints.requiredNetworkType)
        assertTrue(spec.constraints.requiresBatteryNotLow())
        assertFalse(spec.constraints.requiresCharging())
        assertEquals(NewEpisodesWorker::class.java.name, spec.workerClassName)
    }

    @Test
    fun turning_the_switch_on_schedules_one_check_and_off_cancels_it() {
        WorkManagerTestInitHelper.initializeTestWorkManager(context, Configuration.Builder().setExecutor(SynchronousExecutor()).build())
        val work = WorkManager.getInstance(context)
        NewEpisodesSchedule.set(context, on = true)
        // Asked again at the next launch, it stays the one.
        NewEpisodesSchedule.set(context, on = true)
        val scheduled = work.getWorkInfosForUniqueWork(NewEpisodesSchedule.NAME).get()
        assertEquals(1, scheduled.size)
        assertEquals(WorkInfo.State.ENQUEUED, scheduled.single().state)

        NewEpisodesSchedule.set(context, on = false)
        assertEquals(WorkInfo.State.CANCELLED, work.getWorkInfosForUniqueWork(NewEpisodesSchedule.NAME).get().single().state)
    }

    @Test
    fun the_permission_is_asked_for_only_as_the_switch_turns_on() {
        for (canPost in listOf(true, false)) for (permitted in listOf(true, false)) {
            assertEquals(EpisodeAlerts.Turn.Off, EpisodeAlerts.Turn.of(on = false, canPost, permitted))
        }
        assertEquals(EpisodeAlerts.Turn.On, EpisodeAlerts.Turn.of(on = true, canPost = true, permitted = true))
        assertEquals(EpisodeAlerts.Turn.Ask, EpisodeAlerts.Turn.of(on = true, canPost = false, permitted = false))
        assertEquals(EpisodeAlerts.Turn.SystemSettings, EpisodeAlerts.Turn.of(on = true, canPost = false, permitted = true))
    }

    @Test
    fun at_launch_the_switch_goes_off_where_notifications_can_no_longer_show_and_asks_nothing() {
        WorkManagerTestInitHelper.initializeTestWorkManager(context, Configuration.Builder().setExecutor(SynchronousExecutor()).build())
        val alerts = EpisodeAlerts(context, host.episodeNotifier, available = true)
        alerts.set(true)
        assertTrue(alerts.on.value)
        shadowOf(host).denyPermissions(Manifest.permission.POST_NOTIFICATIONS)
        alerts.reconcile()
        assertFalse(alerts.on.value)
        val work = WorkManager.getInstance(context).getWorkInfosForUniqueWork(NewEpisodesSchedule.NAME).get()
        assertEquals(WorkInfo.State.CANCELLED, work.single().state)
        // Where the engine has no check, it never goes on.
        val none = EpisodeAlerts(context, host.episodeNotifier, available = false)
        none.set(true)
        assertFalse(none.on.value)
    }

    // The work

    private fun run(attempt: Int = 0): ListenableWorker.Result = runBlocking {
        TestListenableWorkerBuilder<NewEpisodesWorker>(context).setRunAttemptCount(attempt).build().doWork()
    }

    @Test
    fun a_check_posts_one_notification_for_each_episode_under_a_summary() {
        host.episodeCheck = EpisodeCheck { listOf(wayfarer, relay, library) }
        assertEquals(ListenableWorker.Result.success(), run())
        assertEquals(setOf(tag(wayfarer), tag(relay), tag(library)), episodeTags())
        val summary = summary()!!
        assertEquals("3 new episodes", summary.extras.getString(Notification.EXTRA_TITLE))
        assertEquals(
            listOf("The Library Above the Clouds · Episode 11", "Summer Wind Relay · Episode 3", "Stardust Wayfarer · Episode 8"),
            summary.extras.getCharSequenceArray(Notification.EXTRA_TEXT_LINES)!!.map(CharSequence::toString),
        )
    }

    @Test
    fun one_episode_stands_alone_and_says_which() {
        host.episodeCheck = EpisodeCheck { listOf(wayfarer) }
        run()
        val notification = posted(tag(wayfarer))
        assertEquals("Stardust Wayfarer", notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals("Episode 8 is out", notification.extras.getCharSequence(Notification.EXTRA_TEXT).toString())
        assertEquals(wayfarer.airedAt * 1000, notification.`when`)
        assertEquals(EpisodeNotifier.CHANNEL, notification.channelId)
        assertNull(summary())
    }

    @Test
    fun an_episode_said_twice_replaces_itself() {
        host.episodeCheck = EpisodeCheck { listOf(wayfarer) }
        run()
        run()
        assertEquals(1, manager.activeNotifications.size)
    }

    @Test
    fun nothing_is_asked_while_a_notification_could_not_show() {
        shadowOf(host).denyPermissions(Manifest.permission.POST_NOTIFICATIONS)
        var asked = false
        host.episodeCheck = EpisodeCheck { asked = true; listOf(wayfarer) }
        assertEquals(ListenableWorker.Result.success(), run())
        assertFalse(asked)
        assertTrue(manager.activeNotifications.isEmpty())
    }

    @Test
    fun without_a_check_in_the_engine_the_work_does_nothing() {
        host.episodeCheck = null
        assertEquals(ListenableWorker.Result.success(), run())
        assertTrue(manager.activeNotifications.isEmpty())
    }

    @Test
    fun a_failed_check_is_tried_again_three_times() {
        host.episodeCheck = EpisodeCheck { error("AniList did not answer") }
        assertEquals(ListenableWorker.Result.retry(), run(attempt = 0))
        assertEquals(ListenableWorker.Result.retry(), run(attempt = 2))
        assertEquals(ListenableWorker.Result.success(), run(attempt = 3))
    }

    // The grouping

    @Test
    fun of_a_batch_only_the_first_new_one_rings() {
        val plan = EpisodePlan.of(listOf(wayfarer, relay, wayfarer), shown = emptyMap())
        assertEquals(listOf(true, false), plan.posts.map { it.alert })
        assertEquals(2, plan.summary?.count)
    }

    @Test
    fun an_episode_already_shown_neither_rings_nor_counts_twice() {
        val shown = mapOf(tag(wayfarer) to EpisodePlan.lineOf(wayfarer))
        val again = EpisodePlan.of(listOf(wayfarer), shown)
        assertEquals(listOf(false), again.posts.map { it.alert })
        assertNull(again.summary)

        val beside = EpisodePlan.of(listOf(relay), shown)
        assertEquals(listOf(true), beside.posts.map { it.alert })
        assertEquals(listOf("Summer Wind Relay · Episode 3", "Stardust Wayfarer · Episode 8"), beside.summary?.lines)
    }

    // The actions

    @Test
    fun a_tap_opens_the_title_and_play_plays_the_episode_through_the_app() {
        host.episodeCheck = EpisodeCheck { listOf(wayfarer, relay) }
        run()
        val notification = posted(tag(wayfarer))

        val open = shadowOf(notification.contentIntent)
        assertTrue(open.isActivityIntent)
        assertEquals(AppLink.Title(900001u), AppLink.parse(open.savedIntent.dataString))
        assertEquals(MainActivity::class.java.name, open.savedIntent.component?.className)
        assertTrue(open.savedIntent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
        assertEquals(tag(wayfarer), open.savedIntent.getStringExtra(EpisodeNotifier.EXTRA_NOTIFICATION))

        val (play, mark) = notification.actions.toList()
        assertEquals("Play", play.title.toString())
        val playing = shadowOf(play.actionIntent)
        assertTrue(playing.isActivityIntent)
        assertEquals(AppLink.Play(900001u, "8"), AppLink.parse(playing.savedIntent.dataString))

        assertEquals("Mark watched", mark.title.toString())
        val marking = shadowOf(mark.actionIntent)
        assertTrue(marking.isBroadcastIntent)
        assertEquals(EpisodeNotifier.ACTION_MARK_WATCHED, marking.savedIntent.action)
        assertEquals(EpisodeActionReceiver::class.java.name, marking.savedIntent.component?.className)
        assertEquals(900001u to 8u, EpisodeNotifier.watchedOf(marking.savedIntent.dataString))

        // Each episode's actions are intents of their own, by their data.
        val other = shadowOf(posted(tag(relay)).actions[0].actionIntent).savedIntent
        assertFalse(other.filterEquals(playing.savedIntent))
    }

    @Test
    fun mark_watched_takes_the_notification_away_and_marks_the_episode() {
        host.episodeCheck = EpisodeCheck { listOf(wayfarer, relay) }
        run()
        val mark = shadowOf(posted(tag(wayfarer)).actions[1].actionIntent).savedIntent

        EpisodeActionReceiver().onReceive(context, mark)
        shadowOf(Looper.getMainLooper()).idle()

        assertEquals(listOf(900001u to 8u), host.marked)
        assertEquals(setOf(tag(relay)), episodeTags())
        // The summary stays while the group has an episode in it: the system would take the
        // episode away with it.
        assertEquals("1 new episode", summary()?.extras?.getString(Notification.EXTRA_TITLE))

        EpisodeActionReceiver().onReceive(context, shadowOf(posted(tag(relay)).actions[1].actionIntent).savedIntent)
        shadowOf(Looper.getMainLooper()).idle()
        assertTrue(manager.activeNotifications.isEmpty())
    }

    @Test
    fun a_mark_watched_uri_names_its_episode_and_nothing_else_does() {
        assertEquals(900001u to 8u, EpisodeNotifier.watchedOf(EpisodeNotifier.watchedUri(900001u, 8u).toString()))
        assertNull(EpisodeNotifier.watchedOf("anistream://open/title/900001"))
        assertNull(EpisodeNotifier.watchedOf("anistream://watched/900001"))
        assertNull(EpisodeNotifier.watchedOf("anistream://watched/900001/8/9"))
        assertNull(EpisodeNotifier.watchedOf(null))
    }

    private fun tag(episode: NewEpisode) = EpisodePlan.tagOf(episode)

    private fun episodeTags(): Set<String> =
        manager.activeNotifications.filter { it.id == EpisodeNotifier.EPISODE_ID }.mapNotNull { it.tag }.toSet()

    private fun posted(tag: String): Notification =
        manager.activeNotifications.single { it.id == EpisodeNotifier.EPISODE_ID && it.tag == tag }.notification

    private fun summary(): Notification? =
        manager.activeNotifications.singleOrNull { it.id == EpisodeNotifier.SUMMARY_ID }?.notification
}
