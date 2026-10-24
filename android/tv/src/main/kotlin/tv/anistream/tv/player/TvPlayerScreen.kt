// The TV player: the video full screen through :player, the starting screen until the first
// frame, and the chrome over a 35 % dim (title and clock at the top, the seek bar and the button
// row at the bottom), hidden after 5 s with no key. The D-pad follows the brief's player table.

package tv.anistream.tv.player

import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.delay
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.PlayerChoice
import tv.anistream.player.Device
import tv.anistream.player.PlayerModel
import tv.anistream.player.SeekSegment
import tv.anistream.player.Track
import tv.anistream.store.AppClock
import tv.anistream.tv.ui.ArtImage
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.TvTitle
import tv.anistream.tv.ui.entryFocus
import tv.anistream.tv.ui.tryFocus

/** The player's panels, each the 400-wide column on the right. */
enum class PlayerPanel { Episodes, Tracks, Sources, Info }

/** What the chrome puts focus on as it comes up. */
private enum class ChromeFocus { Play, SeekBar, Episodes }

private val HourMinute = DateTimeFormatter.ofPattern("HH:mm")

/** The chrome's dim over the video: the ground at 35 %. */
private const val DIM = 0.35f

@Composable
fun TvPlayerScreen(
    model: PlayerModel,
    modifier: Modifier = Modifier,
    video: @Composable (Modifier) -> Unit = { VideoSurface(model, it) },
) {
    val root = remember { FocusRequester() }
    val playFocus = remember { FocusRequester() }
    val barFocus = remember { FocusRequester() }
    val episodesFocus = remember { FocusRequester() }
    var panel by remember { mutableStateOf<PlayerPanel?>(null) }
    var wanted by remember { mutableStateOf<ChromeFocus?>(null) }
    val run = remember { SeekRun() }
    var presses by remember { mutableIntStateOf(0) }
    var runShown by remember { mutableStateOf(false) }
    var head by remember { mutableStateOf<ScrubHead?>(null) }
    var headAt by remember { mutableStateOf<Double?>(null) }
    var heldSince by remember { mutableLongStateOf(0L) }

    val starting = model.starting != null || !model.hasPicture
    val chrome = !starting && (model.paused || panel != null || headAt != null || model.clock - model.lastInput < CHROME_HIDE)
    val skip = model.skip?.takeIf { model.corner == PlayerModel.Corner.Skip }
    val skipFresh = skip != null && model.clock - skip.since < SKIP_FRESH

    fun show(focus: ChromeFocus) {
        model.poke()
        wanted = focus
    }

    fun openPanel(which: PlayerPanel) {
        panel = which
        model.hold("panel", true)
    }

    fun closePanel() {
        panel = null
        model.hold("panel", false)
        wanted = ChromeFocus.Play
    }

    fun nextEpisode() {
        val episodes = model.menus?.episodes ?: return
        val at = episodes.indexOfFirst { it.current }
        episodes.getOrNull(at + 1)?.let { model.choose(PlayerChoice.Episode(it.number)) }
    }

    // Back, top to bottom: a panel closes, a scrub cancels, Up Next cancels, else the player closes.
    BackHandler(enabled = model.isPresented) {
        when {
            panel != null -> closePanel()
            headAt != null -> {
                head = null
                headAt = null
            }
            model.corner == PlayerModel.Corner.UpNext -> model.cancelNext()
            else -> model.viewerClosed()
        }
    }

    // The settled seek: one seek half a second after the last press, the count shown a moment more.
    LaunchedEffect(presses) {
        if (presses == 0) return@LaunchedEffect
        delay(SEEK_SETTLE)
        model.seekBy(run.take())
        presses = 0
        delay(SEEK_SETTLE * 2)
        runShown = false
    }

    // Focus follows the chrome: on what asked for it as it comes up, back on the root as it goes.
    // Whether the root itself holds focus, as it does while the chrome is hidden.
    var onRoot by remember { mutableStateOf(false) }

    LaunchedEffect(chrome, wanted, onRoot) {
        if (!chrome) {
            repeat(3) {
                androidx.compose.runtime.withFrameNanos { }
                if (root.tryFocus()) return@LaunchedEffect
            }
            return@LaunchedEffect
        }
        // Up with nothing asked for (paused by a media key, or at the end of an episode): on
        // play/pause, so the D-pad is never left on the hidden root while the chrome shows.
        val asked = wanted ?: if (onRoot) ChromeFocus.Play else return@LaunchedEffect
        val target = when (asked) {
            ChromeFocus.Play -> playFocus
            ChromeFocus.SeekBar -> barFocus
            ChromeFocus.Episodes -> episodesFocus
        }
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (target.tryFocus()) {
                wanted = null
                return@LaunchedEffect
            }
        }
    }

    fun hiddenKey(event: KeyEvent): Boolean {
        if (event.type != KeyEventType.KeyDown) return false
        return when (event.key) {
            Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                if (skipFresh) {
                    model.takeSkip()
                } else {
                    model.togglePause()
                    show(ChromeFocus.Play)
                }
                true
            }
            Key.DirectionLeft, Key.DirectionRight -> {
                run.press(forward = event.key == Key.DirectionRight)
                presses = run.presses
                runShown = true
                true
            }
            Key.DirectionUp -> {
                show(ChromeFocus.SeekBar)
                true
            }
            Key.DirectionDown -> {
                show(ChromeFocus.Episodes)
                true
            }
            else -> false
        }
    }

    Box(
        modifier
            .fillMaxSize()
            .background(Color.Black)
            .onPreviewKeyEvent { event ->
                if (event.type == KeyEventType.KeyDown && event.nativeKeyEvent.repeatCount == 0) heldSince = model.clock
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                // Any key, the TV's remote's or a phone's: somebody is there.
                model.viewerThere()
                when (event.key) {
                    Key.MediaPlayPause -> { model.togglePause(); true }
                    Key.MediaPlay -> { model.setPlaying(true); true }
                    Key.MediaPause -> { model.setPlaying(false); true }
                    Key.MediaRewind -> { model.seekBy(if (event.nativeKeyEvent.repeatCount > 0) -30.0 else -10.0); true }
                    Key.MediaFastForward -> { model.seekBy(if (event.nativeKeyEvent.repeatCount > 0) 30.0 else 10.0); true }
                    Key.MediaNext -> { nextEpisode(); true }
                    Key.MediaPrevious -> { model.seekTo(0.0); true }
                    else -> {
                        if (chrome) model.poke()
                        false
                    }
                }
            }
            .focusRequester(root)
            .onKeyEvent { event -> if (!chrome && !starting) hiddenKey(event) else false }
            .onFocusChanged { onRoot = it.isFocused }
            .focusable(),
    ) {
        video(Modifier.fillMaxSize())
        StartingLayer(model, starting, onClose = { model.viewerClosed() })
        AnimatedVisibility(visible = chrome, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
            Chrome(
                model,
                headAt = headAt,
                playFocus = playFocus,
                barFocus = barFocus,
                episodesFocus = episodesFocus,
                onPanel = ::openPanel,
                onNext = ::nextEpisode,
                hasNext = model.menus?.episodes?.let { list -> list.indexOfFirst { it.current }.let { it >= 0 && it < list.lastIndex } } == true,
                onBarKey = { event ->
                    val duration = model.duration ?: return@Chrome false
                    if (event.type != KeyEventType.KeyDown) return@Chrome false
                    when (event.key) {
                        Key.DirectionLeft, Key.DirectionRight -> {
                            val scrub = head ?: ScrubHead(model.position, duration, model.segments).also { head = it }
                            val held = if (event.nativeKeyEvent.repeatCount > 0) model.clock - heldSince else 0L
                            scrub.move(forward = event.key == Key.DirectionRight, heldMillis = held)
                            headAt = scrub.at
                            model.poke()
                            true
                        }
                        Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                            headAt?.let { model.seekTo(it) }
                            head = null
                            headAt = null
                            true
                        }
                        Key.DirectionUp, Key.DirectionDown -> {
                            // Leaving the bar leaves the head; playback carries on where it was.
                            head = null
                            headAt = null
                            false
                        }
                        else -> false
                    }
                },
            )
        }
        if (!chrome && runShown) SeekCount(model, run.label)
        if (!chrome && skip != null) {
            SkipPill(skip.label, fresh = skipFresh, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 540.dp - 400.dp))
        }
        val upNext = model.upNext?.takeIf { model.corner == PlayerModel.Corner.UpNext && panel == null }
        if (upNext != null) {
            UpNextCard(model, upNext.next, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 27.dp))
        }
        panel?.let { open ->
            Panel(model, open, onDone = { closePanel() }, modifier = Modifier.align(Alignment.CenterEnd))
        }
        if (model.stillWatching) StillWatchingDialog(model)
    }
    LaunchedEffect(Unit) { root.tryFocus() }
}

/** The video plane: mpv's SurfaceView, new for each surface generation. */
@Composable
fun VideoSurface(model: PlayerModel, modifier: Modifier) {
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
                }
            },
            modifier = modifier,
        )
    }
}

/**
 * The starting screen: the ground, the banner at the top, the show title, "Starting episode 7"
 * and what the wait is on, and the 32 dp indicator. After 20 s, another source or Back.
 */
@Composable
private fun StartingLayer(model: PlayerModel, up: Boolean, onClose: () -> Unit) {
    val shown by animateFloatAsState(if (up) 1f else 0f, Motion.fadeIn(), label = "starting")
    if (shown == 0f) return
    val starting = model.starting
    Column(Modifier.fillMaxSize().alpha(shown).background(ObiSilk.Ground), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.fillMaxWidth().height(200.dp)) {
            val banner = starting?.bannerUrl ?: model.bannerUrl
            if (banner != null) ArtImage(banner, plate = null, maxPixelSize = 1920, modifier = Modifier.fillMaxSize())
        }
        Spacer(Modifier.height(40.dp))
        Text(model.showTitle, style = TvTitle.player, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(8.dp))
        Text(
            model.startingLine() ?: model.startingEpisode?.let { "Starting episode $it" }.orEmpty(),
            style = MaterialTheme.typography.bodyLarge,
            color = ObiSilk.Haze,
        )
        Spacer(Modifier.height(24.dp))
        Spinner()
        if (starting != null && starting.offersChoice(model.clock)) {
            val first = remember { FocusRequester() }
            Spacer(Modifier.height(24.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                TonalButton(onClick = { model.chooseSource() }, modifier = Modifier.focusRequester(first)) { ButtonLabel("Choose another source") }
                TonalButton(onClick = onClose) { ButtonLabel("Back") }
            }
            LaunchedEffect(Unit) { first.tryFocus() }
        }
    }
}

/** The 32 dp circular indicator, an amber arc turning. */
@Composable
private fun Spinner() {
    // A preview, and a screenshot, draws it still.
    val still = androidx.compose.ui.platform.LocalInspectionMode.current
    val turning = rememberInfiniteTransition(label = "spinner")
    val turned by turning.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing)), label = "angle")
    val angle = if (still) 0f else turned
    Canvas(Modifier.size(32.dp)) {
        val stroke = 3.dp.toPx()
        drawArc(ObiSilk.Rule, 0f, 360f, useCenter = false, style = Stroke(stroke))
        drawArc(ObiSilk.Amber, angle, 90f, useCenter = false, style = Stroke(stroke, cap = androidx.compose.ui.graphics.StrokeCap.Round))
    }
}

@Composable
private fun Chrome(
    model: PlayerModel,
    headAt: Double?,
    playFocus: FocusRequester,
    barFocus: FocusRequester,
    episodesFocus: FocusRequester,
    onPanel: (PlayerPanel) -> Unit,
    onNext: () -> Unit,
    hasNext: Boolean,
    onBarKey: (KeyEvent) -> Boolean,
) {
    val duration = model.duration ?: 0.0
    val position = model.position
    Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = DIM))) {
        // Top left, the show and the episode; top right, the clock and when it ends.
        Column(Modifier.align(Alignment.TopStart).padding(start = 48.dp, top = 27.dp).width(560.dp)) {
            Text(model.showTitle, style = TvTitle.player, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(model.episodeLine, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Column(Modifier.align(Alignment.TopEnd).padding(end = 48.dp, top = 27.dp), horizontalAlignment = Alignment.End) {
            val now = AppClock.now()
            Text(now.atZone(AppClock.zone).format(HourMinute), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
            if (duration > 0) {
                val left = ((duration - position) / model.speed.coerceAtLeast(0.1)).toLong()
                Text("Ends at ${now.plusSeconds(left).atZone(AppClock.zone).format(HourMinute)}", style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
            }
        }
        // Bottom: the times over the seek bar at y 437, the buttons at y 465.
        Column(Modifier.fillMaxWidth().padding(start = 48.dp, end = 48.dp).offset(y = 437.dp - 26.dp)) {
            Row(Modifier.fillMaxWidth()) {
                Text(clockTime(headAt ?: position), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
                Spacer(Modifier.weight(1f))
                if (duration > 0) Text(remainingTime(headAt ?: position, duration), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
            }
            Spacer(Modifier.height(6.dp))
            SeekBar(position, duration, headAt, model.segments, Modifier.focusRequester(barFocus), onBarKey)
            Spacer(Modifier.height(20.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(
                    onClick = { model.togglePause() },
                    icon = if (model.paused) TvIcons.Play else TvIcons.Pause,
                    modifier = Modifier.focusRequester(playFocus),
                ) { ButtonLabel(if (model.paused) "Play" else "Pause") }
                TonalButton(onClick = { onPanel(PlayerPanel.Episodes) }, icon = TvIcons.Episodes, modifier = Modifier.focusRequester(episodesFocus)) {
                    ButtonLabel("Episodes")
                }
                TonalButton(onClick = { onPanel(PlayerPanel.Tracks) }, icon = TvIcons.Subtitles) { ButtonLabel("Subtitles and audio") }
                TonalButton(onClick = { onPanel(PlayerPanel.Sources) }, icon = TvIcons.Source) { ButtonLabel("Source") }
                TonalButton(onClick = { onPanel(PlayerPanel.Info) }, icon = TvIcons.Info) { ButtonLabel("Info") }
                Spacer(Modifier.weight(1f))
                if (hasNext) TonalButton(onClick = onNext, icon = TvIcons.SkipNext) { ButtonLabel("Next episode") }
            }
        }
    }
}

/**
 * The seek bar, 6 high: amber played on the rule colour, the opening and ending in silk at 40 %.
 * Focused, it shows a head that Left and Right move without seeking, and Center jumps to.
 */
@Composable
private fun SeekBar(
    position: Double,
    duration: Double,
    headAt: Double?,
    segments: List<SeekSegment>,
    modifier: Modifier,
    onKey: (KeyEvent) -> Boolean,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .height(24.dp)
            .onKeyEvent(onKey)
            .focusable(interactionSource = interactions),
        contentAlignment = Alignment.CenterStart,
    ) {
        val width = maxWidth
        Canvas(Modifier.fillMaxWidth().height(24.dp)) {
            val bar = 6.dp.toPx()
            val top = (size.height - bar) / 2
            val radius = CornerRadius(bar / 2)
            drawRoundRect(ObiSilk.Rule, Offset(0f, top), Size(size.width, bar), radius)
            if (duration > 0) {
                segments.filter { it.kind != SeekSegment.Kind.Plain }.forEach { segment ->
                    val start = (segment.start / duration).toFloat() * size.width
                    val end = (segment.end / duration).toFloat() * size.width
                    drawRect(ObiSilk.Silk.copy(alpha = 0.4f), Offset(start, top), Size(end - start, bar))
                }
                val played = (position / duration).toFloat().coerceIn(0f, 1f) * size.width
                drawRoundRect(ObiSilk.Amber, Offset(0f, top), Size(played, bar), radius)
            }
        }
        if (duration > 0 && (focused || headAt != null)) {
            val at = ((headAt ?: position) / duration).toFloat().coerceIn(0f, 1f)
            Box(
                Modifier.offset(x = width * at - 8.dp)
                    .size(16.dp)
                    .background(ObiSilk.Silk, CircleShape),
            )
            if (headAt != null) {
                Text(
                    clockTime(headAt),
                    style = MaterialTheme.typography.labelLarge,
                    color = ObiSilk.Silk,
                    modifier = Modifier.offset(x = width * at - 24.dp, y = (-28).dp),
                )
            }
        }
    }
}

/** The running count beside a slim timeline at the bottom, while the chrome is hidden. */
@Composable
private fun SeekCount(model: PlayerModel, label: String) {
    val duration = model.duration ?: return
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.align(Alignment.BottomStart).padding(start = 48.dp, end = 48.dp, bottom = 40.dp).fillMaxWidth()) {
            Text(label, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            Spacer(Modifier.height(8.dp))
            Box(Modifier.fillMaxWidth().height(4.dp).background(ObiSilk.Rule, CircleShape)) {
                Box(Modifier.fillMaxWidth((model.position / duration).toFloat().coerceIn(0f, 1f)).height(4.dp).background(ObiSilk.Amber, CircleShape))
            }
        }
    }
}

/** The skip offer: silk while it holds Center, dimmed once it lets go. */
@Composable
private fun SkipPill(label: String, fresh: Boolean, modifier: Modifier) {
    Box(
        modifier
            .alpha(if (fresh) 1f else 0.6f)
            .background(if (fresh) ObiSilk.Silk else ObiSilk.RaisedSelected, CircleShape)
            .padding(horizontal = 20.dp, vertical = 12.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = if (fresh) ObiSilk.Ground else ObiSilk.Silk)
    }
}

/** A panel: the 400-wide column on the right; focus starts on the current item. */
@Composable
private fun Panel(model: PlayerModel, panel: PlayerPanel, onDone: () -> Unit, modifier: Modifier) {
    val current = remember { FocusRequester() }
    val list = remember { FocusRequester() }
    Column(
        modifier
            .width(400.dp)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(horizontal = 16.dp, vertical = 27.dp),
    ) {
        val title = when (panel) {
            PlayerPanel.Episodes -> "Episodes"
            PlayerPanel.Tracks -> "Subtitles and audio"
            PlayerPanel.Sources -> "Source"
            PlayerPanel.Info -> "Info"
        }
        Text(title, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 12.dp))
        LazyColumn(Modifier.focusRequester(list).focusGroup(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            when (panel) {
                PlayerPanel.Episodes -> items(model.menus?.episodes.orEmpty(), key = { it.number }) { episode ->
                    TvListRow(
                        episode.title?.let { "E${episode.number} · $it" } ?: episode.label,
                        onClick = {
                            if (!episode.current) model.choose(PlayerChoice.Episode(episode.number))
                            onDone()
                        },
                        supporting = when {
                            episode.current -> "Playing"
                            episode.watched -> "Watched"
                            else -> null
                        },
                        selected = episode.current,
                        modifier = if (episode.current) Modifier.focusRequester(current) else Modifier,
                    )
                }
                PlayerPanel.Tracks -> {
                    val audio = model.tracks.filter { it.kind == Track.Kind.Audio }
                    val subtitles = model.tracks.filter { it.kind == Track.Kind.Subtitle }
                    item(key = "audio") { PanelHeading("Audio") }
                    items(audio, key = { "audio/${it.id}" }) { track ->
                        TvListRow(
                            track.name,
                            onClick = { model.selectAudio(track.id) },
                            selected = track.selected,
                            modifier = if (track.selected) Modifier.focusRequester(current) else Modifier,
                        )
                    }
                    item(key = "subtitles") { PanelHeading("Subtitles") }
                    item(key = "subtitles/off") {
                        val off = subtitles.none { it.selected }
                        TvListRow("Off", onClick = { model.selectSubtitle(null) }, selected = off)
                    }
                    items(subtitles, key = { "sub/${it.id}" }) { track ->
                        TvListRow(track.name, onClick = { model.selectSubtitle(track.id) }, selected = track.selected)
                    }
                    item(key = "style") {
                        Text(
                            "Style follows the system's caption settings",
                            style = MaterialTheme.typography.bodyMedium,
                            color = ObiSilk.Haze,
                            modifier = Modifier.padding(16.dp),
                        )
                    }
                }
                PlayerPanel.Info -> item(key = "info") { InfoPanel(model, Modifier.focusRequester(current)) }
                PlayerPanel.Sources -> items(model.menus?.sources.orEmpty(), key = { it.id }) { source ->
                    TvListRow(
                        source.label,
                        onClick = {
                            if (!source.current) model.choose(PlayerChoice.Source(source.id))
                            onDone()
                        },
                        supporting = source.detail,
                        selected = source.current,
                        modifier = if (source.current) Modifier.focusRequester(current) else Modifier,
                    )
                }
            }
        }
    }
    LaunchedEffect(panel) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (current.tryFocus()) return@LaunchedEffect
        }
        // Nothing current (subtitles off, an empty list): the panel's first row.
        list.tryFocus()
    }
}

@Composable
private fun PanelHeading(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp))
}

/**
 * TVPlayerUpNext: the next episode bottom right as the ending starts, its still (the spoiler rule:
 * blurred, as it is not watched yet), "Plays in 8 s" over the bar, Play now focused and Cancel.
 */
@Composable
private fun UpNextCard(model: PlayerModel, next: NextEpisode, modifier: Modifier) {
    val play = remember { FocusRequester() }
    LaunchedEffect(next.number) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (play.tryFocus()) return@LaunchedEffect
        }
    }
    Row(
        modifier
            .width(UP_NEXT_WIDTH)
            .background(ObiSilk.RaisedHigh, RoundedCornerShape(16.dp))
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        ArtImage(
            next.thumbnail,
            plate = ObiSilk.Raised,
            maxPixelSize = 368,
            spoiler = true,
            modifier = Modifier.size(184.dp, 104.dp).clip(RoundedCornerShape(8.dp)),
        )
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                next.title?.let { "Next: E${next.number} · $it" } ?: "Next: E${next.number}",
                style = TvTitle.player,
                color = ObiSilk.Silk,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            model.countdown?.let { (left, progress) ->
                Text("Plays in $left s", style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
                Box(Modifier.fillMaxWidth().height(4.dp).background(ObiSilk.Rule, CircleShape)) {
                    Box(Modifier.fillMaxWidth(progress).fillMaxHeight().background(ObiSilk.Amber, CircleShape))
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TonalButton(onClick = { model.playNext() }, modifier = Modifier.focusRequester(play)) { ButtonLabel("Play now") }
                TonalButton(onClick = { model.cancelNext() }) { ButtonLabel("Cancel") }
            }
        }
    }
}

private val UP_NEXT_WIDTH = 560.dp

/** "Still watching?" after episodes in a row with nobody there: paused, Continue focused. */
@Composable
private fun StillWatchingDialog(model: PlayerModel) {
    androidx.compose.ui.window.Dialog(onDismissRequest = { model.continueWatching() }) {
        val keep = remember { FocusRequester() }
        Column(
            Modifier
                .entryFocus(keep)
                .width(480.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            Text("Still watching?", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = { model.continueWatching() }, modifier = Modifier.focusRequester(keep)) { ButtonLabel("Continue") }
                TonalButton(onClick = { model.stopWatching() }) { ButtonLabel("Stop") }
            }
        }
    }
}

/** TVPlayerInfo: the episode, then how it plays, where heavy subtitles show their cost. */
@Composable
private fun InfoPanel(model: PlayerModel, modifier: Modifier) {
    val store = LocalAppStore.current
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val number = model.episode?.number
    val row = model.titleId?.let { episodes[it] }?.firstOrNull { it.number == number }
    Column(
        // Focusable, so Back and the D-pad stay in the panel.
        modifier.focusable().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(model.episodeLine, style = TvTitle.player, color = ObiSilk.Silk)
        row?.description?.takeIf { it.isNotBlank() }?.let {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 8, overflow = TextOverflow.Ellipsis)
        }
        Text("Playback details", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
        playbackDetails(model).forEach { line ->
            Text(line, style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
        }
    }
}

/**
 * The Info panel's playback lines: "1080p · HEVC 10-bit · hardware decoding", the source, the
 * dropped frames, and the upscaling.
 */
fun playbackDetails(model: PlayerModel): List<String> = buildList {
    val picture = listOfNotNull(
        model.videoHeight?.let { "${it}p" },
        model.videoCodec?.let { codec ->
            val name = codecName(codec)
            if (model.pixelFormat?.contains("10") == true) "$name 10-bit" else name
        },
        if (model.decoding.trim() == "no") "software decoding" else "hardware decoding",
    )
    add(picture.joinToString(" · "))
    model.menus?.sources?.firstOrNull { it.current }?.let { add(it.label) }
    add("Dropped frames: ${model.droppedFrames}")
    val upscaling = model.menus?.upscaling
    val chosen = upscaling?.options?.firstOrNull { it.id == upscaling.current }?.label
    add("Upscaling: ${chosen?.lowercase() ?: "off on this TV"}")
}

/** mpv's codec names as people read them. */
fun codecName(codec: String): String = when (codec.lowercase()) {
    "hevc", "h265" -> "HEVC"
    "h264", "avc" -> "H.264"
    "av1" -> "AV1"
    "vp9" -> "VP9"
    else -> codec.uppercase()
}
