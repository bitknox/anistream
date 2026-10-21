// The player's controls over a 35 % dim, as AndroidPlayer draws them (T4): the top bar with the
// back arrow, the show and the episode, subtitles and audio, and episodes; the bottom bar with
// back 10 s, play or pause, forward 10 s, the times and the seek slider.

package tv.anistream.player.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.Canvas
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import tv.anistream.player.SeekSegment

/** The 1 dp shadow the board puts under the controls' text and icons, for a bright frame. */
private val ControlShadow = Shadow(Color(0xB30A0C18), Offset(0f, 2.6f), 2.6f)

@Composable
fun PlayerChrome(
    model: PlayerModel,
    onClose: () -> Unit,
    onTracks: () -> Unit = {},
    onEpisodes: () -> Unit = {},
) {
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = 0.35f)))
        TopBar(
            model, onClose, onTracks, onEpisodes,
            Modifier.align(Alignment.TopStart).padding(start = 48.dp, end = 48.dp, top = 16.dp).fillMaxWidth().height(48.dp),
        )
        BottomBar(
            model,
            Modifier.align(Alignment.BottomStart).padding(start = 48.dp, end = 48.dp, bottom = 28.dp).fillMaxWidth().height(48.dp),
        )
    }
}

@Composable
private fun TopBar(model: PlayerModel, onClose: () -> Unit, onTracks: () -> Unit, onEpisodes: () -> Unit, modifier: Modifier) {
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
        ControlButton(PlayerIcons.Subtitles, "Subtitles and audio", onClick = onTracks)
        ControlButton(PlayerIcons.Episodes, "Episodes", onClick = onEpisodes)
    }
}

@Composable
private fun BottomBar(model: PlayerModel, modifier: Modifier) {
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
        SeekSlider(model, Modifier.weight(1f).padding(horizontal = 12.dp))
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
private fun SeekSlider(model: PlayerModel, modifier: Modifier) {
    val haptics = LocalHapticFeedback.current
    val touch = remember { MutableInteractionSource() }
    val pressed by touch.collectIsPressedAsState()
    val dragged by touch.collectIsDraggedAsState()
    val active = pressed || dragged
    val trackHeight by animateDpAsState(if (active) 24.dp else 16.dp, Motion.quick(), label = "track")
    val handleWidth by animateDpAsState(if (active) 2.dp else 4.dp, label = "handle")
    val duration = model.duration?.takeIf { it > 0 } ?: 1.0
    val segments = model.segments
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
    Slider(
        state = state,
        onValueChange = { value ->
            state.value = value
            model.scrub(value.toDouble())
        },
        onValueChangeFinished = { model.endScrub() },
        interactionSource = touch,
        modifier = modifier.height(48.dp).semantics {
            contentDescription = "Position"
            stateDescription = label
        },
        thumb = {
            Box(Modifier.size(handleWidth, 44.dp).background(ObiSilk.Amber, RoundedCornerShape(2.dp)))
        },
        track = { SeekTrack(it, segments, duration, trackHeight) },
    )
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
