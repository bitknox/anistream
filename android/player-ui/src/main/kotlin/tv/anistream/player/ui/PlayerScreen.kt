// The player on a phone, as AndroidPlayer draws it: full screen and landscape, the video in its
// SurfaceView, the starting screen until the first frame (T2), the chrome over a 35 % dim (T4), and
// the close by dragging down, the back gesture or the back arrow (T3).

package tv.anistream.player.ui

import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.layout.onLayoutRectChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import tv.anistream.design.tabular
import tv.anistream.design.ObiSilk
import tv.anistream.player.Device
import tv.anistream.player.PlayerModel
import tv.anistream.player.SeekPreview
import tv.anistream.store.HeroArt
import tv.anistream.ui.ArtworkImage
import tv.anistream.ui.LocalAppStore
import tv.anistream.store.DiscussionKey
import tv.anistream.store.PlayerDiscussion
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
fun PlayerScreen(
    model: PlayerModel,
    video: VideoContent = { m, modifier, view -> VideoSurface(m, modifier, view) },
    /** Play on TV, where the app can: the TV picks the episode up from here. */
    playOnTv: (() -> Unit)? = null,
) {
    val activity = LocalActivity.current
    val phone = LocalConfiguration.current.smallestScreenWidthDp < 600
    val haptics = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var surfaceView by remember { mutableStateOf<SurfaceView?>(null) }
    var sheet by remember { mutableStateOf<PlayerSheet?>(null) }
    val discussion = discussionEntry(model)

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
    // Read in the layers that draw it, so the fade redraws them and composes nothing.
    val ground = animateFloatAsState(if (through) 1f else 0f, if (through) ThroughGround else Motion.fadeIn(), label = "ground") // i18n-skip
    val groundUp by remember { derivedStateOf { ground.value > 0f } }

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
        // Where the video sits in the window, the hint the window grows from. Said once it has
        // settled: while the player grows (T2), closes (T3) or turns, it changes every frame, and
        // each change would set the window's parameters again across the binder.
        var videoBounds by remember { mutableStateOf<android.graphics.Rect?>(null) }
        PictureInPicture(model, videoBounds)
        // In the window the video shows alone: what was open closes, at once.
        LaunchedEffect(pip) { if (pip) sheet = null }

        // The picture, where mpv draws it.
        Box(
            Modifier.offset(rect.x, rect.y).size(rect.width, rect.height).onLayoutRectChanged(throttleMillis = 0, debounceMillis = BOUNDS_SETTLE_MS) { layout ->
                val bounds = layout.boundsInWindow
                videoBounds = android.graphics.Rect(bounds.left, bounds.top, bounds.right, bounds.bottom)
            },
        ) {
            video(model, Modifier.fillMaxSize()) { surfaceView = it }
        }

        // The ground the picture fades through between episodes.
        if (groundUp) Box(Modifier.fillMaxSize().graphicsLayer { alpha = ground.value }.background(ObiSilk.Ground))

        // Waiting for data mid-episode: the thin amber line along the video's top edge, from 1 s
        // in until playback goes on. The stall prompt comes at 8 s, as ever.
        val bufferingLabel = stringResource(R.string.player_buffering)
        AnimatedVisibility(
            model.showsBuffering && !pip,
            enter = fadeIn(Motion.fadeIn()),
            exit = fadeOut(Motion.fadeOut()),
            modifier = Modifier.offset(rect.x, rect.y).width(rect.width),
        ) {
            BufferingLine(Modifier.fillMaxWidth().height(BUFFERING_LINE).semantics { contentDescription = bufferingLabel })
        }

        // Taps on the video: the middle third shows and hides the controls at once; the sides
        // wait for a second tap, which seeks 10 s, and each tap after it within 0.6 s 10 s more.
        // A press held anywhere plays at 2× until it lifts.
        var run by remember { mutableStateOf<SeekRun?>(null) }
        var runShown by remember { mutableStateOf(false) }
        var pressedAt by remember { mutableStateOf(0L) }
        var holding by remember { mutableStateOf<Double?>(null) }
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
                            // Each press: held past the long-press time, 2× until it lifts.
                            val press: suspend androidx.compose.foundation.gestures.PressGestureScope.(androidx.compose.ui.geometry.Offset) -> Unit = {
                                pressedAt = System.nanoTime() / 1_000_000
                                tryAwaitRelease()
                                holding?.let { before ->
                                    holding = null
                                    model.playAt(before)
                                }
                            }
                            val hold: (androidx.compose.ui.geometry.Offset) -> Unit = {
                                if (holding == null && model.speed < HOLD_SPEED) {
                                    holding = model.speed
                                    model.playAt(HOLD_SPEED)
                                    haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
                                }
                            }
                            if (third == Third.Middle) {
                                detectTapGestures(onPress = press, onLongPress = hold, onTap = { toggle() })
                            } else {
                                val step = if (third == Third.Start) -SEEK_STEP else SEEK_STEP
                                // A tap within 0.6 s of the last seek on this side seeks another 10 s.
                                val seek = {
                                    val going = run?.takeIf { it.third == third && pressedAt - it.at <= SEEK_RUN_MS }
                                    model.seekBy(step)
                                    run = SeekRun(third, (going?.taps ?: 0) + 1, pressedAt)
                                    runShown = true
                                }
                                detectTapGestures(
                                    onPress = press,
                                    onLongPress = hold,
                                    onTap = {
                                        val going = run?.let { it.third == third && pressedAt - it.at <= SEEK_RUN_MS } == true
                                        if (going) seek() else toggle()
                                    },
                                    onDoubleTap = { seek() },
                                )
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    // "+30 s": counts up with each tap of a run, gone 0.6 s after the last.
                    val shown = run?.takeIf { it.third == third }
                    FadingLabel(runShown && shown != null, (if (third == Third.Start) "\u2212" else "+") + Strings.durationSeconds(LocalContext.current, (shown?.taps ?: 1) * SEEK_STEP.toInt())) // i18n-skip
                }
            }
        }
        LaunchedEffect(run) {
            if (run != null) {
                kotlinx.coroutines.delay(SEEK_RUN_MS)
                runShown = false
            }
        }
        // "2×" while a press holds the speed up.
        AnimatedVisibility(
            holding != null,
            enter = fadeIn(Motion.fadeIn()),
            exit = fadeOut(Motion.fadeOut()),
            modifier = Modifier.align(Alignment.TopCenter).padding(top = 80.dp),
        ) {
            GestureLabel("2\u00d7")
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
                onPlayOnTv = playOnTv,
                discussion = discussion,
            )
        }

        if (still.frame == null && !pip) {
            CornerCard(model, chromeVisible, { ground.value }, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 88.dp))
            ResumeNote(model, Modifier.align(Alignment.BottomStart).padding(start = 48.dp, bottom = 88.dp))
            AlertNote(model, Modifier.align(Alignment.TopCenter).padding(top = 80.dp))
            StillWatching(model)
        }
        PlayerSheets(model, sheet) { sheet = null }

        // A drag or a back gesture moves a still of the frame over the ground, the bars with it.
        // The gesture is read in the layer alone, so following the finger redraws the still and
        // composes nothing.
        still.frame?.let { frame ->
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground))
            val image = remember(frame) { frame.asImageBitmap() }
            Box(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        val k = (drag.value / DISMISS_DP).coerceIn(0f, 1f)
                        val p = back.value
                        val edge = if (backEdge == BackEventCompat.EDGE_LEFT) 1f else -1f
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
                    image,
                    contentDescription = null,
                    contentScale = ContentScale.FillBounds,
                    modifier = Modifier.offset(rect.x, rect.y).size(rect.width, rect.height),
                )
            }
        }
    }
}

private enum class Third { Start, Middle, End }

/** How long the video's bounds hold still before the window's source-rect hint follows them. */
private const val BOUNDS_SETTLE_MS = 100L

/** The drag down that closes the player, and the fling that does, in dp and dp/s (T3). */
private const val DISMISS_DP = 120f
private const val FLING_DP = 800f

/** The buffering line's weight. */
private val BUFFERING_LINE = 2.dp

/**
 * The thin amber indeterminate line. In a preview or a screenshot, where infinite animations do not
 * run, a still of it: one stretch of the line partway across.
 */
@Composable
private fun BufferingLine(modifier: Modifier) {
    if (LocalInspectionMode.current) {
        Box(modifier) {
            Box(
                Modifier
                    .fillMaxHeight()
                    .fillMaxWidth(0.62f)
                    .padding(start = 120.dp)
                    .background(ObiSilk.Amber, RoundedCornerShape(1.dp)),
            )
        }
        return
    }
    LinearProgressIndicator(modifier = modifier, color = ObiSilk.Amber, trackColor = Color.Transparent, gapSize = 0.dp)
}

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
    val shown by animateFloatAsState(if (up) 1f else 0f, Motion.fadeIn(), label = "starting") // i18n-skip
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
                    Text(Strings.playerFailedTitleText(LocalContext.current, failure.episode), style = line, color = ObiSilk.Silk)
                    Text(
                        // Why, as the engine said it, when it did.
                        failure.reason?.let { "$it." } ?: stringResource(R.string.player_failed_detail), // i18n-skip
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
                    Text(model.startingLine(LocalContext.current) ?: model.startingEpisode?.let { Strings.playerStartingText(LocalContext.current, it) }.orEmpty(), style = line, color = ObiSilk.Silk)
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

/** A run of seeking taps on one side: how many, and when the last was pressed. */
private data class SeekRun(val third: Third, val taps: Int, val at: Long)

private const val SEEK_STEP = 10.0
private const val SEEK_RUN_MS = 600L
private const val HOLD_SPEED = 2.0

/** A gesture's word over the video: "+30 s", "2×", in silk on the ground at 80 %. */
@Composable
private fun GestureLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium.tabular(),
        color = ObiSilk.Silk,
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(ObiSilk.Ground.copy(alpha = 0.8f))
            .padding(horizontal = 12.dp, vertical = 6.dp),
    )
}

/** [text] as a gesture's label, fading in and out with [visible]. */
@Composable
private fun FadingLabel(visible: Boolean, text: String) {
    AnimatedVisibility(visible, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        GestureLabel(text)
    }
}

/**
 * The discussion of the episode playing, for the More menu: there from 85 % of the episode, while
 * discussions are on. Opening it pauses the video where it is, and it stays paused as the sheet closes.
 */
@Composable
private fun discussionEntry(model: PlayerModel): DiscussionEntry? {
    val store = LocalAppStore.current
    val number = model.episode?.number ?: model.staged?.episode
    val episode = PlayerDiscussion.episode(number)
    val media = model.titleId
    val late = remember(media, number) { PlayerDiscussion() }.see(model.position, model.duration)
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val found by store.discussion.found.collectAsStateWithLifecycle()
    val enabled = settings == null || store.discussionsOn()
    if (media == null || episode == null || !PlayerDiscussion.offered(late, enabled, episode)) return null
    return DiscussionEntry(found[DiscussionKey(media, episode)]) {
        model.setPlaying(false)
        store.discussion.show(media, episode)
    }
}
