// The browse screens' parts: posters and 16:9 cards with the TV focus look, rows that keep the
// focused card at the keyline and remember it, and the scroll rules that keep focus inside the
// safe area.

package tv.anistream.tv.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import androidx.tv.material3.Card
import androidx.tv.material3.CardDefaults
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import tv.anistream.design.CoverPlate
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.Entry

/** Rows put the focused card at the content keyline, as they scroll: the brief's TV2. */
class KeylineSpec(private val keyline: Float) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float = offset - keyline
}

/**
 * Grids and lists bring the focused item fully inside the safe area, [leading] and [trailing]
 * pixels in from the container's edges, scrolling no more than that needs.
 */
class SafeSpec(private val leading: Float, private val trailing: Float) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float {
        val end = offset + size
        val limit = containerSize - trailing
        return when {
            offset < leading -> offset - leading
            end > limit -> minOf(end - limit, offset - leading)
            else -> 0f
        }
    }
}

/** No scrolling on a request: the screen scrolls itself, row by row. */
object HeldScroll : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float = 0f
}

/** [content] with requests from beneath answered by [spec]. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ScrollRule(spec: BringIntoViewSpec, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalBringIntoViewSpec provides spec, content = content)
}

/**
 * A row: its header (titleMedium) at the keyline and its cards 12 below, scrolling so the focused
 * card sits at the keyline. Focus coming back into the row returns to the card it last had (its
 * first card if it never had one), and nothing wraps.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun BrowseRow(
    title: String,
    modifier: Modifier = Modifier,
    state: LazyListState = rememberLazyListState(),
    subtitle: String? = null,
    fallback: FocusRequester = FocusRequester.Default,
    content: LazyListScope.() -> Unit,
) {
    val keyline = with(LocalDensity.current) { Canvas.Keyline.toPx() }
    val spec = remember(keyline) { KeylineSpec(keyline) }
    Column(modifier.fillMaxWidth()) {
        Row(Modifier.padding(start = Canvas.Keyline, end = Canvas.SafeX), verticalAlignment = Alignment.Bottom) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, maxLines = 1)
            if (subtitle != null) {
                Spacer(Modifier.width(12.dp))
                Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1)
            }
        }
        Spacer(Modifier.height(Canvas.HeaderGap - Canvas.FocusRoom))
        ScrollRule(spec) {
            LazyRow(
                state = state,
                modifier = Modifier
                    .fillMaxWidth()
                    // Cards scrolled past end before the keyline, so the rail's icons stay on the
                    // ground and no sliver of the card before shows; the focused card's ring and
                    // scale still fit.
                    .drawWithContent { clipRect(left = (Canvas.Keyline - Canvas.CardGap + 4.dp).toPx()) { this@drawWithContent.drawContent() } }
                    .focusRestorer(fallback)
                    .focusGroup(),
                contentPadding = PaddingValues(start = Canvas.Keyline, end = Canvas.SafeX, top = Canvas.FocusRoom, bottom = Canvas.FocusRoom),
                horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap),
                content = content,
            )
        }
    }
}

/**
 * Art with the TV focus look: tv-material's Card, no glow, no shadow, the silk ring 3 dp outside
 * it and the scale on quick, which snaps while a key is held.
 */
@Composable
fun ArtCard(
    onClick: () -> Unit,
    width: Dp,
    height: Dp,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    val shape = RoundedCornerShape(8.dp)
    Card(
        onClick = onClick,
        onLongClick = onLongClick,
        modifier = modifier.size(width, height).focusScale(focused, ArtFocus.scale(width)),
        shape = CardDefaults.shape(shape),
        colors = CardDefaults.colors(containerColor = ObiSilk.Raised, focusedContainerColor = ObiSilk.Raised),
        scale = CardDefaults.scale(scale = 1f, focusedScale = 1f, pressedScale = 1f),
        border = CardDefaults.border(border = Border.None, focusedBorder = ArtFocus.ring(), pressedBorder = ArtFocus.ring()),
        glow = CardDefaults.glow(),
        interactionSource = interactions,
    ) {
        Box(Modifier.fillMaxSize().clip(shape), content = content)
    }
}

/** A poster, 118 × 177, with the title under it in two lines and an optional line below. */
@Composable
fun PosterCard(
    entry: Entry,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    line: String? = null,
    progress: Float? = null,
    onLongClick: (() -> Unit)? = null,
) = PosterCard(entry.title, entry.coverUrl, entry.coverColor, onClick, modifier, line, progress, onLongClick)

/** A poster of any title, by its cover. */
@Composable
fun PosterCard(
    title: String,
    coverUrl: String?,
    coverColor: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    line: String? = null,
    progress: Float? = null,
    onLongClick: (() -> Unit)? = null,
) {
    Column(Modifier.width(Canvas.PosterWidth)) {
        ArtCard(onClick, Canvas.PosterWidth, Canvas.PosterHeight, modifier, onLongClick) {
            ArtImage(coverUrl, CoverPlate.parse(coverColor), POSTER_PIXELS, Modifier.fillMaxSize())
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
        }
        CardText(title, line)
    }
}

/** Posters are decoded at AniList's large cover, 240 × 360. */
private const val POSTER_PIXELS = 360

/** A 16:9 card, 185 × 104, of the show's banner (the cover beside a plate without one). */
@Composable
fun WideCard(
    entry: Entry,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    line: String? = null,
    progress: Float? = null,
    newDot: Boolean = false,
    art: String? = entry.bannerUrl,
    spoiler: Boolean = false,
    title: String = entry.title,
    onLongClick: (() -> Unit)? = null,
) {
    Column(Modifier.width(Canvas.CardWidth)) {
        ArtCard(onClick, Canvas.CardWidth, Canvas.CardHeight, modifier, onLongClick) {
            if (art != null) {
                ArtImage(art, CoverPlate.parse(entry.coverColor), pixels(Canvas.CardWidth), Modifier.fillMaxSize(), spoiler = spoiler)
            } else {
                // No banner: the cover at its own aspect on a plate, never stretched across.
                CoverPlate(CoverPlate.parse(entry.coverColor), Modifier.fillMaxSize())
                ArtImage(
                    entry.coverUrl,
                    plate = null,
                    maxPixelSize = pixels(Canvas.CardHeight),
                    modifier = Modifier.align(Alignment.CenterStart).padding(start = 8.dp).size(60.dp, 90.dp).clip(RoundedCornerShape(6.dp)),
                )
            }
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
        }
        CardText(title, line, dot = if (newDot) ObiSilk.Water else null)
    }
}

/** The title under a card, two lines then an ellipsis, and its second line in haze. */
@Composable
fun CardText(title: String, line: String?, dot: Color? = null, titleColor: Color = ObiSilk.Silk) {
    Spacer(Modifier.height(8.dp))
    Text(title, style = TvTitle.card, color = titleColor, maxLines = 2, overflow = TextOverflow.Ellipsis)
    if (line != null) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
            if (dot != null) {
                Box(Modifier.size(6.dp).background(dot, CircleShape))
                Spacer(Modifier.width(6.dp))
            }
            Text(line, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** The 4 dp amber progress bar along a card's bottom edge, on the rule colour. */
@Composable
fun ProgressEdge(fraction: Float, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(4.dp).background(ObiSilk.Rule)) {
        Box(Modifier.fillMaxWidth(fraction.coerceIn(0f, 1f)).height(4.dp).background(ObiSilk.Amber))
    }
}

/** A static placeholder in a card's place while a list loads: a flat rect, never a shimmer. */
@Composable
fun CardPlaceholder(width: Dp, height: Dp) {
    Column(Modifier.width(width)) {
        Box(Modifier.size(width, height).background(ObiSilk.Raised, RoundedCornerShape(8.dp)))
        Spacer(Modifier.height(10.dp))
        Box(Modifier.size(width * 0.7f, 12.dp).background(ObiSilk.Silk.copy(alpha = 0.12f), RoundedCornerShape(4.dp)))
    }
}

/**
 * Scroll [list] so the item [key] tops out at [top] px as focus comes into this part of the page:
 * a row's header at y 48, the row under it.
 */
fun Modifier.settlesAt(list: LazyListState, scope: CoroutineScope, key: String, top: Int): Modifier =
    onFocusChanged { focus ->
        if (!focus.hasFocus) return@onFocusChanged
        val item = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == key } ?: return@onFocusChanged
        scope.launch { list.animateScrollBy((item.offset - top).toFloat(), Motion.quick()) }
    }
