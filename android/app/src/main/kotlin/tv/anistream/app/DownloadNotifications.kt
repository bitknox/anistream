// The download job's notification, as NativeLiveActivity draws it: on Android 16 a Live Update, the
// notification promoted to the top of the shade and the lock screen with a bar of a segment per
// episode and the time left in the status bar's chip; on Android 15 the plain progress
// notification. Each finished episode gets an ordinary one with Play.

package tv.anistream.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.compose.ui.graphics.toArgb
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.store.AppStore
import tv.anistream.store.LiveDownloads
import tv.anistream.ui.AppLink

/** Builds and posts the Downloads channel's notifications. */
class DownloadNotifier(private val context: Context, private val sdk: Int = Build.VERSION.SDK_INT) {
    private val manager = context.getSystemService(NotificationManager::class.java)

    init {
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, context.getString(R.string.downloads_channel), NotificationManager.IMPORTANCE_LOW).apply {
                description = context.getString(R.string.downloads_channel_description)
            },
        )
    }

    /** What the job says the moment it starts, before the queue is known. */
    fun starting(): Notification = frame()
        .setContentTitle(context.getString(R.string.downloads_channel))
        .setContentText("Starting")
        .setProgress(0, 0, true)
        .build()

    /**
     * The ongoing notification for [live]: promoted with ProgressStyle from Android 16, the plain
     * progress bar before it. Pause or Resume, for all where there are several, and Cancel.
     */
    fun ongoing(live: LiveDownloads): Notification {
        val toggle = when {
            live.paused && live.several -> R.string.downloads_resume_all to ACTION_RESUME
            live.paused -> R.string.downloads_resume to ACTION_RESUME
            live.several -> R.string.downloads_pause_all to ACTION_PAUSE
            else -> R.string.downloads_pause to ACTION_PAUSE
        }
        val builder = frame()
            .setContentTitle(live.title)
            .setContentText(live.text)
            .addAction(0, context.getString(toggle.first), action(toggle.second))
            .addAction(0, context.getString(R.string.downloads_cancel), action(ACTION_CANCEL))
        if (sdk >= LIVE_UPDATES) {
            val ends = live.segments.runningReduce(Int::plus).dropLast(1)
            builder
                .setStyle(
                    NotificationCompat.ProgressStyle()
                        .setStyledByProgress(false)
                        .setProgressSegments(live.segments.map { NotificationCompat.ProgressStyle.Segment(it).setColor(WATER) })
                        .setProgressPoints(ends.map { NotificationCompat.ProgressStyle.Point(it).setColor(SILK) })
                        .setProgress(live.progress)
                        .setProgressIndeterminate(live.indeterminate),
                )
                .setRequestPromotedOngoing(true)
            live.chip?.let(builder::setShortCriticalText)
        } else {
            builder.setProgress(LiveDownloads.BAR, live.progress, live.indeterminate)
        }
        return builder.build()
    }

    /** "Episode 8 downloaded", "Stardust Wayfarer · ready to watch offline", with Play. */
    fun finished(row: DownloadRow): Notification = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        // Amber is only for Play once an episode is down.
        .setColor(AMBER)
        .setContentTitle("Episode ${row.episode} downloaded")
        .setContentText("${row.title} · ready to watch offline")
        .setAutoCancel(true)
        .setContentIntent(open(AppLink.Downloads))
        .addAction(0, context.getString(R.string.download_play), open(AppLink.Download(row.id)))
        .build()

    /** Post [row]'s finished notification, which stays once the job's own goes. */
    fun postFinished(row: DownloadRow) {
        manager.notify(finishedTag(row.id), FINISHED_ID, finished(row))
    }

    /** The ongoing notification's frame: the mark in water, silent after the first, opening Downloads. */
    private fun frame(): NotificationCompat.Builder = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        .setColor(WATER)
        .setOnlyAlertOnce(true)
        .setOngoing(true)
        .setCategory(NotificationCompat.CATEGORY_PROGRESS)
        .setContentIntent(open(AppLink.Downloads))

    private fun open(link: AppLink): PendingIntent = PendingIntent.getActivity(
        context,
        0,
        context.linkIntent(link).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun action(action: String): PendingIntent = PendingIntent.getBroadcast(
        context,
        0,
        Intent(action, null, context, DownloadActionReceiver::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    companion object {
        const val CHANNEL = "downloads"

        /** Android 16, where the notification is a Live Update. */
        const val LIVE_UPDATES = 36

        /** Each finished episode's notification goes by its tag under this id. */
        const val FINISHED_ID = 4102

        const val ACTION_PAUSE = "tv.anistream.action.DOWNLOADS_PAUSE"
        const val ACTION_RESUME = "tv.anistream.action.DOWNLOADS_RESUME"
        const val ACTION_CANCEL = "tv.anistream.action.DOWNLOADS_CANCEL"

        fun finishedTag(id: Long) = "download/$id"

        /** Download progress is water, as everywhere in the app. */
        private val WATER = ObiSilk.Water.toArgb()
        private val SILK = ObiSilk.Silk.toArgb()
        private val AMBER = ObiSilk.Amber.toArgb()
    }
}

/**
 * The ongoing notification's buttons, which act on the queue: Pause (all), Resume (all) and
 * Cancel, which takes every episode not yet down off it. The job keeps the process, and so the
 * engine, up while its notification shows.
 */
class DownloadActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as AnistreamApp
        val act: (AppStore) -> Unit = when (intent.action) {
            DownloadNotifier.ACTION_PAUSE -> AppStore::pauseAllDownloads
            DownloadNotifier.ACTION_RESUME -> AppStore::resumeAllDownloads
            DownloadNotifier.ACTION_CANCEL -> { store -> cancelAll(store) }
            else -> return
        }
        app.start(Flavour.launch(context, Intent()))
        val pending = goAsync()
        MainScope().launch {
            try {
                // A process started for the button knows the queue once the engine says it.
                if (!app.store.downloads.loaded.value) {
                    app.store.loadDownloads(force = true)
                    withTimeoutOrNull(LOAD_MILLIS) { app.store.downloads.loaded.first { it } }
                }
                act(app.store)
            } finally {
                pending.finish()
                cancel()
            }
        }
    }

    private companion object {
        const val LOAD_MILLIS = 5_000L

        /** Every episode not yet down, off the queue. */
        fun cancelAll(store: AppStore) {
            for (row in store.downloads.rows.value) {
                if (row.state == DownloadState.ACTIVE || row.state == DownloadState.QUEUED || row.state == DownloadState.PAUSED) {
                    store.cancelDownload(row.id)
                }
            }
        }
    }
}
