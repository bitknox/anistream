// Casting to TVs with Chromecast built in: the Chromecast group of the Play on TV sheet, the card
// of the cast running now, the bar that leads back to it, and the notice when someone else takes
// the TV. The engine's sender says what the rows, the card and the notices read (CastModel);
// these lay them out as the Play on TV sheets are.

package tv.anistream.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderColors
import androidx.compose.material3.SliderState
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.ffi.FfiCastRouteHint
import tv.anistream.engine.ffi.FfiCastUpNext
import tv.anistream.engine.ffi.FfiCastRow
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvTrack
import tv.anistream.engine.ffi.FfiTvTrackKind
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.SkipKind
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.CastModel
import tv.anistream.store.CastShown
import tv.anistream.store.CastStage
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.text

/** The glyphs casting adds to the app's set: the same 24 grid, stroked at 2 with round ends. */
private object CastIcons {
    private fun stroked(name: String, vararg paths: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in paths) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    /** A screen with the waves rising from its lower left corner. */
    val Cast = stroked(
        "Cast", // i18n-skip
        "M3 8 V6 A2 2 0 0 1 5 4 H19 A2 2 0 0 1 21 6 V18 A2 2 0 0 1 19 20 H13",
        "M3 12 A8 8 0 0 1 11 20",
        "M3 16 A4 4 0 0 1 7 20",
        "M3 20 H3.01",
    )
}

// The picker's Chromecast group

/** What a Chromecast row says under its name: whether the stream goes out through this device. */
@Composable
private fun castDetail(row: FfiCastRow): String {
    val tablet = LocalConfiguration.current.smallestScreenWidthDp >= 600
    return when (row.routeHint) {
        FfiCastRouteHint.THROUGH_THIS_DEVICE ->
            stringResource(if (tablet) R.string.cast_sheet_through_tablet else R.string.cast_sheet_through_phone)
        FfiCastRouteHint.DIRECT -> row.model
    }
}

/**
 * The TVs with Chromecast built in, between On this network and Not listed, looked for while the
 * picker is up. Only with an episode to play: a cast plays one, and pairs with nothing.
 */
@Composable
internal fun CastPickerGroup(cast: CastModel, request: TvPlayRequest?) {
    if (request == null || !cast.available) return
    val rows by cast.rows.collectAsStateWithLifecycle()
    val problem by cast.problem.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    DisposableEffect(cast) {
        cast.startBrowsing()
        onDispose { cast.stopBrowsing() }
    }
    if (rows.isEmpty()) return
    TvHeading(stringResource(R.string.cast_sheet_group))
    rows.forEachIndexed { index, row ->
        if (index > 0) TvDivider()
        TvSheetRow(
            row.name,
            castDetail(row),
            leading = { Icon(CastIcons.Cast, contentDescription = null, tint = colors.onSurfaceVariant) },
            trailing = { Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant) },
            onClick = { cast.cast(row, request) },
        )
    }
    problem?.let { ProblemLine(it) }
}

/**
 * Android 17's local network permission, turned down: no TV can be found or reached until it is
 * allowed in the app's settings, which Open settings goes to.
 */
@Composable
internal fun CastLocalNetworkRow() {
    val context = LocalContext.current
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.cast_sheet_local_network), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
        Button(
            onClick = {
                try {
                    context.startActivity(
                        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)) // i18n-skip
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                } catch (_: ActivityNotFoundException) {
                }
            },
        ) { Text(stringResource(R.string.action_open_settings)) }
    }
}

// The card

/** The cast's card over whatever is on screen, and the notice when someone else took the TV. */
@Composable
internal fun CastSheetHost() {
    val cast = LocalAppStore.current.cast
    val shown by cast.shown.collectAsStateWithLifecycle()
    val open by cast.open.collectAsStateWithLifecycle()
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    LaunchedEffect(cast) {
        cast.ended.collect { ended ->
            snackbars.show(ended.notice.text(context), Strings.castCardPlayHere(context)) {
                cast.playHere?.invoke(ended.id, ended.episode)
            }
        }
    }
    val held = shown ?: return
    if (!open) return
    val over = held.stage == CastStage.Failed || held.stage == CastStage.Lost
    // A card that only says why the cast ended goes with its sheet.
    SettingsSheet(onDismiss = if (over) cast::dismiss else cast::closeCard, tall = true) {
        CastCard(cast, held)
    }
}

/**
 * The line at the top of the card: where the cast is, after its dot. While an episode starts it
 * is the engine's phase line, which counts the seconds itself.
 */
@Composable
private fun CastPhase(shown: CastShown) {
    val context = LocalContext.current
    val stage = shown.stage
    val line = when (stage) {
        CastStage.Starting -> shown.cast.phaseLine?.text(context) ?: Strings.tvSessionStarting(context, shown.tvName)
        CastStage.Playing -> Strings.tvStatusPlaying(context, shown.tvName)
        CastStage.Failed -> Strings.castCardFailed(context, shown.tvName)
        CastStage.Lost -> Strings.castCardStopped(context)
    }
    val dot = when (stage) {
        CastStage.Playing -> ObiSilk.Water
        CastStage.Failed, CastStage.Lost -> ObiSilk.Vermilion
        CastStage.Starting -> null
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        StateDot(dot)
        SheetText(line)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CastCard(cast: CastModel, shown: CastShown) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val now = CastModel.now(shown)
    val stage = shown.stage
    val playing = stage == CastStage.Playing
    val entry = store.titles[now.id]
    var choosingSource by remember { mutableStateOf(false) }
    CastPhase(shown)
    Spacer(Modifier.height(16.dp))
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        entry?.let { CoverArt(it, 72.dp, Modifier.size(72.dp, 108.dp)) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(now.title, style = ShowTitle.row, color = colors.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                now.episodeLine.ifEmpty { Strings.castEpisode(context, now.episode) },
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            shown.cast.notice?.let {
                Text(it.text(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurface, modifier = Modifier.padding(top = 4.dp))
            }
            if (playing) {
                now.skip?.let { skip ->
                    OutlinedButton(onClick = { cast.send(FfiTvCommand.Skip) }, modifier = Modifier.padding(top = 4.dp)) { Text(skip.label) }
                }
            }
        }
    }
    if (stage == CastStage.Starting || playing) {
        shown.cast.undo?.let { CastUndoNote(cast, it.kind) }
        CastSeekLine(cast, now, shown, playing)
        shown.cast.upNext?.let { CastUpNext(cast, it, entry) }
        CastTransport(cast, now, playing)
        TvDivider(Modifier.padding(top = 8.dp))
        CastTrackRow(stringResource(R.string.player_subtitles), FfiTvTrackKind.SUBTITLE, now.subtitles, now.subtitlesOff, cast, enabled = playing)
        if (now.audio.isNotEmpty()) {
            TvDivider()
            CastTrackRow(stringResource(R.string.player_audio), FfiTvTrackKind.AUDIO, now.audio, off = false, cast, enabled = playing)
        }
        TvDivider()
        CastVolume(cast, shown, now, enabled = playing)
    }
    // What each ending offers, as the board lays them out.
    val notice = shown.cast.notice
    val choose = entry != null
    FlowRow(
        Modifier.fillMaxWidth().padding(top = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val playHere = @Composable { CastPill(stringResource(R.string.cast_card_play_here), cast::playHere) }
        val another = @Composable { CastPill(stringResource(R.string.tv_choose_another), cast::chooseAnother) }
        val source = @Composable { if (choose) CastPill(stringResource(R.string.player_choose_source)) { choosingSource = true } }
        when (notice) {
            null -> {
                playHere()
                another()
                CastPill(stringResource(R.string.cast_card_stop), cast::stop)
            }
            is Msg.NoticeCastNeverFetches -> {
                CastPill(stringResource(R.string.action_try_again), cast::tryAgain)
                playHere()
            }
            is Msg.NoticeCastNotSameNetwork, is Msg.NoticeCastLost, is Msg.NoticeCastNoDevice -> {
                playHere()
                another()
            }
            is Msg.NoticeCastDeepLink -> source()
            else -> {
                source()
                playHere()
            }
        }
    }
    if (choosingSource && entry != null) SourcePickerSheet(entry) { choosingSource = false }
}

/**
 * The card's slider: a 4 dp line with the thumb the board draws, none while there is nothing to
 * move. Reads [value] and reports the drag through [onChange] and its end through [onFinished].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CastSlider(
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    onChange: (Float) -> Unit,
    onFinished: () -> Unit,
    enabled: Boolean,
    colors: SliderColors,
    thumb: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val state = remember(range) { SliderState(value, 0, range) }
    if (state.value != value) state.value = value
    Slider(
        state = state,
        onValueChange = onChange,
        onValueChangeFinished = onFinished,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        thumb = { thumb?.invoke() },
        track = {
            SliderDefaults.Track(
                it,
                modifier = Modifier.height(4.dp),
                enabled = enabled,
                colors = colors,
                drawStopIndicator = null,
                thumbTrackGapSize = 0.dp,
            )
        },
    )
}

/** After an automatic skip: what it did, and Undo. */
@Composable
private fun CastUndoNote(cast: CastModel, kind: SkipKind) {
    val context = LocalContext.current
    Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            if (kind == SkipKind.OPENING) Strings.castCardSkippedOpening(context) else Strings.castCardSkippedEnding(context),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        QuietButton(stringResource(R.string.cast_card_undo)) { cast.undoSkip() }
    }
}

/**
 * The next episode near the end: the cover, "Up next · plays in 6 s" over its number and title, a
 * 2 dp line running down the count, then Cancel and Play now. With auto-next off it waits for Play now.
 */
@Composable
private fun CastUpNext(cast: CastModel, next: FfiCastUpNext, entry: Entry?) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val left = next.secondsLeft?.toInt()
    // The count's first value is the whole of its line.
    var total by remember(next.number) { mutableStateOf(1) }
    if (left != null && left > total) total = left
    Column(Modifier.fillMaxWidth().padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        TvDivider()
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            entry?.let { CoverArt(it, 40.dp, Modifier.size(40.dp, 60.dp)) }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    if (left != null) Strings.castCardUpNextIn(context, left) else Strings.castCardUpNext(context),
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.onSurfaceVariant,
                )
                Text(
                    next.title?.let { Strings.castEpisodeTitled(context, next.number, it) } ?: Strings.castEpisode(context, next.number),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (left != null) {
            Box(Modifier.fillMaxWidth().height(2.dp).background(ObiSilk.Rule)) {
                Box(Modifier.fillMaxWidth((left.toFloat() / total).coerceIn(0f, 1f)).height(2.dp).background(ObiSilk.Water))
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = cast::cancelNext, modifier = Modifier.weight(1f)) {
                Text(stringResource(R.string.cast_card_up_next_cancel), color = colors.onSurface)
            }
            Button(onClick = cast::playNext, modifier = Modifier.weight(1f)) { Text(stringResource(R.string.cast_card_up_next_play)) }
        }
    }
}

/** An action on the card: an outlined pill, never amber. */
@Composable
private fun CastPill(text: String, onClick: () -> Unit) {
    OutlinedButton(onClick = onClick) { Text(text, color = MaterialTheme.colorScheme.onSurface) }
}

/**
 * The TV's playhead on a 4 dp line, amber up to the handle, with the position and the length
 * under it once the TV plays. Dragged, it seeks on release and holds there until the TV's next
 * card, or 3 s.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CastSeekLine(cast: CastModel, now: FfiNowCard, shown: CastShown, playing: Boolean) {
    val colors = MaterialTheme.colorScheme
    val duration = now.duration?.takeIf { it > 0 } ?: 0.0
    val position by produceState(cast.playhead() ?: now.position, now, playing) {
        while (true) {
            value = cast.playhead() ?: now.position
            if (now.paused || !playing) break
            delay(1000)
        }
    }
    var dragging by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    var sent by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    LaunchedEffect(shown.cast.card.rev) { sent = null }
    LaunchedEffect(sent) {
        if (sent != null) {
            delay(3_000)
            sent = null
        }
    }
    val at = ((dragging ?: sent)?.toDouble() ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    val time = MaterialTheme.typography.labelMedium.tabular()
    val positionLabel = stringResource(R.string.tv_sheet_position)
    val positionState = stringResource(R.string.tv_sheet_position_state, TvWords.clock(at), TvWords.clock(duration))
    val sliderColors = SliderDefaults.colors(
        thumbColor = colors.primary,
        activeTrackColor = colors.primary,
        inactiveTrackColor = ObiSilk.Rule,
        disabledThumbColor = ObiSilk.Rule,
        disabledActiveTrackColor = ObiSilk.Rule,
        disabledInactiveTrackColor = ObiSilk.Rule,
    )
    val usable = playing && duration > 0
    Column(Modifier.padding(top = 16.dp)) {
        CastSlider(
            value = at.toFloat(),
            range = 0f..maxOf(duration.toFloat(), 1f),
            onChange = { dragging = it },
            onFinished = {
                dragging?.let {
                    cast.send(FfiTvCommand.SeekTo(it.toDouble()))
                    sent = it
                }
                dragging = null
            },
            enabled = usable,
            colors = sliderColors,
            thumb = if (usable) ({ Box(Modifier.size(12.dp).background(colors.primary, RoundedCornerShape(6.dp))) }) else null,
            modifier = Modifier.fillMaxWidth().semantics {
                contentDescription = positionLabel
                if (playing) stateDescription = positionState
            },
        )
        // The times only once the TV plays: while it starts there is nothing to count.
        Row(Modifier.fillMaxWidth().alpha(if (playing) 1f else 0f)) {
            Text(TvWords.clock(at), style = time, color = colors.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            Text(if (duration > 0) TvWords.clock(duration) else "", style = time, color = colors.onSurfaceVariant)
        }
    }
}

/** Back 10, Pause in its 64 dp tonal square, forward 10 and Next; faded while the TV starts. */
@Composable
private fun CastTransport(cast: CastModel, now: FfiNowCard, playing: Boolean) {
    Row(
        Modifier.fillMaxWidth().padding(top = 8.dp).alpha(if (playing) 1f else 0.4f),
        horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TvStep(Icons.Replay, stringResource(R.string.player_back_10), enabled = playing) { cast.send(FfiTvCommand.Seek(-10.0)) }
        val label = stringResource(if (now.paused) R.string.action_play else R.string.action_pause)
        FilledTonalIconButton(
            onClick = { cast.send(FfiTvCommand.TogglePause) },
            enabled = playing,
            shape = RoundedCornerShape(16.dp),
            modifier = Modifier.size(64.dp).semantics { contentDescription = label },
        ) {
            Icon(if (now.paused) Icons.PlayRow else Icons.Pause, contentDescription = null, modifier = Modifier.size(32.dp))
        }
        TvStep(Icons.Ahead, stringResource(R.string.player_forward_10), enabled = playing) { cast.send(FfiTvCommand.Seek(10.0)) }
        TvGlyph(Icons.SkipNext, stringResource(R.string.player_next_episode), enabled = playing) { cast.send(FfiTvCommand.Next) }
    }
}

/** The subtitle or audio choice: its value under its name, a menu of the tracks, the one on checked. */
@Composable
private fun CastTrackRow(title: String, kind: FfiTvTrackKind, tracks: List<FfiTvTrack>, off: Boolean, cast: CastModel, enabled: Boolean) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    Box {
        TvSheetRow(
            title,
            TvWords.chosen(context, tracks, off),
            modifier = Modifier.alpha(if (enabled) 1f else 0.38f),
            trailing = { Icon(Icons.DropDown, contentDescription = null, tint = colors.onSurfaceVariant) },
            onClick = if (tracks.isEmpty() || !enabled) null else ({ open = true }),
        )
        Menu(expanded = open, onDismissRequest = { open = false }) {
            val choices = buildList {
                if (kind == FfiTvTrackKind.SUBTITLE) add(null to Strings.valueOff(context))
                tracks.forEach { add(it to TvWords.track(context, it)) }
            }
            val selected = stringResource(R.string.value_selected)
            for ((track, label) in choices) {
                val picked = if (track == null) off else !off && track.selected
                DropdownMenuItem(
                    text = { Text(label, style = MaterialTheme.typography.labelLarge) },
                    leadingIcon = {
                        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                            if (picked) Icon(Icons.Check, contentDescription = null, tint = colors.onSurface)
                        }
                    },
                    modifier = Modifier.heightIn(min = 48.dp).semantics { if (picked) stateDescription = selected },
                    onClick = {
                        open = false
                        cast.send(FfiTvCommand.Track(kind, track?.id))
                    },
                )
            }
        }
    }
}

/**
 * The TV's volume: a slider between the speakers on a TV that takes a level, sending the
 * difference on release; on one whose volume is fixed, "Set on the TV" with Mute alone.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CastVolume(cast: CastModel, shown: CastShown, now: FfiNowCard, enabled: Boolean) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val muted = shown.cast.muted == true
    if (!shown.level) {
        TvSheetRow(
            stringResource(R.string.tv_sheet_volume),
            stringResource(R.string.cast_card_volume_fixed),
            modifier = Modifier.alpha(if (enabled) 1f else 0.38f),
            trailing = {
                CastPill(stringResource(if (muted) R.string.cast_card_unmute else R.string.cast_card_mute)) {
                    if (enabled) cast.toggleMute()
                }
            },
        )
        return
    }
    val level = now.volume ?: 0.0
    var dragging by remember { mutableStateOf<Float?>(null) }
    var sent by remember { mutableStateOf<Float?>(null) }
    LaunchedEffect(shown.cast.card.rev) { sent = null }
    val at = dragging ?: sent ?: level.toFloat()
    val label = stringResource(R.string.tv_sheet_volume)
    val value = TvWords.volume(context, now.copy(volume = at.toDouble()))
    val sliderColors = SliderDefaults.colors(
        thumbColor = colors.onSurface,
        activeTrackColor = colors.onSurface,
        inactiveTrackColor = ObiSilk.Rule,
    )
    Column(Modifier.padding(vertical = 8.dp).alpha(if (enabled) 1f else 0.38f)) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TvGlyph(
                if (muted) Icons.SoundOff else Icons.VolumeDown,
                stringResource(if (muted) R.string.cast_card_unmute else R.string.cast_card_mute),
                enabled = enabled,
                onClick = cast::toggleMute,
            )
            CastSlider(
                value = at,
                range = 0f..100f,
                onChange = { dragging = it },
                onFinished = {
                    dragging?.let {
                        cast.setVolume(it.toDouble())
                        sent = it
                    }
                    dragging = null
                },
                enabled = enabled,
                colors = sliderColors,
                thumb = { Box(Modifier.size(4.dp, 28.dp).background(colors.onSurface, RoundedCornerShape(2.dp))) },
                modifier = Modifier.weight(1f).semantics {
                    contentDescription = label
                    value?.let { stateDescription = it }
                },
            )
            Icon(Icons.VolumeUp, contentDescription = null, tint = colors.onSurface, modifier = Modifier.padding(horizontal = 12.dp))
        }
    }
}

// The way back to the card

/**
 * The cast, over the navigation bar while its card is closed: the cover, the title, "Playing on
 * Living room" and play or pause, on surfaceContainerHigh with the playhead in amber along its
 * top. A tap opens the card.
 */
@Composable
internal fun CastNowPlayingBar(modifier: Modifier = Modifier) {
    val cast = LocalAppStore.current.cast
    val shown by cast.shown.collectAsStateWithLifecycle()
    val open by cast.open.collectAsStateWithLifecycle()
    val held = shown
    val visible = held != null && !open && (held.stage == CastStage.Starting || held.stage == CastStage.Playing)
    val last = remember { LastCast() }
    val showing = if (visible) held else last.value
    SideEffect { if (visible) last.value = held }
    AnimatedVisibility(
        visible,
        modifier = modifier,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
    ) {
        showing?.let { CastNowPlayingBarContent(cast, it) }
    }
}

/** The cast the bar last showed, kept out of the snapshot: it is only read as the bar leaves. */
private class LastCast {
    var value: CastShown? = null
}

@Composable
private fun CastNowPlayingBarContent(cast: CastModel, shown: CastShown) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val now = CastModel.now(shown)
    val playing = shown.stage == CastStage.Playing
    val duration = now.duration?.takeIf { it > 0 }
    val position by produceState(cast.playhead() ?: now.position, now, playing) {
        while (true) {
            value = cast.playhead() ?: now.position
            if (now.paused || !playing) break
            delay(1000)
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .background(colors.surfaceContainerHigh)
            .clickable(onClickLabel = stringResource(R.string.tv_show_remote, shown.tvName)) { cast.openCard() },
    ) {
        Box(Modifier.fillMaxWidth().height(2.dp).background(ObiSilk.Rule)) {
            if (duration != null && playing) {
                Box(Modifier.fillMaxWidth((position / duration).toFloat().coerceIn(0f, 1f)).height(2.dp).background(colors.primary))
            }
        }
        Row(
            Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            store.titles[now.id]?.let { CoverArt(it, 32.dp, Modifier.size(32.dp, 48.dp)) }
            Column(Modifier.weight(1f)) {
                Text(now.title, style = ShowTitle.caption, color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    if (playing) Strings.tvStatusPlaying(context, shown.tvName) else Strings.tvSessionStarting(context, shown.tvName),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Box(Modifier.alpha(if (playing) 1f else 0.38f)) {
                TvGlyph(
                    if (now.paused) Icons.PlayRow else Icons.Pause,
                    stringResource(if (now.paused) R.string.action_play else R.string.action_pause),
                    enabled = playing,
                ) { cast.send(FfiTvCommand.TogglePause) }
            }
        }
    }
}
