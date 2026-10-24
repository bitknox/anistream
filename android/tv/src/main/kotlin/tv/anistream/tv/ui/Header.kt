// The header Continue's hero and the title page share: the banner bleeding full width from y 0 to
// 200 with no text on it, then the title on the ground at the keyline. With no banner and no
// still safe to show, a compact header: the poster at the keyline beside the title.

package tv.anistream.tv.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import tv.anistream.design.CoverPlate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.store.HeroArt

/** The banner's height on the canvas. */
val BannerHeight = 200.dp

/** The banners are decoded 1920 px wide at most, the canvas's width in pixels. */
private const val BANNER_PIXELS = 1920

@Composable
fun HeroHeader(entry: Entry, rows: List<EpisodeRow>?, modifier: Modifier = Modifier, below: @Composable ColumnScope.() -> Unit) {
    val art = HeroArt.of(entry, rows)
    if (art == HeroArt.Compact) {
        Row(modifier.fillMaxWidth().padding(start = Canvas.Keyline, top = Canvas.SafeY + 20.dp, end = Canvas.SafeX)) {
            ArtImage(
                entry.coverUrl,
                CoverPlate.parse(entry.coverColor),
                pixels(Canvas.PosterHeight),
                Modifier.size(Canvas.PosterWidth, Canvas.PosterHeight).clip(RoundedCornerShape(8.dp)),
            )
            Spacer(Modifier.width(24.dp))
            Column(Modifier.padding(top = 8.dp)) {
                HeaderTitle(entry.title)
                below()
            }
        }
        return
    }
    Column(modifier.fillMaxWidth()) {
        Box(Modifier.fillMaxWidth().height(BannerHeight)) {
            // Waiting for the episode list keeps the ground: never a cover stretched across.
            if (art.url != null) ArtImage(art.url, plate = ObiSilk.Ground, maxPixelSize = BANNER_PIXELS, modifier = Modifier.fillMaxSize())
        }
        Column(Modifier.padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 12.dp)) {
            HeaderTitle(entry.title)
            below()
        }
    }
}

@Composable
private fun HeaderTitle(title: String) {
    // Two lines before the ellipsis: long titles are common, and the second line costs little.
    Text(title, style = TvTitle.page, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { heading() })
}
