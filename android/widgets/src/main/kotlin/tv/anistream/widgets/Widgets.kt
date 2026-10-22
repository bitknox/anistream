// Keeping the widgets drawn: again whenever the record changes, and at the next moment what they
// show changes on its own: a broadcast airing, the record turning a day old, midnight.

package tv.anistream.widgets

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.updateAll
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.concurrent.TimeUnit
import tv.anistream.engine.model.WidgetData
import tv.anistream.store.AppClock

object Widgets {
    private const val TAG = "anistream.widgets"

    /** The one redraw waiting, replaced each time it is planned again. */
    const val REDRAW = "widgets-redraw"

    /** How often the previews in the widget picker are made again, at most: the system limits it. */
    private const val PREVIEWS_EVERY_MILLIS = 24L * 3_600_000

    /**
     * Draw both widgets again from the record [data], and plan the next redraw. What the app calls
     * each time the record is written.
     */
    suspend fun update(context: Context, data: WidgetData?) {
        ContinueWidget().updateAll(context)
        AiringWidget().updateAll(context)
        plan(context, data)
    }

    /** Whether any widget of either kind is on a home screen. */
    suspend fun placed(context: Context): Boolean {
        val manager = GlanceAppWidgetManager(context)
        return manager.getGlanceIds(ContinueWidget::class.java).isNotEmpty() ||
            manager.getGlanceIds(AiringWidget::class.java).isNotEmpty()
    }

    /** Plan the redraw at the next moment the widgets change on their own, while any is placed. */
    suspend fun plan(context: Context, data: WidgetData?) {
        val work = WorkManager.getInstance(context)
        if (!placed(context)) {
            work.cancelUniqueWork(REDRAW)
            return
        }
        val now = AppClock.now()
        val at = nextRedraw(data?.let(Record::of), now, AppClock.zone) ?: return
        val delay = Duration.between(now, at).toMillis().coerceAtLeast(0)
        work.enqueueUniqueWork(
            REDRAW,
            ExistingWorkPolicy.REPLACE,
            OneTimeWorkRequestBuilder<RedrawWork>().setInitialDelay(delay, TimeUnit.MILLISECONDS).build(),
        )
    }

    /**
     * The next moment after [now] that what the widgets show changes without a new record: a
     * broadcast of today's airing, Continue's record turning a day old, or the next midnight.
     */
    fun nextRedraw(record: Record?, now: Instant, zone: ZoneId): Instant {
        val midnight = LocalDate.ofInstant(now, zone).plusDays(1).atStartOfDay(zone).toInstant()
        val moments = buildList {
            add(midnight)
            record?.generatedAt?.plusSeconds(WidgetModel.CONTINUE_FRESH_SECS + 1)?.let(::add)
            record?.airing?.forEach { add(it.airsAt) }
        }
        return moments.filter { it.isAfter(now) }.min()
    }

    /**
     * Make the picker's previews again from the record, at most once a day: they show the
     * viewer's own titles once there is a record, and the sign-in step before.
     */
    suspend fun publishPreviews(context: Context, preferences: SharedPreferences = context.getSharedPreferences("widgets", Context.MODE_PRIVATE)) {
        val now = System.currentTimeMillis()
        if (now - preferences.getLong("previews_at", 0) < PREVIEWS_EVERY_MILLIS) return
        val manager = GlanceAppWidgetManager(context)
        runCatching {
            for (receiver in listOf(ContinueSmallReceiver::class, ContinueWideReceiver::class, AiringSmallReceiver::class, AiringWideReceiver::class)) {
                manager.setWidgetPreviews(receiver)
            }
            preferences.edit().putLong("previews_at", now).apply()
        }.onFailure { Log.w(TAG, "the widget previews were not published: ${it.message}") }
    }
}

/** The planned redraw: both widgets again, a new record asked for once today has none, and the next planned. */
class RedrawWork(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val host = widgetHost(applicationContext)
        val data = host?.widgetRecord?.value
        if (WidgetModel.airing(data?.let(Record::of), AppClock.now(), AppClock.zone) == AiringView.NoSchedule) {
            host?.refreshWidgetRecord()
        }
        Widgets.update(applicationContext, data)
        return Result.success()
    }
}
