// Downloads as a user-initiated data transfer job: the engine downloads in the app's own process,
// and the job keeps that process going while anything is queued or running, with the system's
// progress notification. Android 15 stops a dataSync foreground service after 6 hours; a job the
// user started has no such cap.

package tv.anistream.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.store.DownloadsModel
import tv.anistream.store.Wording

/** The job's own service: it runs while downloads wait or run, and shows how far they are. */
class DownloadJob : JobService() {
    private val app get() = application as AnistreamApp
    private var scope: CoroutineScope? = null

    override fun onStartJob(params: JobParameters): Boolean {
        // A job the system runs again after the process went needs the engine back first.
        app.start(Flavour.launch(this, Intent()))
        val scope = MainScope().also { this.scope = it }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.downloads_channel), NotificationManager.IMPORTANCE_LOW).apply {
                description = getString(R.string.downloads_channel_description)
            },
        )
        // A user-initiated job says what it does at once, or the system stops it.
        setNotification(params, NOTIFICATION, notification(app.store.downloads.rows.value), JOB_END_NOTIFICATION_POLICY_REMOVE)
        scope.launch {
            app.store.loadDownloads(force = true)
            app.store.downloads.rows.collect { rows ->
                if (!DownloadsModel.busy(rows) && app.store.downloads.loaded.value) {
                    Log.i(AnistreamApp.TAG, "downloads done, the job ends")
                    jobFinished(params, false)
                    this@DownloadJob.scope?.cancel()
                    return@collect
                }
                manager.notify(NOTIFICATION, notification(rows))
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The system took the job back, or the person stopped it from the task manager: the
        // engine keeps the queue, and the next time the app is open the job is asked for again.
        scope?.cancel()
        scope = null
        return false
    }

    /** The notification: how many are left, and how far the running one is. */
    private fun notification(rows: List<DownloadRow>): Notification {
        val running = rows.filter { it.state == DownloadState.ACTIVE }
        val waiting = rows.count { it.state == DownloadState.QUEUED }
        val total = running.sumOf { it.total.toLong() }
        val done = running.sumOf { it.downloaded.toLong() }
        val title = running.firstOrNull()?.let { "${it.title} · episode ${it.episode}" } ?: "Downloads"
        val text = listOfNotNull(
            if (total > 0) "${Wording.bytes(done)} of ${Wording.bytes(total)}" else null,
            if (waiting > 0) "${Wording.plural(waiting, "episode")} waiting" else null,
        ).joinToString(" · ").ifEmpty { "Starting" }
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).putExtra(MainActivity.PAGE, "downloads"),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(title)
            .setContentText(text)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setContentIntent(open)
            .setProgress(if (total > 0) 1000 else 0, if (total > 0) (done * 1000 / total).toInt() else 0, total == 0L)
            .build()
    }

    companion object {
        private const val JOB = 4101
        private const val NOTIFICATION = 4101
        private const val CHANNEL = "downloads"

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
