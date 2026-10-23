// Covers, banners and stills: the plate first, the art fading in over it once it arrives, decoded
// at the size it is drawn. Spoiler stills are drawn from a 24 × 14 decode, scaled up.

package tv.anistream.tv.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.core.graphics.scale
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import tv.anistream.design.CoverPlate
import tv.anistream.design.Motion
import tv.anistream.store.AppStore
import tv.anistream.store.Artwork
import tv.anistream.store.ArtworkDecoder

/** The store the screens read and send through. */
val LocalAppStore = staticCompositionLocalOf<AppStore> { error("no AppStore in this composition") }

/** How many bytes of decoded art the TV keeps: 40 MB, as a 2 GB box shares the rest with the video. */
const val TV_ART_BUDGET = 40L shl 20

/**
 * The TV's decoder: opaque art (covers, cards, stills) in RGB_565, half the memory, and banners,
 * whose skies band in 565, in ARGB_8888. A banner is told by its shape, wider than 2.5 : 1.
 */
object TvArtDecoder : ArtworkDecoder {
    override fun decode(bytes: ByteArray, maxPixelSize: Int): Artwork? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val longSide = maxOf(bounds.outWidth, bounds.outHeight)
        if (longSide <= 0) return null
        var sample = 1
        while (longSide / (sample * 2) >= maxPixelSize.coerceAtLeast(1)) sample *= 2
        val banner = bounds.outHeight > 0 && bounds.outWidth.toFloat() / bounds.outHeight > 2.5f
        val options = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = if (banner) Bitmap.Config.ARGB_8888 else Bitmap.Config.RGB_565
        }
        val image = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options) ?: return null
        image.prepareToDraw()
        return Artwork(image, maxPixelSize, longSide)
    }
}

/** The pixels a box [dp] wide or high needs. */
@Composable
fun pixels(dp: Dp): Int = with(LocalDensity.current) { dp.toPx().roundToInt() }

/**
 * Art from [url] filling its box over the plate in [plate]'s muted colour, fading in when it
 * arrives, never popping in. Asked for at [maxPixelSize] pixels on its long side. With [spoiler],
 * drawn from a 24 × 14 copy scaled up with filtering: the still's colours, none of its detail.
 */
@Composable
fun ArtImage(url: String?, plate: Color?, maxPixelSize: Int, modifier: Modifier = Modifier, spoiler: Boolean = false) {
    val store = LocalAppStore.current
    // This URL's arrivals alone: another cover landing leaves this one be.
    val arrived = remember(url) { url?.let(store.artwork::arrivals) ?: NONE_ARRIVED }
    val arrivals by arrived.collectAsStateWithLifecycle()
    val art = url?.let { arrivals; store.artwork.image(it) }
    // Asked again when the art goes, as when the image cache is cleared under it.
    LaunchedEffect(url, maxPixelSize, art == null) {
        if (url != null) store.artwork.request(url, maxPixelSize)
    }
    // Art already here when the box first draws shows at once; art that arrives fades in.
    val shown = remember(url) { Animatable(if (art != null) 1f else 0f) }
    LaunchedEffect(url, art != null) {
        if (art != null) shown.animateTo(1f, Motion.fadeIn())
    }
    Box(modifier) {
        CoverPlate(plate, Modifier.fillMaxSize())
        if (art != null) {
            val bitmap = remember(art, spoiler) {
                val image = if (spoiler) art.image.scale(SPOILER_WIDTH, SPOILER_HEIGHT) else art.image
                image.asImageBitmap()
            }
            Image(
                bitmap,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                // Bilinear, which is what scales the spoiler copy up smoothly, and costs nothing.
                filterQuality = FilterQuality.Low,
                modifier = Modifier.fillMaxSize().graphicsLayer { alpha = shown.value },
            )
        }
    }
}

private const val SPOILER_WIDTH = 24
private const val SPOILER_HEIGHT = 14

/** What a box with no URL observes: nothing ever lands. */
private val NONE_ARRIVED: StateFlow<Int> = MutableStateFlow(0)
