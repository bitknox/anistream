// Airing today: the day's times, everything airing, with the shows in the list marked by a
// bookmark and a silk title and the others in haze. Two layouts (NativeWidgetsAndroid): the times
// still to come at 2 × 2, and from 4 × 2 the times with each episode and its state.

package tv.anistream.widgets

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalSize
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import kotlinx.coroutines.flow.MutableStateFlow
import tv.anistream.engine.model.WidgetData
import tv.anistream.ui.AppLink

/** Airing today. [hostOf] finds the app it runs in; the tests give one of their own. */
class AiringWidget(private val hostOf: (Context) -> WidgetHost? = ::widgetHost) : GlanceAppWidget() {
    // Exact, not a few sizes: how many rows fit is the widget's own height.
    override val sizeMode = SizeMode.Exact

    override val previewSizeMode = SizeMode.Responsive(setOf(ContinueWidget.SMALL, ContinueWidget.WIDE))

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val host = hostOf(context)
        val drawing = drawing(context, host)
        // No record, or none that says anything of today: ask for one, and say so meanwhile.
        val record = host?.widgetRecord?.value?.let(Record::of)
        if (WidgetModel.airing(record, drawing.now, drawing.zone) == AiringView.NoSchedule) host?.refreshWidgetRecord()
        provideContent {
            // Followed while the widget's session lasts, so a record landing meanwhile is drawn.
            val data by (host?.widgetRecord ?: MutableStateFlow(null)).collectAsState()
            Drawn(context, host, data)
        }
    }

    override suspend fun providePreview(context: Context, widgetCategory: Int) {
        // The record as it is, not followed: a preview's composition runs once and waits for
        // every effect in it to end, and following the record never would.
        val host = hostOf(context)
        provideContent { Drawn(context, host, host?.widgetRecord?.value) }
    }

    @Composable
    private fun Drawn(context: Context, host: WidgetHost?, data: WidgetData?) {
        val drawing = drawing(context, host)
        AiringContent(WidgetModel.airing(data?.let(Record::of), drawing.now, drawing.zone, drawing.words), drawing)
    }

    companion object {
        /** From this wide, each row says its episode and state. */
        val WIDE_FROM: Dp = 240.dp
    }
}

/** The picker's 2 × 2 Airing today. */
class AiringSmallReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = AiringWidget()
}

/** The picker's 4 × 2 Airing today: the same widget, placed wider. */
class AiringWideReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = AiringWidget()
}

@Composable
internal fun AiringContent(view: AiringView, drawing: Drawing) {
    val calendar = drawing.open(AppLink.Calendar)
    when (view) {
        AiringView.NoSchedule -> Message("Airing today", "No schedule for today yet. Open anistream to update it.", calendar)
        is AiringView.NothingToday -> Message(
            "Airing today",
            view.next?.let { "Nothing airing. Next: ${drawing.airs(it.airsAt)}." } ?: "Nothing airing today.",
            calendar,
        )
        is AiringView.Today -> {
            val size = LocalSize.current
            if (size.width < AiringWidget.WIDE_FROM) Times(view, size, drawing) else TimesAndStates(view, size, drawing)
        }
    }
}

/** How wide the times are: "21:00", or "10:30 PM" on a twelve-hour clock. */
private fun timeWidth(drawing: Drawing, base: Dp): Dp = if (drawing.words.twentyFourHour) base else base + 18.dp

/** 2 × 2: the times still to come, each with its title and the bookmark for a show in the list. */
@Composable
private fun Times(view: AiringView.Today, size: DpSize, drawing: Drawing) {
    val count = ((size.height - 48.dp) / 28.dp).toInt().coerceAtLeast(1)
    Column(root(drawing.open(AppLink.Calendar)).padding(start = 16.dp, end = 14.dp, top = 14.dp)) {
        Text("Airing today", modifier = GlanceModifier.height(22.dp), style = words(14.sp, weight = FontWeight.Medium), maxLines = 1)
        Spacer(GlanceModifier.height(6.dp))
        for (row in WidgetModel.pick(view.rows, count, withOut = false)) {
            Row(
                GlanceModifier.fillMaxWidth().height(28.dp).clickable(drawing.open(AppLink.Title(row.id))),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(row.time, modifier = GlanceModifier.width(timeWidth(drawing, 46.dp)), style = words(13.sp, if (row.aired) Palette.haze else Palette.silk), maxLines = 1)
                Spacer(GlanceModifier.width(8.dp))
                Text(row.title, modifier = GlanceModifier.defaultWeight(), style = words(13.sp, titleColour(row, view.marks), FontWeight.Medium), maxLines = 1)
                if (row.onList) {
                    Spacer(GlanceModifier.width(6.dp))
                    Image(ImageProvider(R.drawable.widget_bookmark), contentDescription = "In your list", modifier = GlanceModifier.size(10.dp))
                }
            }
        }
    }
}

/** 4 × 2: the header with the day, then each time with its mark, title, episode and state. */
@Composable
private fun TimesAndStates(view: AiringView.Today, size: DpSize, drawing: Drawing) {
    val count = ((size.height - 40.dp) / 26.dp).toInt().coerceAtLeast(1)
    Column(root(null).padding(start = 16.dp, end = 16.dp, top = 12.dp)) {
        Row(
            GlanceModifier.fillMaxWidth().height(24.dp).clickable(drawing.open(AppLink.Calendar)),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Airing today", modifier = GlanceModifier.defaultWeight(), style = words(14.sp, weight = FontWeight.Medium), maxLines = 1)
            Text(view.weekday, style = words(12.sp, Palette.haze), maxLines = 1)
        }
        for (row in WidgetModel.pick(view.rows, count, withOut = true)) {
            Row(
                GlanceModifier.fillMaxWidth().height(26.dp).clickable(drawing.open(AppLink.Title(row.id))),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(row.time, modifier = GlanceModifier.width(timeWidth(drawing, 50.dp)), style = words(13.sp, if (row.aired) Palette.haze else Palette.silk), maxLines = 1)
                Spacer(GlanceModifier.width(8.dp))
                Box(GlanceModifier.width(12.dp), contentAlignment = Alignment.CenterStart) {
                    if (row.onList) {
                        Image(ImageProvider(R.drawable.widget_bookmark), contentDescription = "In your list", modifier = GlanceModifier.size(10.dp))
                    }
                }
                Spacer(GlanceModifier.width(8.dp))
                Text(row.title, modifier = GlanceModifier.defaultWeight(), style = words(13.sp, titleColour(row, view.marks), FontWeight.Medium), maxLines = 1)
                Spacer(GlanceModifier.width(8.dp))
                State(row)
            }
        }
    }
}

/** The right-hand column: the water dot for an episode out, the check for one watched, or the episode. */
@Composable
private fun State(row: AiringView.Row) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        when {
            row.out -> {
                Image(ImageProvider(R.drawable.widget_dot), contentDescription = null, modifier = GlanceModifier.size(6.dp))
                Spacer(GlanceModifier.width(5.dp))
            }
            row.watched -> {
                Image(ImageProvider(R.drawable.widget_check), contentDescription = "Watched", modifier = GlanceModifier.size(11.dp))
                Spacer(GlanceModifier.width(4.dp))
            }
        }
        Text(row.state, style = words(12.sp, Palette.haze), maxLines = 1)
    }
}

/** A title in the list is silk and the others haze; with no list to mark, every title is silk. */
private fun titleColour(row: AiringView.Row, marks: Boolean) = if (row.onList || !marks) Palette.silk else Palette.haze
