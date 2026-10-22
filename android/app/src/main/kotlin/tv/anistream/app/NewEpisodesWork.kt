// The periodic check for new episodes, which the system runs about every six hours on a network
// with the battery not low, and the Mark watched action its notifications carry.

package tv.anistream.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequest
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.BackgroundEngine
import tv.anistream.store.EpisodeCheck

/** What the work and the action need of the application. */
interface NewEpisodesHost {
    /** The check, or `null` while the engine has none. */
    val episodeCheck: EpisodeCheck?

    val episodeNotifier: EpisodeNotifier

    /** Mark [episode] of [id] watched, on the app's engine or one started for it. */
    suspend fun markWatched(id: AnilistId, episode: UInt)

    /**
     * Make the widgets' record, while a widget is placed: what a run does when no notification
     * could show, which the check would otherwise make it after.
     */
    suspend fun recordWidgets() {}
}

/** When the check runs: about every six hours, on a network, with the battery not low. */
object NewEpisodesSchedule {
    /** The unique work's name. */
    const val NAME = "new-episodes"

    const val INTERVAL_HOURS = 6L

    /** The last hour of each interval, which the system picks a moment in. */
    const val FLEX_HOURS = 1L

    val constraints: Constraints
        get() = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .setRequiresBatteryNotLow(true)
            .build()

    fun request(): PeriodicWorkRequest =
        PeriodicWorkRequestBuilder<NewEpisodesWorker>(INTERVAL_HOURS, TimeUnit.HOURS, FLEX_HOURS, TimeUnit.HOURS)
            .setConstraints(constraints)
            .build()

    /**
     * Check from now on, or stop. Asked again on every launch, which keeps the schedule's place
     * and takes this version's constraints.
     */
    fun set(context: Context, on: Boolean) {
        val work = WorkManager.getInstance(context)
        if (on) {
            work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request())
        } else {
            work.cancelUniqueWork(NAME)
        }
    }
}

/**
 * One check: ask the engine for the episodes that aired since the last one, and post them, then
 * keep the widgets' record. Nothing is asked of the check while a notification could not show, so
 * no episode is spent on a check that cannot say it: only the widgets' record is made then. With
 * nothing notifying, the engine's check says nothing, and a run only keeps the widgets' record. A
 * check that fails is tried again later, three times at most.
 */
class NewEpisodesWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val host = applicationContext as? NewEpisodesHost ?: return Result.failure()
        val check = host.episodeCheck ?: return Result.success()
        if (!host.episodeNotifier.canPost) {
            runCatching { host.recordWidgets() }.onFailure { if (it is CancellationException) throw it }
            return Result.success()
        }
        val episodes = try {
            check.newEpisodes()
        } catch (e: BackgroundEngine.Yielded) {
            // The app started as it ran: the next run asks the app's engine.
            return Result.retry()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(AnistreamApp.TAG, "the check for new episodes failed: ${e.message}")
            return if (runAttemptCount < RETRIES) Result.retry() else Result.success()
        }
        host.episodeNotifier.post(episodes)
        return Result.success()
    }

    companion object {
        const val RETRIES = 3
    }
}

/**
 * A notification's Mark watched: the notification goes at once, and the episode is marked on the
 * app's engine, or on one started for it while the app is not running.
 */
class EpisodeActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != EpisodeNotifier.ACTION_MARK_WATCHED) return
        val (id, episode) = EpisodeNotifier.watchedOf(intent) ?: return
        val host = context.applicationContext as? NewEpisodesHost ?: return
        host.episodeNotifier.dismiss(EpisodePlan.tagOf(id, episode))
        val pending = goAsync()
        MainScope().launch {
            try {
                // Within the time a receiver has, an engine started for it included.
                withTimeout(MARK_MILLIS) { host.markWatched(id, episode) }
            } catch (e: Exception) {
                Log.w(AnistreamApp.TAG, "episode $episode of $id was not marked watched: ${e.message}")
            } finally {
                pending.finish()
                cancel()
            }
        }
    }

    private companion object {
        const val MARK_MILLIS = 9_000L
    }
}
