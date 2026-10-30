// A title's trailer on the phone's title page (AndroidTitle, trailers board): the banner is taller
// when a trailer may play, the video sits below the app bar's buttons, the art fades away over it
// once it plays and comes back before it ends, and the page never moves.

package tv.anistream.ui

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.currentStateAsState
import kotlinx.coroutines.delay
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.EmbedStyle
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.TrailerControl
import tv.anistream.i18n.R
import tv.anistream.store.TrailerPlayer
import tv.anistream.store.TrailerVisit

/**
 * The canvas's fade curves, as PathInterpolators: the banner leaves over the video on fade-in,
 * (0, 0, 0.58, 1) for 200 ms, and returns on fade-out, (0.42, 0, 1, 1) for 300 ms.
 */
internal val TrailerFadeIn = CubicBezierEasing(0f, 0f, 0.58f, 1f)
internal val TrailerFadeOut = CubicBezierEasing(0.42f, 0f, 1f, 1f)

/** The page's measurements with room for a trailer; the video is 16:9 across the full width. */
internal object TrailerRoom {
    /** From the status bar's foot to the video: below the app bar's buttons. */
    val videoTop = 60.dp

    /** The banner's height below the status bar: the video and its foot. */
    val band = 300.dp

    /** Where the poster and title start: below the band, so nothing sits over the video. */
    val headTop = 312.dp

    val sound = 48.dp
    val gutter = 16.dp

    /** The title leaves room for the sound button at the head's trailing edge. */
    val titleGap = sound + gutter

    /** How long the page rests before the trailer loads: the page's own transition and half a second. */
    const val SETTLE_MILLIS = 900L

    /** Scrolling past this pauses the trailer under the returning art. */
    val scrollAway = 8.dp
}

/** Where a page's room for a trailer is decided, once. */
private class RoomLatch {
    var value: Boolean? = null
}

/**
 * Whether this title's page makes room for a trailer, decided once when the page opens so that
 * nothing moves afterwards: the title has one that the engine lets start by itself, now.
 */
@Composable
internal fun rememberTrailerRoom(entry: Entry): Boolean {
    val store = LocalAppStore.current
    val latch = remember(entry.id) { RoomLatch() }
    if (latch.value == null) {
        if (entry.trailer != null) {
            latch.value = store.trailerMayAutoplay(entry)
        } else if (entry.id in store.titles.detailed) {
            latch.value = false
        }
    }
    return latch.value == true
}

/** What is hanging on the visit's web view, for the visit to tell. */
private class PlayerRef {
    var player: TrailerPlayer? = null
}

/**
 * The head with room for a trailer: the banner art at the full band, the video under it at the
 * band's top, and the poster, title and sound button below.
 */
@Composable
internal fun TrailerHead(
    entry: Entry,
    art: @Composable (Modifier) -> Unit,
    list: LazyListState,
    inset: Dp,
    active: Boolean,
    header: @Composable (Modifier) -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val trailer = entry.trailer
    val scope = rememberCoroutineScope()
    val ref = remember(entry.id) { PlayerRef() }
    val visit = remember(entry.id) { TrailerVisit(scope, send = { ref.player?.control(it) }) }
    val live by visit.live.collectAsStateWithLifecycle()
    val shown by visit.shown.collectAsStateWithLifecycle()

    // Only the page on screen plays, and only while the app is in front with nothing over the page.
    val resumed by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    val focused = LocalWindowInfo.current.isWindowFocused
    val front = active && trailer != null && resumed.isAtLeast(Lifecycle.State.RESUMED) && focused
    LaunchedEffect(front) {
        if (front) {
            delay(TrailerRoom.SETTLE_MILLIS)
            if (store.trailerMayAutoplay(entry)) visit.begin()
        } else {
            // A sheet, the background or another page: the view goes, and the trailer does not come back.
            visit.leave()
        }
    }
    DisposableEffect(visit) { onDispose { visit.release() } }

    val away = with(LocalDensity.current) {
        val limit = TrailerRoom.scrollAway.toPx()
        remember(list) { derivedStateOf { list.firstVisibleItemIndex > 0 || list.firstVisibleItemScrollOffset > limit } }
    }
    LaunchedEffect(away) { snapshotFlow { away.value }.collect { visit.scrolledAway(it) } }

    val player = remember(live, trailer) {
        if (live && trailer != null) TrailerPlayer(context, trailer, EmbedStyle.BANNER) { visit.signal(it) } else null
    }
    ref.player = player
    DisposableEffect(player) {
        onDispose {
            player?.release()
            if (ref.player === player) ref.player = null
        }
    }

    val artAlpha by animateFloatAsState(
        targetValue = if (shown) 0f else 1f,
        animationSpec = if (shown) tween(200, easing = TrailerFadeIn) else tween(300, easing = TrailerFadeOut),
        label = "trailer art", // i18n-skip
    )
    Box {
        if (player != null) {
            // The video's own rectangle, 16:9 across the page, and nothing in front of it once it plays.
            AndroidView(
                factory = { player.view },
                modifier = Modifier
                    .padding(top = inset + TrailerRoom.videoTop)
                    .fillMaxWidth()
                    .aspectRatio(16f / 9f),
            )
        }
        art(
            Modifier
                .fillMaxWidth()
                .height(inset + TrailerRoom.band)
                .graphicsLayer {
                    alpha = artAlpha
                    translationY = (if (list.firstVisibleItemIndex == 0) list.firstVisibleItemScrollOffset else 0) * 0.5f
                },
        )
        header(Modifier.padding(top = inset + TrailerRoom.headTop))
        SoundButton(
            visible = shown,
            player = player,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = inset + TrailerRoom.headTop, end = TrailerRoom.gutter),
        )
    }
}

/**
 * The sound button at the head's trailing edge, 48 dp, plain: on while the video is in front, off
 * with it. Sound is off at every start and the choice lasts for this trailer only.
 */
@Composable
private fun SoundButton(visible: Boolean, player: TrailerPlayer?, modifier: Modifier = Modifier) {
    var muted by remember(player) { mutableStateOf(true) }
    val alpha by animateFloatAsState(
        targetValue = if (visible) 1f else 0f,
        animationSpec = if (visible) tween(200, easing = TrailerFadeIn) else tween(300, easing = TrailerFadeOut),
        label = "trailer sound", // i18n-skip
    )
    if (player == null || alpha == 0f && !visible) return
    val label = stringResource(R.string.title_trailer_sound)
    val state = stringResource(if (muted) R.string.value_off else R.string.value_on)
    IconButton(
        onClick = {
            muted = !muted
            player.control(if (muted) TrailerControl.MUTE else TrailerControl.UNMUTE)
        },
        enabled = visible,
        modifier = modifier
            .size(TrailerRoom.sound)
            .alpha(alpha)
            .semantics {
                contentDescription = label
                stateDescription = state
            },
    ) {
        Box(
            Modifier
                .size(TrailerRoom.sound)
                .background(MaterialTheme.colorScheme.surfaceContainer, CircleShape)
                .border(1.dp, ObiSilk.Silk.copy(alpha = 0.12f), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(if (muted) Icons.SoundOff else Icons.SoundOn, contentDescription = null, modifier = Modifier.size(20.dp))
        }
    }
}
