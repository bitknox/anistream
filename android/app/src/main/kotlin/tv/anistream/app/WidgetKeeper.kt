// The app's side of the home-screen widgets: it keeps their record and art, made on the engine,
// and draws the widgets again each time the record changes.

package tv.anistream.app

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.WidgetData
import tv.anistream.store.AppStore
import tv.anistream.store.covers
import tv.anistream.widgets.WidgetArt
import tv.anistream.widgets.Widgets

/**
 * Keeps the widgets' [record] and its [art]. A record is made on an engine, the background
 * check's or the app's: its art is fetched through that engine first, as every picture is, and
 * kept where the widgets read it, and then the record is written. Each record written draws both
 * widgets again.
 */
class WidgetKeeper(private val context: Context, val record: WidgetRecord, val art: WidgetArt) {
    /** Make a new record on [store]'s engine and keep it, with its art. */
    suspend fun make(store: AppStore) {
        val data = store.ask(Command.WidgetSnapshot, SNAPSHOT_MILLIS) { (it as? Event.WidgetSnapshot)?.v1 } ?: run {
            Log.w(AnistreamApp.TAG, "the widgets' record was not made in time")
            return
        }
        keep(store, data)
    }

    /** Keep [data], made on [store]'s engine: the art it names that is not kept yet, then the record. */
    suspend fun keep(store: AppStore, data: WidgetData) {
        val urls = artOf(data)
        val missing = urls.filterNot(art::has)
        if (missing.isNotEmpty()) {
            for ((url, image) in store.covers(missing, ART_SIZE, ART_MILLIS)) art.keep(url, image)
        }
        art.keepOnly(urls.toSet())
        record.write(data)
    }

    /** Draw the widgets again each time a record is written, from now on in [scope]. */
    fun follow(scope: CoroutineScope) {
        scope.launch {
            record.latest.drop(1).collect { data ->
                Widgets.update(context, data)
                Widgets.publishPreviews(context)
            }
        }
    }

    companion object {
        /** A widget's art at its largest, the 4 × 3's banner: its long side, in pixels. */
        const val ART_SIZE = 720

        const val ART_MILLIS = 10_000L

        const val SNAPSHOT_MILLIS = 20_000L

        /** The art [data]'s widgets draw: each title's wide art and its poster, for the titles Continue draws. */
        fun artOf(data: WidgetData): List<String> =
            data.continueWatching.take(3).flatMap { listOfNotNull(it.bannerUrl ?: it.coverUrl, it.coverUrl) }.distinct()
    }
}

/**
 * A new record for the widgets, asked for by a widget with none for today: made on the app's
 * engine while it runs, or on a quiet one of its own.
 */
class WidgetRecordWork(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val app = applicationContext as? AnistreamApp ?: return Result.failure()
        return runCatching { app.makeWidgetRecord() }.fold(
            onSuccess = { Result.success() },
            onFailure = {
                Log.w(AnistreamApp.TAG, "the widgets' record was not made: ${it.message}")
                if (runAttemptCount < 2) Result.retry() else Result.failure()
            },
        )
    }

    companion object {
        const val NAME = "widgets-record"

        /** Ask for one, unless one is on its way. */
        fun ask(context: Context) {
            WorkManager.getInstance(context)
                .enqueueUniqueWork(NAME, ExistingWorkPolicy.KEEP, OneTimeWorkRequestBuilder<WidgetRecordWork>().build())
        }
    }
}
