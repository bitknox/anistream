// Continue watching: the next episode or episodes, with progress. One widget that changes layout
// as it is resized (NativeWidgetsAndroid): one show at 2 × 2, its art beside the text at 3 × 2,
// two shows at 4 × 2, and the next episode with two more at 4 × 3.

package tv.anistream.widgets

import android.content.Context
import android.text.format.DateFormat
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.LocalSize
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxHeight
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import java.util.Locale
import kotlinx.coroutines.flow.MutableStateFlow
import tv.anistream.engine.model.WidgetData
import tv.anistream.store.AppClock
import tv.anistream.ui.AppLink

/** Continue watching. [hostOf] finds the app it runs in; the tests give one of their own. */
class ContinueWidget(private val hostOf: (Context) -> WidgetHost? = ::widgetHost) : GlanceAppWidget() {
    // Exact rather than Responsive's four sizes: the layouts change at those same sizes, and the
    // art is cut to the widget's real size, with its corners in the picture, where a size bucket
    // would leave it short of a wider widget's edge.
    override val sizeMode = SizeMode.Exact

    override val previewSizeMode = SizeMode.Responsive(setOf(SMALL, WIDE))

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val host = hostOf(context)
        if (host?.widgetRecord?.value == null) host?.refreshWidgetRecord()
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
        ContinueContent(WidgetModel.continuing(data?.let(Record::of), drawing.now, drawing.zone, drawing.words), drawing)
    }

    companion object {
        val SMALL = DpSize(160.dp, 160.dp)
        val BESIDE = DpSize(250.dp, 160.dp)
        val WIDE = DpSize(340.dp, 160.dp)
        val TALL = DpSize(340.dp, 250.dp)

        /** The four layouts, each from the largest of these the widget fits as it is resized. */
        val SIZES = setOf(SMALL, BESIDE, WIDE, TALL)
    }
}

/** How a widget in [context] draws for [host]: its links, its art, and the clock. */
internal fun drawing(context: Context, host: WidgetHost?) = Drawing(
    open = { link -> androidx.glance.appwidget.action.actionStartActivity(host?.widgetIntent(link) ?: android.content.Intent()) },
    art = { url, width, height -> host?.widgetArt?.image(url, width, height) },
    now = AppClock.now(),
    zone = AppClock.zone,
    words = Words(DateFormat.is24HourFormat(context), Locale.getDefault()),
)

/** The picker's 2 × 2 Continue watching. */
class ContinueSmallReceiver : AnistreamWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = ContinueWidget()
}

/** The picker's 4 × 2 Continue watching: the same widget, placed wider. */
class ContinueWideReceiver : AnistreamWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = ContinueWidget()
}

@Composable
internal fun ContinueContent(view: ContinueView, drawing: Drawing) {
    when (view) {
        ContinueView.SignedOut -> Message(
            title = "Continue",
            body = "Connect AniList, or play an episode, to see it here.",
            onClick = drawing.open(AppLink.Accounts),
            action = "Connect",
        )
        is ContinueView.CaughtUp -> {
            val next = view.next
            if (next == null) {
                Message("Continue", "You’re caught up.", drawing.open(null))
            } else {
                Message("Caught up", "${next.title} E${next.episode} airs ${drawing.airs(next.airsAt)}.", drawing.open(AppLink.Title(next.id)))
            }
        }
        is ContinueView.Shows -> {
            val size = LocalSize.current
            val shows = view.shows
            when {
                size.width < ContinueWidget.BESIDE.width -> Small(shows.first(), view.updated, drawing)
                size.height >= ContinueWidget.TALL.height && size.width >= ContinueWidget.TALL.width -> Tall(shows, view.updated, drawing)
                size.width < ContinueWidget.WIDE.width || shows.size == 1 -> Beside(shows.first(), view.updated, drawing)
                else -> Two(shows.take(2), view.updated, drawing)
            }
        }
    }
}

/** What a show says under its title: its progress, or how old the record is once it is stale. */
@Composable
private fun Progress(show: ContinueView.Show, updated: String?, gap: Int) {
    when {
        updated != null -> Text(updated, style = words(12.sp, Palette.haze), maxLines = 1)
        show.progress != null -> {
            Spacer(GlanceModifier.height(gap.dp))
            Bar(show.progress)
        }
    }
}

/** 2 × 2: the art across the top, the amber play button on its edge, the title and progress. */
@Composable
private fun Small(show: ContinueView.Show, updated: String?, drawing: Drawing) {
    val width = LocalSize.current.width
    Box(root(drawing.open(AppLink.Title(show.id)))) {
        Column(GlanceModifier.fillMaxSize()) {
            Art(drawing, show.wideUrl, show.plate, width, 84.dp, 0.dp)
            Column(GlanceModifier.fillMaxWidth().padding(start = 14.dp, end = 14.dp, top = 12.dp)) {
                Text(show.title, style = words(15.sp, weight = FontWeight.Medium), maxLines = 1)
                Text(show.line, style = words(13.sp, Palette.haze), maxLines = 1)
                Progress(show, updated, gap = 8)
            }
        }
        Box(GlanceModifier.fillMaxSize().padding(top = 56.dp, end = 8.dp), contentAlignment = Alignment.TopEnd) {
            Play(drawing.open(AppLink.Continue(show.id)), playLabel(show), filled = true)
        }
    }
}

/** 3 × 2: the art beside the text, and the amber play button in the corner. */
@Composable
private fun Beside(show: ContinueView.Show, updated: String?, drawing: Drawing) {
    val height = LocalSize.current.height
    Box(root(drawing.open(AppLink.Title(show.id)))) {
        Row(GlanceModifier.fillMaxSize().padding(12.dp)) {
            Art(drawing, show.posterUrl ?: show.wideUrl, show.plate, 120.dp, height - 24.dp, 16.dp)
            Spacer(GlanceModifier.width(14.dp))
            Column(GlanceModifier.defaultWeight().fillMaxHeight().padding(top = 2.dp)) {
                Text(show.title, style = words(15.sp, weight = FontWeight.Medium), maxLines = 2)
                Text(show.line, style = words(13.sp, Palette.haze), maxLines = 1)
                Progress(show, updated, gap = 8)
            }
        }
        Box(GlanceModifier.fillMaxSize().padding(end = 12.dp, bottom = 12.dp), contentAlignment = Alignment.BottomEnd) {
            Play(drawing.open(AppLink.Continue(show.id)), playLabel(show), filled = true)
        }
    }
}

/** 4 × 2: two shows as cards, each with the dimmed play button over its art; amber stays on progress. */
@Composable
private fun Two(shows: List<ContinueView.Show>, updated: String?, drawing: Drawing) {
    val card = (LocalSize.current.width - 36.dp) / 2
    Box(root(null)) {
        Row(GlanceModifier.fillMaxSize().padding(12.dp)) {
            shows.forEachIndexed { index, show ->
                if (index > 0) Spacer(GlanceModifier.width(12.dp))
                Column(GlanceModifier.defaultWeight().clickable(drawing.open(AppLink.Title(show.id)))) {
                    Box(contentAlignment = Alignment.TopEnd) {
                        Art(drawing, show.wideUrl, show.plate, card, 88.dp, 16.dp)
                        Box(GlanceModifier.padding(top = 40.dp, end = 4.dp)) {
                            Play(drawing.open(AppLink.Continue(show.id)), playLabel(show), filled = false)
                        }
                    }
                    Spacer(GlanceModifier.height(8.dp))
                    Text(show.title, style = words(14.sp, weight = FontWeight.Medium), maxLines = 1)
                    Text(show.line, style = words(12.sp, Palette.haze), maxLines = 1)
                    if (updated == null && show.progress != null) {
                        Spacer(GlanceModifier.height(6.dp))
                        Bar(show.progress)
                    }
                }
            }
        }
        if (updated != null) {
            Box(GlanceModifier.fillMaxSize().padding(end = 14.dp, bottom = 10.dp), contentAlignment = Alignment.BottomEnd) {
                Text(updated, style = words(12.sp, Palette.haze), maxLines = 1)
            }
        }
    }
}

/** 4 × 3: the next episode with its art and the amber play button, then two more as rows. */
@Composable
private fun Tall(shows: List<ContinueView.Show>, updated: String?, drawing: Drawing) {
    val first = shows.first()
    val width = LocalSize.current.width - 24.dp
    Column(root(null).padding(12.dp)) {
        Column(GlanceModifier.fillMaxWidth().clickable(drawing.open(AppLink.Title(first.id)))) {
            Box(contentAlignment = Alignment.TopEnd) {
                Art(drawing, first.wideUrl, first.plate, width, 96.dp, 16.dp)
                Box(GlanceModifier.padding(top = 40.dp, end = 8.dp)) {
                    Play(drawing.open(AppLink.Continue(first.id)), playLabel(first), filled = true)
                }
            }
            Spacer(GlanceModifier.height(8.dp))
            Text(first.title, style = words(15.sp, weight = FontWeight.Medium), maxLines = 1)
            Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(first.line, style = words(12.sp, Palette.haze), maxLines = 1)
                Spacer(GlanceModifier.width(10.dp))
                when {
                    updated != null -> Text(updated, style = words(12.sp, Palette.haze), maxLines = 1)
                    first.progress != null -> Bar(first.progress, GlanceModifier.defaultWeight())
                }
            }
        }
        for (show in shows.drop(1).take(2)) {
            Row(
                GlanceModifier.fillMaxWidth().height(52.dp).clickable(drawing.open(AppLink.Title(show.id))),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Art(drawing, show.posterUrl ?: show.wideUrl, show.plate, 32.dp, 48.dp, 6.dp)
                Spacer(GlanceModifier.width(12.dp))
                Column(GlanceModifier.defaultWeight()) {
                    Text(show.title, style = words(14.sp, weight = FontWeight.Medium), maxLines = 1)
                    Text(show.line, style = words(12.sp, Palette.haze), maxLines = 1)
                }
            }
        }
    }
}

/**
 * What the play button says to a screen reader: "Resume episode 7 of …", or "Play episode 4 of …",
 * and for a dub the mark as the row shows it: "Resume episode 7, Dub, of …".
 */
internal fun playLabel(show: ContinueView.Show): String {
    val episode = listOfNotNull("episode ${show.episode}", show.mark).joinToString(", ")
    return "${if (show.progress != null) "Resume" else "Play"} $episode${if (show.mark != null) "," else ""} of ${show.title}"
}
