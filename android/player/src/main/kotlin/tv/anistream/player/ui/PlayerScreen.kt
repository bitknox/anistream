// The player on a phone, as AndroidPlayer draws it: full screen and landscape, the video in its
// SurfaceView, the starting screen until the first frame (T2), the chrome over a 35 % dim (T4), and
// the close by dragging down, the back gesture or the back arrow (T3).

package tv.anistream.player.ui

import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.accessibility.AccessibilityManager
import android.accessibilityservice.AccessibilityServiceInfo
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.PredictiveBackHandler
import androidx.activity.BackEventCompat
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.player.Device
import tv.anistream.player.PlayerModel
import tv.anistream.player.SeekPreview
import tv.anistream.store.HeroArt
import tv.anistream.ui.ArtworkImage
import tv.anistream.ui.LocalAppStore
import tv.anistream.ui.LocalPageAnimation
import tv.anistream.ui.pixels

/** Where the video sits: the largest rect of its aspect in the screen, centred. */
internal data class VideoRect(val x: Dp, val y: Dp, val width: Dp, val height: Dp)

internal fun videoRect(width: Dp, height: Dp, aspect: Double): VideoRect {
    val fitted = if (width / height > aspect.toFloat()) {
        val w = height * aspect.toFloat()
        VideoRect((width - w) / 2, 0.dp, w, height)
    } else {
        val h = width / aspect.toFloat()
        VideoRect(0.dp, (height - h) / 2, width, h)
    }
    return fitted
}

/** What the player draws the picture with: mpv's SurfaceView, or a still in a screenshot. */
typealias VideoContent = @Composable (model: PlayerModel, modifier: Modifier, view: (SurfaceView?) -> Unit) -> Unit

/** The player's screen. [video] draws the picture: mpv's surface, but for screenshots. */
@Composable
fun PlayerScreen(model: PlayerModel, video: VideoContent = { m, modifier, view -> VideoSurface(m, modifier, view) }) {
    val activity = LocalActivity.current
    val phone = LocalConfiguration.current.smallestScreenWidthDp < 600
    val haptics = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var surfaceView by remember { mutableStateOf<SurfaceView?>(null) }
    var sheet by remember { mutableStateOf<PlayerSheet?>(null) }

    // TalkBack and Switch Access keep the controls up, and Up Next waits for them (T4, T5).
    val accessibility = remember { context.getSystemService(AccessibilityManager::class.java) }
    DisposableEffect(accessibility) {
        fun assisted() {
            model.assisted = accessibility?.isTouchExplorationEnabled == true ||
                accessibility?.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
                    ?.any { it.id.contains("switchaccess", ignoreCase = true) } == true
        }
        assisted()
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { assisted() }
        accessibility?.addTouchExplorationStateChangeListener(listener)
        onDispose { accessibility?.removeTouchExplorationStateChangeListener(listener) }
    }

    // The frame under the scrubbing finger, from a second mpv, closed with the file.
    val preview = remember { SeekPreview(context, scope) }
    val previewSource = model.previewSource
    DisposableEffect(previewSource) { onDispose { preview.close() } }

    // At Up Next's zero the picture fades into the ground, and out of it on the next first frame,
    // never through black (T5).
    var through by remember { mutableStateOf(false) }
    LaunchedEffect(model.upNext?.played) { if (model.upNext?.played == true) through = true }
    LaunchedEffect(model.hasPicture, model.playback) { if (model.hasPicture) through = false }
    val ground by animateFloatAsState(if (through) 1f else 0f, if (through) ThroughGround else Motion.fadeIn(), label = "ground")

    Immersive()
    KeepAwake(playing = model.hasPicture && !model.paused)
    // Landscape on a phone once the shared bounds have settled, so the turn never cuts into the grow,
    // and portrait again whenever the player stops being up, however it went: the viewer's close,
    // the end of the file, the notification's Stop, the engine stopping it, the app swiped away.
    val page = LocalPageAnimation.current
    val settled = page == null || !page.transition.isRunning
    val presented = model.isPresented
    LaunchedEffect(settled, phone, presented) {
        if (!phone) return@LaunchedEffect
        if (!presented) {
            activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        } else if (settled) {
            activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
    }
    DisposableEffect(activity, phone) {
        onDispose { if (phone) activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT }
    }

    // The still that moves while the player is dragged, or closes, in place of the SurfaceView (T3).
    val still = remember { StillFrame() }
    val drag = remember { Animatable(0f) }
    val back = remember { Animatable(0f) }
    var backEdge by remember { mutableStateOf(BackEventCompat.EDGE_LEFT) }
    var closing by remember { mutableStateOf(false) }

    fun close() {
        if (closing) return
        closing = true
        if (phone) activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        model.viewerClosed()
    }

    PredictiveBackHandler { events ->
        still.freeze(surfaceView)
        try {
            events.collect { event ->
                backEdge = event.swipeEdge
                back.snapTo(event.progress)
            }
            close()
        } catch (e: CancellationException) {
            scope.launch {
                back.animateTo(0f, Motion.standard())
                if (drag.value == 0f) still.unfreeze()
            }
            throw e
        }
    }

    BoxWithConstraints(Modifier.fillMaxSize().background(Color.Black)) {
        val rect = videoRect(maxWidth, maxHeight, model.aspect ?: (16.0 / 9.0))
        val density = LocalDensity.current
        val pip = model.inPip
        val chromeVisible = model.controlsVisible && model.hasPicture && !closing && still.frame == null && !pip
        var videoBounds by remember { mutableStateOf<android.graphics.Rect?>(null) }
        PictureInPicture(model, videoBounds)
        // In the window the video shows alone: what was open closes, at once.
        LaunchedEffect(pip) { if (pip) sheet = null }

        // The picture, where mpv draws it.
        Box(
            Modifier.offset(rect.x, rect.y).size(rect.width, rect.height).onGloballyPositioned { coordinates ->
                val bounds = coordinates.boundsInWindow()
                videoBounds = android.graphics.Rect(bounds.left.toInt(), bounds.top.toInt(), bounds.right.toInt(), bounds.bottom.toInt())
            },
        ) {
            video(model, Modifier.fillMaxSize()) { surfaceView = it }
        }

        // The ground the picture fades through between episodes.
        if (ground > 0f) Box(Modifier.fillMaxSize().graphicsLayer { alpha = ground }.background(ObiSilk.Ground))

        // Taps on the video: the middle third shows and hides the controls at once; the sides
        // wait for a second tap, which seeks 10 s.
        val dragState = rememberDraggableState { delta ->
            val before = drag.value
            val after = (before + with(density) { delta.toDp().value }).coerceAtLeast(0f)
            scope.launch { drag.snapTo(after) }
            if (before < DISMISS_DP && after >= DISMISS_DP) haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
        }
        if (!pip) Row(
            Modifier
                .fillMaxSize()
                .draggable(
                    dragState,
                    Orientation.Vertical,
                    onDragStarted = { still.freeze(surfaceView) },
                    onDragStopped = { velocity ->
                        val fling = with(density) { velocity.toDp().value }
                        if (drag.value > DISMISS_DP || fling > FLING_DP) {
                            close()
                        } else {
                            drag.animateTo(0f, Motion.standard())
                            still.unfreeze()
                        }
                    },
                ),
        ) {
            for (third in Third.entries) {
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .pointerInput(third, chromeVisible) {
                            val toggle = { if (chromeVisible) model.hideControls() else model.poke() }
                            if (third == Third.Middle) {
                                detectTapGestures(onTap = { toggle() })
                            } else {
                                detectTapGestures(
                                    onTap = { toggle() },
                                    onDoubleTap = { model.seekBy(if (third == Third.Start) -10.0 else 10.0) },
                                )
                            }
                        },
                )
            }
        }

        // Over the taps, so its own buttons take theirs: Choose source, and Close on a play that
        // never played. Nothing else in it takes a touch, so a drag down still closes the player.
        StartingLayer(model, rect, art = !through) { sheet = PlayerSheet.Sources.also { model.chooseSource() } }

        // The subtitles rise above the controls while they are up, and fall back with them (T4).
        val lift by animateFloatAsState(if (chromeVisible) SUBTITLES_UP else SUBTITLES_DOWN, if (pip) snap() else Motion.standard())
        LaunchedEffect(rect.height) {
            snapshotFlow { lift }.collectLatest { above ->
                val percent = 100.0 - (above - SUBTITLES_DOWN) / rect.height.value * 100.0
                model.liftSubtitles(percent)
                kotlinx.coroutines.delay(40)
            }
        }

        AnimatedVisibility(chromeVisible, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
            PlayerChrome(
                model,
                onClose = { still.freeze(surfaceView) { close() } },
                onTracks = { sheet = PlayerSheet.Tracks },
                onEpisodes = { sheet = PlayerSheet.Episodes },
                preview = preview.takeIf { previewSource != null && !Device.power(context).lowPower },
            )
        }

        if (still.frame == null && !pip) {
            CornerCard(model, chromeVisible, ground, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 88.dp))
            ResumeNote(model, Modifier.align(Alignment.BottomStart).padding(start = 48.dp, bottom = 88.dp))
            AlertNote(model, Modifier.align(Alignment.TopCenter).padding(top = 80.dp))
            StillWatching(model)
        }
        PlayerSheets(model, sheet) { sheet = null }

        // A drag or a back gesture moves a still of the frame over the ground, the bars with it.
        still.frame?.let { frame ->
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground))
            val k = (drag.value / DISMISS_DP).coerceIn(0f, 1f)
            val p = back.value
            val edge = if (backEdge == BackEventCompat.EDGE_LEFT) 1f else -1f
            Box(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        val scale = (1f - 0.1f * k) * (1f - 0.1f * p)
                        scaleX = scale
                        scaleY = scale
                        translationY = (drag.value - 12.6f * p).dp.toPx()
                        translationX = (37.75f * p * edge).dp.toPx()
                        shape = RoundedCornerShape((16f * k + 28f * p).dp)
                        clip = true
                    }
                    .background(Color.Black),
            ) {
                Image(
                    frame.asImageBitmap(),
                    contentDescription = null,
                    contentScale = ContentScale.FillBounds,
                    modifier = Modifier.offset(rect.x, rect.y).size(rect.width, rect.height),
                )
            }
        }
    }
}

private enum class Third { Start, Middle, End }

/** The drag down that closes the player, and the fling that does, in dp and dp/s (T3). */
private const val DISMISS_DP = 120f
private const val FLING_DP = 800f

/** Where the subtitles stand above the bottom edge, in dp: with the controls hidden, and up (T4). */
private const val SUBTITLES_DOWN = 32f
private const val SUBTITLES_UP = 88f

/** mpv's SurfaceView: a new one when the model asks for one, after an mpv that stopped answering. */
@Composable
fun VideoSurface(model: PlayerModel, modifier: Modifier, view: (SurfaceView?) -> Unit) {
    val density = LocalDensity.current.density
    key(model.surfaceGeneration) {
        AndroidView(
            factory = { context ->
                SurfaceView(context).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = model.surfaceCreated(holder.surface)

                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                            model.surfaceChanged(width, height)
                            display?.let { model.surfaceDisplay(Device.display(it, density)) }
                        }

                        override fun surfaceDestroyed(holder: SurfaceHolder) = model.surfaceDestroyed()
                    })
                    view(this)
                }
            },
            onRelease = { view(null) },
            modifier = modifier,
        )
    }
}

/**
 * The starting screen, in the player at full size: the ground, the title's wide art cropped to
 * the video rect under a 35 % ground dim, and the 32 dp indicator with "Starting episode 7" below
 * it, until the first frame lifts it on the fade-in curve (T2).
 */
@Composable
private fun StartingLayer(model: PlayerModel, rect: VideoRect, art: Boolean, onChooseSource: () -> Unit) {
    val up = model.isPresented && (model.starting != null || !model.hasPicture || model.failure != null)
    val shown by animateFloatAsState(if (up) 1f else 0f, Motion.fadeIn(), label = "starting")
    if (shown == 0f) return
    val store = LocalAppStore.current
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val id = model.titleId
    // The hero-art rule, as the title page has it: the banner, else a still already seen.
    val wide = if (!art) null else id?.let { entries[it] }?.let { HeroArt.of(it, tables[id]).url } ?: model.starting?.bannerUrl
    Box(Modifier.fillMaxSize().graphicsLayer { alpha = shown }.background(ObiSilk.Ground)) {
        Box(Modifier.offset(rect.x, rect.y).size(rect.width, rect.height)) {
            if (wide != null) ArtworkImage(wide, plate = null, maxPixelSize = pixels(rect.width), modifier = Modifier.fillMaxSize())
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = 0.35f)))
            val line = MaterialTheme.typography.titleMedium.copy(
                fontSize = 16.sp,
                lineHeight = 24.sp,
                fontWeight = FontWeight.Medium,
                shadow = Shadow(Color.Black.copy(alpha = 0.7f), Offset(0f, 1f), 1f),
            )
            Column(Modifier.align(Alignment.Center), horizontalAlignment = Alignment.CenterHorizontally) {
                val failure = model.failure
                if (failure != null) {
                    // It never played: said where the viewer is looking, with another source to choose.
                    Text("Couldn't play episode ${failure.episode}", style = line, color = ObiSilk.Silk)
                    Text(
                        "No source had a stream that played. Another may.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = ObiSilk.Haze,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                } else {
                    CircularProgressIndicator(
                        modifier = Modifier.size(32.dp),
                        color = ObiSilk.Silk,
                        strokeWidth = 3.dp,
                        trackColor = Color.Transparent,
                    )
                    Spacer(Modifier.height(16.dp))
                    Text(model.startingLine() ?: model.startingEpisode?.let { "Starting episode $it" }.orEmpty(), style = line, color = ObiSilk.Silk)
                }
                StartingChoices(model, onChooseSource)
            }
        }
    }
}

/** The system bars hidden while the player is up, and back when it goes; a swipe shows them a moment. */
@Composable
private fun Immersive() {
    val activity = LocalActivity.current ?: return
    val view = LocalView.current
    DisposableEffect(activity) {
        val controller = WindowCompat.getInsetsController(activity.window, view)
        controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())
        onDispose { controller.show(WindowInsetsCompat.Type.systemBars()) }
    }
}

/** The screen kept on while a picture plays. */
@Composable
private fun KeepAwake(playing: Boolean) {
    val view = LocalView.current
    DisposableEffect(playing) {
        view.keepScreenOn = playing
        onDispose { view.keepScreenOn = false }
    }
}

/**
 * A copy of the frame on screen, taken with PixelCopy when a drag or a back gesture starts: what
 * moves and what the return carries into the banner, since a SurfaceView takes no scale or clip.
 */
private class StillFrame {
    var frame by mutableStateOf<Bitmap?>(null)
        private set
    private var asked = false

    /** Copy [view]'s frame, then [then]; with no surface to copy, the ground stands in. */
    fun freeze(view: SurfaceView?, then: () -> Unit = {}) {
        if (frame != null || asked) {
            then()
            return
        }
        val surface = view?.holder?.surface
        if (view == null || surface == null || !surface.isValid || view.width == 0 || view.height == 0) {
            frame = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888).apply { eraseColor(android.graphics.Color.BLACK) }
            then()
            return
        }
        asked = true
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        PixelCopy.request(view, bitmap, { result ->
            asked = false
            frame = if (result == PixelCopy.SUCCESS) bitmap else Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888).apply {
                eraseColor(android.graphics.Color.BLACK)
            }
            then()
        }, Handler(Looper.getMainLooper()))
    }

    fun unfreeze() {
        frame = null
    }
}
