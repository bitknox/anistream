// The player's controls over a 35 % dim, as AndroidPlayer draws them (T4): the top bar with the
// back arrow, the show and the episode, subtitles and audio, and episodes; the bottom bar with
// back 10 s, play or pause, forward 10 s, the times and the seek slider.

package tv.anistream.player.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import kotlin.math.roundToInt
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.player.PlayerClock
import tv.anistream.player.PlayerModel
import tv.anistream.player.SeekPreview
import tv.anistream.player.SeekSegment

/** The 1 dp shadow the board puts under the controls' text and icons, for a bright frame. */
private val ControlShadow = Shadow(Color(0xB30A0C18), Offset(0f, 2.6f), 2.6f)

@Composable
fun PlayerChrome(
    model: PlayerModel,
    onClose: () -> Unit,
    onTracks: () -> Unit = {},
    onEpisodes: () -> Unit = {},
    /** The frames under the scrubbing finger, when there are to be any. */
    preview: SeekPreview? = null,
    /** Play on TV, where the app can: the episode carries on on a TV from here. */
    onPlayOnTv: (() -> Unit)? = null,
) {
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = 0.35f)))
        TopBar(
            model, onClose, onTracks, onEpisodes, onPlayOnTv,
            Modifier.align(Alignment.TopStart).padding(start = 48.dp, end = 48.dp, top = 16.dp).fillMaxWidth().height(48.dp),
        )
        BottomBar(
            model,
            preview,
            Modifier.align(Alignment.BottomStart).padding(start = 48.dp, end = 48.dp, bottom = 28.dp).fillMaxWidth().height(48.dp),
        )
    }
}

@Composable
private fun TopBar(
    model: PlayerModel,
    onClose: () -> Unit,
    onTracks: () -> Unit,
    onEpisodes: () -> Unit,
    onPlayOnTv: (() -> Unit)?,
    modifier: Modifier,
) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        ControlButton(PlayerIcons.Back, "Back", onClick = onClose)
        Column(Modifier.weight(1f)) {
            Text(
                model.showTitle,
                style = ShowTitle.row.copy(shadow = ControlShadow),
                color = ObiSilk.Silk,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                model.episodeLine,
                style = MaterialTheme.typography.bodyMedium.copy(shadow = ControlShadow),
                color = ObiSilk.Silk,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        onPlayOnTv?.let { ControlButton(PlayerIcons.Tv, "Play on TV", onClick = it) }
        ControlButton(PlayerIcons.Subtitles, "Subtitles, audio and picture", onClick = onTracks)
        ControlButton(PlayerIcons.Episodes, "Episodes", onClick = onEpisodes)
    }
}

@Composable
private fun BottomBar(model: PlayerModel, preview: SeekPreview?, modifier: Modifier) {
    val time = MaterialTheme.typography.labelLarge.tabular().copy(shadow = ControlShadow)
    val duration = model.duration ?: 0.0
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        StepButton(PlayerIcons.Replay, "Back 10 seconds") { model.seekBy(-10.0) }
        Spacer(Modifier.width(4.dp))
        ControlButton(if (model.paused) PlayerIcons.Play else PlayerIcons.Pause, if (model.paused) "Play" else "Pause", size = 32.dp) {
            model.togglePause()
        }
        Spacer(Modifier.width(4.dp))
        StepButton(PlayerIcons.Forward, "Forward 10 seconds") { model.seekBy(10.0) }
        Spacer(Modifier.width(8.dp))
        Text(PlayerClock.label(model.position), style = time, color = ObiSilk.Silk, modifier = Modifier.width(44.dp))
        SeekSlider(model, preview, Modifier.weight(1f).padding(horizontal = 12.dp))
        Text(
            PlayerClock.remaining(maxOf(0.0, duration - model.position)),
            style = time,
            color = ObiSilk.Silk,
            textAlign = TextAlign.End,
            modifier = Modifier.width(52.dp),
        )
    }
}

@Composable
private fun ControlButton(icon: ImageVector, label: String, size: Dp = 24.dp, onClick: () -> Unit) {
    IconButton(onClick = onClick, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
        Icon(icon, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(size))
    }
}

/** Back or forward 10 s: the turning arrow at 28 dp with its 10 inside. */
@Composable
private fun StepButton(icon: ImageVector, label: String, onClick: () -> Unit) {
    IconButton(onClick = onClick, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
        Box(Modifier.size(28.dp)) {
            Icon(icon, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.fillMaxSize())
            Text(
                "10",
                style = TextStyle(fontSize = 8.5.sp, lineHeight = 10.sp, fontWeight = FontWeight.Bold),
                color = ObiSilk.Silk,
                modifier = Modifier.align(Alignment.Center).offset(y = 2.dp),
            )
        }
    }
}

/**
 * The Expressive slider with the board's track: 16 dp, 24 while touched, amber up to the handle,
 * #434653 after it with the opening and the ending in haze, 2 dp gaps between the stretches and
 * 6 dp either side of a 4 × 44 amber handle. While it moves the seek line follows the finger and
 * mpv seeks by keyframes; on release it seeks exactly.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SeekSlider(model: PlayerModel, preview: SeekPreview?, modifier: Modifier) {
    val haptics = LocalHapticFeedback.current
    val touch = remember { MutableInteractionSource() }
    val pressed by touch.collectIsPressedAsState()
    val dragged by touch.collectIsDraggedAsState()
    val active = pressed || dragged
    val trackHeight by animateDpAsState(if (active) 24.dp else 16.dp, Motion.quick(), label = "track")
    val handleWidth by animateDpAsState(if (active) 2.dp else 4.dp, label = "handle")
    val duration = model.duration?.takeIf { it > 0 } ?: 1.0
    // The stretches as a value that changes only with the chapters or the length, not with each
    // tick of the position, so the tick below is not started again and the track can skip.
    val segments by remember(model) { derivedStateOf { model.segments } }
    // A tick each time the stretch under the handle changes while it is dragged.
    LaunchedEffect(segments) {
        snapshotFlow { segments.firstOrNull { model.position >= it.start && model.position < it.end }?.kind }
            .distinctUntilChanged()
            .drop(1)
            .collect { if (model.scrubbing != null) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick) }
    }
    val label = buildString {
        append("${PlayerClock.label(model.position)} of ${PlayerClock.label(duration)}")
        model.chapter(model.position)?.let { append(", $it") }
    }
    val state = remember(duration) { SliderState(0f, 0, 0f..duration.toFloat()) }
    // The line follows mpv, but for while the finger is on it.
    SideEffect {
        if (!state.isDragging && model.scrubbing == null) state.value = model.position.toFloat().coerceIn(0f, duration.toFloat())
    }
    // The pair over the handle while it moves: the frame there, and its time and stretch (T4).
    val scrubbing = model.scrubbing
    var sliderLeft by remember { mutableFloatStateOf(0f) }
    var sliderWidth by remember { mutableIntStateOf(0) }
    val density = LocalDensity.current
    Box(modifier.onGloballyPositioned { sliderLeft = it.positionInWindow().x; sliderWidth = it.size.width }) {
    if (active && scrubbing != null) {
        val handle = (scrubbing / duration).toFloat().coerceIn(0f, 1f) * sliderWidth
        val window = with(density) { LocalConfiguration.current.screenWidthDp.dp.toPx() }
        val pairWidth = with(density) { 160.dp.toPx() }
        val min = with(density) { 48.dp.toPx() }
        val left = (sliderLeft + handle - pairWidth / 2).coerceIn(min, maxOf(min, window - min - pairWidth)) - sliderLeft
        PreviewPair(
            frame = preview?.frame,
            label = buildString {
                append(PlayerClock.label(scrubbing))
                model.chapter(scrubbing)?.takeIf { model.segments.any { s -> s.kind != SeekSegment.Kind.Plain && scrubbing >= s.start && scrubbing < s.end } }
                    ?.let { append(" · $it") }
            },
            // Measured free of the slider's 48 dp, and set with its foot 12 dp above the handle's top.
            modifier = Modifier.layout { measurable, _ ->
                val pair = measurable.measure(Constraints())
                layout(0, 0) { pair.place(left.roundToInt(), (2.dp - 12.dp).roundToPx() - pair.height) }
            },
        )
    }
    Slider(
        state = state,
        onValueChange = { value ->
            state.value = value
            model.scrub(value.toDouble())
            model.previewSource?.let { source -> preview?.show(value.toDouble(), source) }
        },
        onValueChangeFinished = { model.endScrub() },
        interactionSource = touch,
        modifier = Modifier.fillMaxWidth().height(48.dp).semantics {
            contentDescription = "Position"
            stateDescription = label
        },
        thumb = {
            Box(Modifier.size(handleWidth, 44.dp).background(ObiSilk.Amber, RoundedCornerShape(2.dp)))
        },
        track = { SeekTrack(it, segments, duration, trackHeight) },
    )
    }
}

/** The 160 × 90 frame at corners 12 with a silk outline at 24 %, and under it, 4 apart, the time pill. */
@Composable
private fun PreviewPair(frame: android.graphics.Bitmap?, label: String, modifier: Modifier) {
    Column(modifier.width(160.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            Modifier
                .size(160.dp, 90.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(ObiSilk.Ground)
                .border(1.dp, ObiSilk.Silk.copy(alpha = 0.24f), RoundedCornerShape(12.dp)),
        ) {
            if (frame != null) {
                val image = remember(frame) { frame.asImageBitmap() }
                Image(image, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            }
        }
        Spacer(Modifier.height(4.dp))
        Box(
            Modifier.height(24.dp).clip(RoundedCornerShape(12.dp)).background(ObiSilk.Raised).padding(horizontal = 10.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(label, style = MaterialTheme.typography.labelMedium.tabular(), color = ObiSilk.Silk, maxLines = 1)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SeekTrack(state: SliderState, segments: List<SeekSegment>, duration: Double, height: Dp) {
    Canvas(Modifier.fillMaxWidth().height(height)) {
        val fraction = state.coercedValueAsFraction
        val gap = 6.dp.toPx() + 2.dp.toPx()
        val seam = 2.dp.toPx()
        val outer = (height / 2).toPx()
        val inner = 2.dp.toPx()
        val handle = fraction * size.width
        val playedEnd = handle - gap
        if (playedEnd > 0) piece(0f, playedEnd, ObiSilk.Amber, outer, inner)
        var at = handle + gap
        if (at >= size.width) return@Canvas
        // After the handle: the plain stretches and the marked ones, each its own piece.
        val stretches = segments.ifEmpty { listOf(SeekSegment(SeekSegment.Kind.Plain, 0.0, duration)) }
        val pieces = stretches.mapNotNull { segment ->
            val start = maxOf(at, (segment.start / duration).toFloat() * size.width)
            val end = (segment.end / duration).toFloat() * size.width
            if (end - start < 1f) null else Triple(start, end, segment.kind)
        }
        pieces.forEachIndexed { i, (start, end, kind) ->
            val last = i == pieces.lastIndex
            val left = if (i == 0) start else start + seam / 2
            val right = if (last) size.width else end - seam / 2
            if (right - left < 1f) return@forEachIndexed
            val color = if (kind == SeekSegment.Kind.Plain) ObiSilk.Rule else ObiSilk.Haze
            piece(left, right, color, inner, if (last) outer else inner)
            at = end
        }
    }
}

/** One stretch of the track, its corners [left] and [right] round. */
private fun DrawScope.piece(start: Float, end: Float, color: Color, left: Float, right: Float) {
    val path = Path().apply {
        addRoundRect(
            RoundRect(
                start, 0f, end, size.height,
                topLeftCornerRadius = CornerRadius(left), bottomLeftCornerRadius = CornerRadius(left),
                topRightCornerRadius = CornerRadius(right), bottomRightCornerRadius = CornerRadius(right),
            ),
        )
    }
    drawPath(path, color)
}
