// The pieces both widgets are drawn from, in Obi & Silk's fixed colours: the system draws widgets
// in no wallpaper colour of ours, and the titles are in the system sans, the one family a
// widget's RemoteViews can use (NativeWidgetsAndroid).

package tv.anistream.widgets

import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.appwidget.LinearProgressIndicator
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.appwidget.cornerRadius
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ContentScale
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontFamily
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import java.time.Instant
import java.time.ZoneId
import tv.anistream.ui.AppLink

/** Obi & Silk, as the widgets use it. */
internal object Palette {
    val ground = Color(0xFF161A2E)
    val rule = Color(0xFF434653)
    val silk = Color(0xFFE4E0D6)
    val haze = Color(0xFF9298B5)
    val amber = Color(0xFFF2A64B)
}

/** A widget's text, in the system sans. */
internal fun words(size: TextUnit, color: Color = Palette.silk, weight: FontWeight = FontWeight.Normal) =
    TextStyle(color = ColorProvider(color), fontSize = size, fontWeight = weight, fontFamily = FontFamily.SansSerif)

/** What a widget draws with: where it opens, its art, and the moment it is drawn for. */
internal class Drawing(
    /** The action that opens the app on a link, or as the launcher does for `null`. */
    val open: (AppLink?) -> Action,
    /** A record's art, cut to a size in pixels; `null` while it is not kept. */
    val art: (url: String?, width: Int, height: Int) -> Bitmap?,
    val now: Instant,
    val zone: ZoneId,
    val words: Words,
) {
    fun airs(at: Instant): String = words.airs(at, now, zone)
}

/** The widget's own ground: its background, with the system's corners. */
internal fun root(onClick: Action?): GlanceModifier {
    val ground = GlanceModifier.fillMaxSize().appWidgetBackground().background(Palette.ground)
        .cornerRadius(android.R.dimen.system_app_widget_background_radius)
    return if (onClick == null) ground else ground.clickable(onClick)
}

/**
 * A title's art at [width] × [height], cut from the middle, with [radius] corners: its plate, in
 * the cover's colour or the raised ground, while the art is not there. The corners are in the
 * picture itself rather than clipped, so they hold however the launcher draws.
 */
@Composable
internal fun Art(drawing: Drawing, url: String?, plate: Long?, width: Dp, height: Dp, radius: Dp, modifier: GlanceModifier = GlanceModifier) {
    // Drawn at no more than 2.5 × the size in dp: the launcher scales it, and a widget's
    // pictures share one memory budget.
    val density = LocalContext.current.resources.displayMetrics.density.coerceAtMost(2.5f)
    val (w, h, r) = Triple((width.value * density).toInt(), (height.value * density).toInt(), radius.value * density)
    val image = drawing.art(url, w, h)?.let { WidgetArt.rounded(it, r) }
        ?: WidgetArt.plate((plate ?: PLATE).toInt(), w, h, r)
    Image(ImageProvider(image), contentDescription = null, modifier = modifier.size(width, height), contentScale = ContentScale.FillBounds)
}

/** The plate's colour when the record names none: the raised ground. */
private const val PLATE = 0xFF222638L

/**
 * The play button: a 40 dp circle in a 48 dp target. [filled] is amber, the one play action a
 * layout has; otherwise the dimmed glass over the art.
 */
@Composable
internal fun Play(onClick: Action, label: String, filled: Boolean) {
    Box(
        GlanceModifier.size(48.dp).clickable(onClick).semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            GlanceModifier.size(40.dp).background(ImageProvider(if (filled) R.drawable.widget_play_amber else R.drawable.widget_glass)),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                ImageProvider(R.drawable.widget_play),
                contentDescription = null,
                modifier = GlanceModifier.size(12.dp),
                colorFilter = ColorFilter.tint(ColorProvider(if (filled) Palette.ground else Palette.silk)),
            )
        }
    }
}

/** Progress through an episode: the amber line on its track, 4 dp. */
@Composable
internal fun Bar(progress: Float, modifier: GlanceModifier = GlanceModifier.fillMaxWidth()) {
    LinearProgressIndicator(
        progress = progress,
        modifier = modifier.height(4.dp).cornerRadius(2.dp),
        color = ColorProvider(Palette.amber),
        backgroundColor = ColorProvider(Palette.rule),
    )
}

/**
 * A widget with nothing of its own to show: what it is, a line on why or what next, and at most
 * one action (NativeWidgetsStates).
 */
@Composable
internal fun Message(title: String, body: String, onClick: Action?, action: String? = null) {
    Column(root(onClick).padding(start = 14.dp, end = 14.dp, top = 14.dp, bottom = 12.dp)) {
        Text(title, style = words(13.sp, weight = FontWeight.Medium), maxLines = 1)
        Spacer(GlanceModifier.height(6.dp))
        Text(body, style = words(13.sp, Palette.haze), maxLines = 4)
        if (action != null && onClick != null) {
            Spacer(GlanceModifier.height(10.dp))
            Box(
                GlanceModifier.height(40.dp).background(ImageProvider(R.drawable.widget_pill)).padding(horizontal = 16.dp)
                    .clickable(onClick),
                contentAlignment = Alignment.Center,
            ) {
                Text(action, style = words(14.sp, weight = FontWeight.Medium), maxLines = 1)
            }
        }
    }
}
