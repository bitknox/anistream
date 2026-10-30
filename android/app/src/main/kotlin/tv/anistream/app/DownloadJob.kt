// Downloads as a user-initiated data transfer job: the engine downloads in the app's own process,
// and the job keeps that process going while anything is queued or running, with its notification,
// a Live Update on Android 16 (DownloadNotifications.kt). Android 15 stops a dataSync foreground
// service after 6 hours; a job the user started has no such cap.

package tv.anistream.app

import android.app.NotificationManager
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import tv.anistream.store.AppStore
import kotlinx.coroutines.launch
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.store.DownloadsModel
import tv.anistream.store.DownloadSession
import tv.anistream.store.LiveDownloads

/**
 * The job's own service: it runs while downloads wait or run, and shows how far they are. A queue
 * paused from the notification keeps it for [PAUSE_GRACE_MILLIS], so its Resume stays to hand;
 * each episode that finishes while it runs gets its own notification, with Play.
 */
class DownloadJob : JobService() {
    private val app get() = application as AnistreamApp
    private var scope: CoroutineScope? = null

    override fun onStartJob(params: JobParameters): Boolean {
        // A job the system runs again after the process went needs the engine back first.
        app.start(Flavour.launch(this, Intent()))
        val scope = MainScope().also { this.scope = it }
        val manager = getSystemService(NotificationManager::class.java)
        val notifier = DownloadNotifier(this)
        // A user-initiated job says what it does at once, or the system stops it.
        setNotification(params, NOTIFICATION, notifier.starting(), JOB_END_NOTIFICATION_POLICY_REMOVE)
        val session = DownloadSession()
        var grace: Job? = null
        val end = {
            Log.i(AnistreamApp.TAG, "downloads done, the job ends")
            jobFinished(params, false)
            this.scope?.cancel()
        }
        // An engine that failed or stopped has nothing to download with: the job ends rather than
        // holding its notification for ever.
        scope.launch {
            app.store.phase.first { it is AppStore.Phase.Failed || it is AppStore.Phase.Stopped }
            Log.w(AnistreamApp.TAG, "the engine is not running, the download job ends")
            end()
        }
        scope.launch {
            app.store.loadDownloads(force = true)
            combine(app.store.downloads.rows, app.store.downloads.rates, ::Pair).collect { (rows, rates) ->
                if (!app.store.downloads.loaded.value) return@collect
                session.saw(rows).forEach(notifier::postFinished)
                val live = LiveDownloads.of(this@DownloadJob, rows, rates, session.finished)
                if (live == null) {
                    end()
                    return@collect
                }
                // Paused, nothing runs; the notification stays a while for Resume, then goes.
                if (live.paused) {
                    if (grace == null) grace = launch { delay(PAUSE_GRACE_MILLIS); end() }
                } else {
                    grace?.cancel()
                    grace = null
                }
                manager.notify(NOTIFICATION, notifier.ongoing(live))
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The system took the job back, or the person stopped it from the task manager: the
        // engine keeps the queue, and the next time the app is open the job is asked for again.
        scope?.cancel()
        scope = null
        // Run it again when the system allows: the queue is still there to work through.
        return true
    }

    companion object {
        private const val JOB = 4101
        private const val NOTIFICATION = 4101

        /** How long a queue paused from the notification keeps it, for Resume. */
        const val PAUSE_GRACE_MILLIS = 10 * 60_000L

        /**
         * Ask for the job while anything waits or runs, from a screen the person is looking at,
         * which is when a user-initiated job may be asked for. Once a job runs, asking again does
         * nothing.
         */
        fun ensure(context: Context, rows: List<DownloadRow>, wifiOnly: Boolean) {
            if (!DownloadsModel.busy(rows)) return
            val scheduler = context.getSystemService(JobScheduler::class.java)
            if (scheduler.getPendingJob(JOB) != null) return
            val left = rows.filter { it.state != DownloadState.DONE }.sumOf { (it.total - it.downloaded).toLong().coerceAtLeast(0) }
            val job = JobInfo.Builder(JOB, ComponentName(context, DownloadJob::class.java))
                .setUserInitiated(true)
                .setRequiredNetworkType(if (wifiOnly) JobInfo.NETWORK_TYPE_UNMETERED else JobInfo.NETWORK_TYPE_ANY)
                .setEstimatedNetworkBytes(left.coerceAtLeast(1), 0)
                .build()
            val result = runCatching { scheduler.schedule(job) }
            if (result.getOrNull() != JobScheduler.RESULT_SUCCESS) {
                Log.w(AnistreamApp.TAG, "the download job was not scheduled: ${result.exceptionOrNull()?.message ?: "refused"}")
            }
        }
    }
}
