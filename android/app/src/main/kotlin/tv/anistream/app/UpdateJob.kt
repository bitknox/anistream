// The app's update downloads as a user-initiated data transfer job, as episodes do: asked for as
// the viewer presses Update, with its notification, and stopped by Cancel.

package tv.anistream.app

import android.app.NotificationManager
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import tv.anistream.store.HttpApkFetch
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState

/**
 * The job's own service: it runs the update's download, its checks and the hand-over to the
 * installer (`UpdateModel.run`), showing how far the download is, and ends once the installer has
 * the file or the update stopped.
 */
class UpdateJob : JobService() {
    private val app get() = application as AnistreamApp
    private var scope: CoroutineScope? = null

    override fun onStartJob(params: JobParameters): Boolean {
        val updates = app.store.updates
        val notifier = UpdateNotifier(this)
        // A user-initiated job says what it does at once, or the system stops it.
        setNotification(params, NOTIFICATION, notifier.progress(updates.state.value), JOB_END_NOTIFICATION_POLICY_REMOVE)
        val manager = getSystemService(NotificationManager::class.java)
        val scope = MainScope().also { this.scope = it }
        scope.launch {
            val shown = launch {
                updates.state.collect { state -> if (state is UpdateState.Downloading) manager.notify(NOTIFICATION, notifier.progress(state)) }
            }
            // A job started again after the process went finds nothing pressed, and ends.
            updates.run(HttpApkFetch(plainHttp = BuildConfig.UPDATE_TEST))
            shown.cancel()
            Log.i(AnistreamApp.TAG, "update: the job ends at ${updates.state.value::class.simpleName}")
            jobFinished(params, false)
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The app's own Cancel has said so already; Android stopping it, as from the task manager,
        // fails the update. Either way the run deletes what it fetched.
        if (params.stopReason != JobParameters.STOP_REASON_CANCELLED_BY_APP) app.store.updates.stopped()
        scope?.cancel()
        scope = null
        return false
    }

    companion object {
        private const val JOB = 4201
        const val NOTIFICATION = 4201

        /** Ask for the job, as the viewer presses Update: whether Android took it. */
        fun schedule(context: Context, offer: UpdateOffer): Boolean {
            val scheduler = context.getSystemService(JobScheduler::class.java)
            val job = JobInfo.Builder(JOB, ComponentName(context, UpdateJob::class.java))
                .setUserInitiated(true)
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setEstimatedNetworkBytes(offer.bytes.coerceAtLeast(1), 0)
                .build()
            val result = runCatching { scheduler.schedule(job) }
            if (result.getOrNull() != JobScheduler.RESULT_SUCCESS) {
                Log.w(AnistreamApp.TAG, "the update job was not scheduled: ${result.exceptionOrNull()?.message ?: "refused"}")
                return false
            }
            return true
        }

        fun cancel(context: Context) {
            context.getSystemService(JobScheduler::class.java).cancel(JOB)
        }
    }
}
