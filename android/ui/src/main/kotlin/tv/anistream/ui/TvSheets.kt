// Play on TV: the sheet that chooses a TV and pairs with it, the now-playing card that drives it,
// the bar that leads back to the card, and the paired TVs in Settings. The engine's sender says
// what the rows, the pairing's failures and the card read (TvRemoteModel); these only lay them
// out, as the app's other sheets are: SettingsSheet on surfaceContainer, flat rows with haze
// headings, the one filled amber action at the foot.

package tv.anistream.ui

import androidx.annotation.StringRes
import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import android.content.ActivityNotFoundException
import android.content.Intent
import android.provider.MediaStore
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.util.Locale
import kotlinx.coroutines.delay
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.ffi.FfiLinkPhase
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiRemoteCard
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvRow
import tv.anistream.engine.ffi.FfiTvRowKind
import tv.anistream.engine.ffi.FfiTvTrack
import tv.anistream.engine.ffi.FfiTvTrackKind
import tv.anistream.store.TvCard
import tv.anistream.store.TvPairingStep
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.TvRemoteModel

/** What the TV sheets say. */
internal object TvWords {
    fun pickerTitle(context: Context, request: TvPlayRequest?): String =
        if (request == null) Strings.tvSheetTitlePair(context) else Strings.tvPlayOn(context)

    fun pickerLine(context: Context, request: TvPlayRequest?): String = when {
        request == null -> Strings.tvOpenOnTv(context)
        request.title.isEmpty() -> Strings.tvSheetRequestPlain(context, request.episode)
        else -> Strings.tvSheetRequestTitled(context, request.title, request.episode)
    }

    fun pairingLine(context: Context, name: String): String = Strings.tvTypeCode(context, name)

    @StringRes val EMPTY = R.string.tv_looking
    @StringRes val NONE_FOUND = R.string.tv_none_found

    /** What to check when no TV answers. The picker's own line says it already when it is pairing, with nothing to play. */
    fun noneLine(context: Context, request: TvPlayRequest?): String =
        if (request == null) Strings.tvNoneFoundShort(context) else Strings.tvNoneFoundDetail(context)

    @StringRes val ADDRESS = R.string.tv_address_hint
    @StringRes val SCAN = R.string.tv_scan_hint
    @StringRes val NOTHING = R.string.tv_after_pairing

    /** A track as the card names it: its language in this phone's language, and its title. */
    fun track(context: Context, track: FfiTvTrack): String {
        val language = track.language?.takeIf { it.isNotBlank() }
            ?.let { Locale.forLanguageTag(it).getDisplayLanguage(Locale.getDefault()).takeIf(String::isNotBlank) }
        val title = track.title?.takeIf { it.isNotBlank() }
        return when {
            language != null && title != null -> "$language · $title" // i18n-skip
            language != null -> language
            title != null -> title
            else -> Strings.tvTrackN(context, track.id.toInt())
        }
    }

    fun chosen(context: Context, tracks: List<FfiTvTrack>, off: Boolean): String =
        if (off) Strings.valueOff(context) else tracks.firstOrNull { it.selected }?.let { track(context, it) } ?: Strings.valueNone(context)

    fun volume(context: Context, now: FfiNowCard): String? = now.volume?.let { Strings.tvSheetVolumeValue(context, Math.round(it).toInt()) }

    fun status(context: Context, card: FfiRemoteCard?): String = card?.phaseLine ?: Strings.tvStatusConnected(context)

    fun lost(card: FfiRemoteCard?): Boolean = card?.phase == FfiLinkPhase.UNPAIRED || card?.phase == FfiLinkPhase.KEY_CHANGED

    /** "1:02:03" or "4:05". */
    fun clock(seconds: Double): String {
        val total = seconds.coerceAtLeast(0.0).toLong()
        val h = total / 3600
        val m = total % 3600 / 60
        val s = total % 60
        return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
    }
}

/** The dot beside the link's line: water while connected, vermilion when the TV let this phone go. */
@Composable
private fun phaseDot(phase: FfiLinkPhase?) = when (phase) {
    FfiLinkPhase.CONNECTED -> ObiSilk.Water
    FfiLinkPhase.UNPAIRED, FfiLinkPhase.KEY_CHANGED -> ObiSilk.Vermilion
    else -> null
}

/** The TV sheet, over whatever is on screen, the player too. */
@Composable
internal fun TvSheetHost() {
    val tv = LocalAppStore.current.tv
    val sheet by tv.sheet.collectAsStateWithLifecycle()
    val pairing by tv.pairing.collectAsStateWithLifecycle()
    val shown = sheet ?: return
    SettingsSheet(onDismiss = tv::sheetClosed, tall = shown == TvRemoteModel.Sheet.Remote) {
        val step = pairing
        when {
            step != null -> TvPairing(tv, step)
            shown == TvRemoteModel.Sheet.Picker -> TvPicker(tv)
            else -> TvRemote(tv)
        }
    }
}

// The picker

@Composable
private fun TvPicker(tv: TvRemoteModel) {
    val request by tv.request.collectAsStateWithLifecycle()
    val rows by tv.rows.collectAsStateWithLifecycle()
    val connected by tv.connected.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var address by rememberSaveable { mutableStateOf("") }
    DisposableEffect(tv) {
        tv.startBrowsing()
        onDispose { tv.stopBrowsing() }
    }
    SheetTitle(TvWords.pickerTitle(context, request))
    SheetText(TvWords.pickerLine(context, request))
    TvHeading(stringResource(R.string.tv_sheet_on_network))
    if (rows.isEmpty()) TvSearchingRow(request)
    rows.forEachIndexed { index, row ->
        if (index > 0) TvDivider()
        TvPickerRow(row, chosen = row.fingerprint != null && row.fingerprint == connected) { tv.choose(row) }
    }
    TvHeading(stringResource(R.string.tv_sheet_not_listed))
    TvSheetRow(
        stringResource(R.string.tv_sheet_scan),
        stringResource(TvWords.SCAN),
        onClick = {
            try {
                context.startActivity(Intent(MediaStore.INTENT_ACTION_STILL_IMAGE_CAMERA))
            } catch (_: ActivityNotFoundException) {
            }
        },
    )
    Spacer(Modifier.height(8.dp))
    SheetField(
        address,
        { address = it },
        stringResource(R.string.tv_address),
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
        actions = KeyboardActions(onGo = { address.trim().takeIf(String::isNotEmpty)?.let(tv::begin) }),
        supporting = stringResource(TvWords.ADDRESS),
    )
    SheetButtons {
        QuietButton(stringResource(R.string.action_close), onClick = tv::sheetClosed)
    }
}

private enum class TvSearch { QUIET, LOOKING, NONE }

/**
 * The picker before any TV answers: its place held empty for 600 ms, as a loading state is, then
 * the searching line, and after 10 s with nothing found what to check. The browse goes on, so a
 * TV that turns up later still takes the row's place.
 */
@Composable
private fun TvSearchingRow(request: TvPlayRequest?) {
    var stage by remember { mutableStateOf(TvSearch.QUIET) }
    val context = LocalContext.current
    LaunchedEffect(Unit) {
        delay(PLACEHOLDERS_AFTER)
        stage = TvSearch.LOOKING
        delay(SEARCH_GIVES_UP - PLACEHOLDERS_AFTER)
        stage = TvSearch.NONE
    }
    Box(Modifier.heightIn(min = 72.dp)) {
        Crossfade(stage, animationSpec = Motion.fadeIn(), label = "searching") { shown -> // i18n-skip
            when (shown) {
                TvSearch.QUIET -> Spacer(Modifier.fillMaxWidth().height(72.dp))
                TvSearch.LOOKING -> TvSheetRow(
                    stringResource(TvWords.EMPTY),
                    null,
                    modifier = Modifier.heightIn(min = 72.dp),
                    leading = { CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) },
                )
                TvSearch.NONE -> TvSheetRow(stringResource(TvWords.NONE_FOUND), TvWords.noneLine(context, request))
            }
        }
    }
}

/** How long the picker looks before it says no TV answered. */
private const val SEARCH_GIVES_UP = 10_000L

/** A row of the picker: the TV, what the engine says under it, and what choosing it does. */
@Composable
private fun TvPickerRow(row: FfiTvRow, chosen: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val usable = row.kind != FfiTvRowKind.NEEDS_UPDATE
    TvSheetRow(
        row.name,
        row.detail,
        modifier = Modifier.alpha(if (usable) 1f else 0.38f),
        trailing = {
            when {
                row.kind == FfiTvRowKind.NEW -> Text(stringResource(R.string.tv_pair), style = MaterialTheme.typography.labelLarge, color = colors.onSurface)
                chosen -> Icon(Icons.Check, contentDescription = stringResource(R.string.tv_status_connected), tint = colors.onSurface)
                usable -> Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant)
            }
        },
        onClick = if (usable) onClick else null,
    )
}

// Pairing

@Composable
private fun TvPairing(tv: TvRemoteModel, step: TvPairingStep) {
    val colors = MaterialTheme.colorScheme
    var code by rememberSaveable { mutableStateOf("") }
    val typing = step.needsCode && step.tvName.isNotEmpty()
    LaunchedEffect(step.problem) { if (step.problem != null) code = "" }
    SheetTitle(stringResource(R.string.tv_pair_with_tv))
    if (typing) {
        SheetText(TvWords.pairingLine(LocalContext.current, step.tvName))
        TvHeading(stringResource(R.string.tv_sheet_code_on, step.tvName))
        TvCodeBoxes(code, { code = it }) { digits -> if (!step.working) tv.submit(digits) }
        step.problem?.let { ProblemLine(it) }
        SheetButtons {
            QuietButton(stringResource(R.string.action_back), onClick = tv::cancelPairing)
            QuietButton(stringResource(R.string.action_new_code), enabled = !step.working) {
                code = ""
                tv.requestCodeAgain()
            }
            Button(onClick = { tv.submit(code) }, enabled = code.length == 6 && !step.working) {
                Text(stringResource(if (step.working) R.string.tv_sheet_pairing_button else R.string.tv_pair))
            }
        }
    } else {
        TvSheetRow(
            when {
                step.tvName.isNotEmpty() -> stringResource(R.string.tv_pairing_with, step.tvName)
                // Nothing is connecting once there is a problem to read.
                step.problem != null && !step.working -> stringResource(R.string.tv_sheet_title_fallback)
                else -> stringResource(R.string.tv_sheet_connecting)
            },
            step.problem,
            leading = { if (step.working) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) },
        )
        SheetButtons {
            QuietButton(stringResource(R.string.action_back), onClick = tv::cancelPairing)
        }
    }
}

/** Six digits in boxes, typed into one field under them, as the code reads on the TV. */
@Composable
private fun TvCodeBoxes(code: String, onChange: (String) -> Unit, submit: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    val typeTheCode = stringResource(R.string.tv_sheet_type_code)
    val pairingCode = stringResource(R.string.tv_pairing_code)
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Box(Modifier.padding(top = 4.dp).clickable(onClickLabel = typeTheCode) { runCatching { focus.requestFocus() } }) {
        BasicTextField(
            value = code,
            onValueChange = { typed ->
                val kept = typed.filter(Char::isDigit).take(6)
                onChange(kept)
                if (kept.length == 6) submit(kept)
            },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (code.length == 6) submit(code) }),
            singleLine = true,
            modifier = Modifier
                .size(1.dp)
                .alpha(0f)
                .focusRequester(focus)
                .onFocusChanged { focused = it.isFocused }
                .semantics { contentDescription = pairingCode },
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            for (index in 0 until 6) {
                val digit = code.getOrNull(index)?.toString().orEmpty()
                val current = index == code.length && focused
                Box(
                    Modifier
                        .size(44.dp, 56.dp)
                        .background(colors.surfaceContainerHighest, MaterialTheme.shapes.small)
                        .border(if (current) 2.dp else 1.dp, if (current) colors.onSurface else colors.outlineVariant, MaterialTheme.shapes.small),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(digit, style = TextStyle(fontSize = 28.sp, lineHeight = 34.sp, fontWeight = FontWeight.SemiBold).tabular(), color = colors.onSurface)
                }
                if (index == 2) Spacer(Modifier.width(8.dp))
            }
        }
    }
}

// The card

@Composable
private fun TvRemote(tv: TvRemoteModel) {
    val colors = MaterialTheme.colorScheme
    val held by tv.card.collectAsStateWithLifecycle()
    val starting by tv.starting.collectAsStateWithLifecycle()
    val problem by tv.problem.collectAsStateWithLifecycle()
    val card = held?.card
    val now = TvRemoteModel.shown(held, starting)
    // While the link connects or tries again, the card shows the TV as it last was, and nothing on it reaches the TV.
    val live = held?.live == true
    var search by rememberSaveable { mutableStateOf("") }
    val context = LocalContext.current
    SheetTitle(card?.tvName ?: stringResource(R.string.tv_sheet_title_fallback))
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        StateDot(phaseDot(card?.phase))
        SheetText(TvWords.status(context, card))
    }
    Spacer(Modifier.height(16.dp))
    if (now != null) {
        TvNowCard(tv, now, held, live)
        now.skip?.let { skip ->
            FilledTonalButton(onClick = { tv.send(FfiTvCommand.Skip) }, enabled = live, modifier = Modifier.padding(top = 8.dp)) { Text(skip.label) }
        }
        TvDivider(Modifier.padding(top = 16.dp))
        TvTrackRow(stringResource(R.string.player_subtitles), FfiTvTrackKind.SUBTITLE, now.subtitles, now.subtitlesOff, tv, enabled = live)
        TvDivider()
        TvTrackRow(stringResource(R.string.player_audio), FfiTvTrackKind.AUDIO, now.audio, off = false, tv, enabled = live)
        TvDivider()
        TvSheetRow(
            stringResource(R.string.tv_sheet_volume),
            TvWords.volume(context, now),
            modifier = Modifier.alpha(if (live) 1f else 0.38f),
            trailing = {
                Row {
                    TvGlyph(Icons.VolumeDown, stringResource(R.string.tv_sheet_volume_down), enabled = live) { tv.send(FfiTvCommand.Volume(-TvRemoteModel.VOLUME_STEP)) }
                    TvGlyph(Icons.VolumeUp, stringResource(R.string.tv_sheet_volume_up), enabled = live) { tv.send(FfiTvCommand.Volume(TvRemoteModel.VOLUME_STEP)) }
                }
            },
        )
    } else {
        // What to do from here only once the TV can be reached: the line under the title says why not.
        TvSheetRow(if (live) stringResource(R.string.tv_sheet_nothing_playing) else card?.tvName ?: stringResource(R.string.tv_sheet_title_fallback), if (live) stringResource(TvWords.NOTHING) else null)
    }
    TvHeading(stringResource(R.string.tv_search_on_tv))
    SheetField(
        search,
        {
            search = it
            tv.send(FfiTvCommand.Text(it, submit = false))
        },
        stringResource(R.string.tv_type_search),
        keyboard = KeyboardOptions(imeAction = ImeAction.Search, autoCorrectEnabled = false),
        actions = KeyboardActions(onSearch = { tv.send(FfiTvCommand.Text(search, submit = true)) }),
        enabled = live,
    )
    problem?.let { ProblemLine(it) }
    SheetButtons {
        if (TvWords.lost(card)) {
            QuietButton(stringResource(R.string.tv_pair_again)) {
                tv.connected.value?.let(tv::forget)
                tv.openPicker()
            }
        } else {
            QuietButton(stringResource(R.string.tv_choose_another), onClick = tv::openPicker)
        }
        QuietButton(stringResource(R.string.action_disconnect)) {
            tv.disconnect()
            tv.sheetClosed()
        }
    }
}

/** What the TV plays: the cover, the title and episode, the seek line, and the transport. */
@Composable
private fun TvNowCard(tv: TvRemoteModel, now: FfiNowCard, held: TvCard?, live: Boolean) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val entry = store.titles[now.id]
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        entry?.let { CoverArt(it, 40.dp, Modifier.size(40.dp, 60.dp)) }
        Column(Modifier.weight(1f)) {
            Text(now.title, style = ShowTitle.row, color = colors.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                if (now.loading) stringResource(R.string.tv_starting) else now.episodeLine,
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
    TvSeekLine(tv, now, held?.card?.rev, live)
    val usable = live && !now.loading
    Row(
        Modifier.fillMaxWidth().alpha(if (usable) 1f else 0.38f),
        horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TvStep(Icons.Replay, stringResource(R.string.player_back_10), enabled = usable) { tv.send(FfiTvCommand.Seek(-10.0)) }
        TvGlyph(if (now.paused) Icons.PlayRow else Icons.Pause, stringResource(if (now.paused) R.string.action_play else R.string.action_pause), size = 32.dp, enabled = usable) {
            tv.send(FfiTvCommand.TogglePause)
        }
        TvStep(Icons.Ahead, stringResource(R.string.player_forward_10), enabled = usable) { tv.send(FfiTvCommand.Seek(10.0)) }
        TvGlyph(Icons.SkipNext, stringResource(R.string.player_next_episode), enabled = usable) { tv.send(FfiTvCommand.Next) }
    }
}

/**
 * The TV's playhead on the Expressive slider, amber up to the handle; dragged, it seeks on release,
 * and holds where it was let go until the TV's next card says where it is (or 3 s pass), so the
 * handle does not jump back while the command travels.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TvSeekLine(tv: TvRemoteModel, now: FfiNowCard, rev: ULong?, live: Boolean) {
    val colors = MaterialTheme.colorScheme
    val duration = now.duration?.takeIf { it > 0 } ?: 0.0
    // A tick a second while the TV plays, for the line to move on the phone's own clock.
    val position by produceState(tv.playhead() ?: now.position, now) {
        while (true) {
            value = tv.playhead() ?: now.position
            if (now.paused || now.loading) break
            delay(1000)
        }
    }
    var dragging by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    var sent by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    LaunchedEffect(rev) { sent = null }
    LaunchedEffect(sent) {
        if (sent != null) {
            delay(SEEK_HOLD)
            sent = null
        }
    }
    val shown = ((dragging ?: sent)?.toDouble() ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    val time = MaterialTheme.typography.labelMedium.tabular()
    val positionLabel = stringResource(R.string.tv_sheet_position)
    val positionState = stringResource(R.string.tv_sheet_position_state, TvWords.clock(shown), TvWords.clock(duration))
    Column(Modifier.padding(top = 8.dp)) {
        Slider(
            value = shown.toFloat(),
            onValueChange = { dragging = it },
            onValueChangeFinished = {
                dragging?.let {
                    tv.send(FfiTvCommand.SeekTo(it.toDouble()))
                    sent = it
                }
                dragging = null
            },
            valueRange = 0f..maxOf(duration.toFloat(), 1f),
            enabled = duration > 0 && !now.loading && live,
            colors = SliderDefaults.colors(
                thumbColor = colors.primary,
                activeTrackColor = colors.primary,
                inactiveTrackColor = ObiSilk.Rule,
                disabledInactiveTrackColor = ObiSilk.Rule,
            ),
            modifier = Modifier.fillMaxWidth().semantics {
                contentDescription = positionLabel
                stateDescription = positionState
            },
        )
        Row(Modifier.fillMaxWidth()) {
            Text(TvWords.clock(shown), style = time, color = colors.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            // The time left, as the player shows it.
            Text(if (duration > 0) "−" + TvWords.clock(duration - shown) else "", style = time, color = colors.onSurfaceVariant, textAlign = TextAlign.End)
        }
    }
}

/** How long a let-go seek holds its place with no word from the TV. */
private const val SEEK_HOLD = 3_000L

/** A silk glyph in a 48 target, never amber. */
@Composable
private fun TvGlyph(icon: ImageVector, label: String, size: androidx.compose.ui.unit.Dp = 24.dp, enabled: Boolean = true, onClick: () -> Unit) {
    IconButton(onClick = onClick, enabled = enabled, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(size))
    }
}

/** Back or forward 10 s: the turning arrow at 28 with its 10 inside, as the player's. */
@Composable
private fun TvStep(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    val silk = MaterialTheme.colorScheme.onSurface
    IconButton(onClick = onClick, enabled = enabled, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
        Box(Modifier.size(28.dp)) {
            Icon(icon, contentDescription = null, tint = silk, modifier = Modifier.size(28.dp))
            Text(
                "10",
                style = TextStyle(fontSize = 8.5.sp, lineHeight = 10.sp, fontWeight = FontWeight.Bold),
                color = silk,
                modifier = Modifier.align(Alignment.Center).offset(y = 2.dp),
            )
        }
    }
}

/** The subtitle or audio choice: its value under its name, a menu of the tracks, the one on checked. */
@Composable
private fun TvTrackRow(title: String, kind: FfiTvTrackKind, tracks: List<FfiTvTrack>, off: Boolean, tv: TvRemoteModel, enabled: Boolean = true) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Box {
        TvSheetRow(
            title,
            TvWords.chosen(LocalContext.current, tracks, off),
            modifier = Modifier.alpha(if (enabled) 1f else 0.38f),
            trailing = { Icon(Icons.DropDown, contentDescription = null, tint = colors.onSurfaceVariant) },
            onClick = if (tracks.isEmpty() || !enabled) null else ({ open = true }),
        )
        Menu(expanded = open, onDismissRequest = { open = false }) {
            val context = LocalContext.current
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
                        tv.send(FfiTvCommand.Track(kind, track?.id))
                    },
                )
            }
        }
    }
}

// Parts of the sheets

/** A heading over a stretch of the sheet: titleMedium in haze. */
@Composable
private fun TvHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 24.dp, bottom = 4.dp).semantics { heading() },
    )
}

@Composable
private fun TvDivider(modifier: Modifier = Modifier) {
    HorizontalDivider(modifier, color = MaterialTheme.colorScheme.outlineVariant)
}

/** A row flat on the sheet: its name in bodyLarge, what it says under it in haze, 56 high and more. */
@Composable
private fun TvSheetRow(
    title: String,
    value: String?,
    modifier: Modifier = Modifier,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = if (value != null) 72.dp else 56.dp)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(vertical = 8.dp)
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        leading?.let { Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) { it() } }
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
            value?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
        trailing?.invoke()
    }
}

// The way back to the card

/**
 * What the TV plays, over the navigation bar while a TV is driven and its card is closed: the
 * cover, the title, the TV's name, and play or pause, on surfaceContainerHigh with the playhead in
 * amber along its top. A tap opens the card.
 */
@Composable
internal fun TvNowPlayingBar(modifier: Modifier = Modifier) {
    val tv = LocalAppStore.current.tv
    val held by tv.card.collectAsStateWithLifecycle()
    val starting by tv.starting.collectAsStateWithLifecycle()
    val sheet by tv.sheet.collectAsStateWithLifecycle()
    val connected by tv.connected.collectAsStateWithLifecycle()
    val now = TvRemoteModel.shown(held, starting)
    val card = held?.card
    val visible = card != null && now != null && connected != null && sheet == null
    // What the bar showed last, for it to leave with rather than go blank.
    val last = remember { LastShown() }
    val shown = if (visible) card!! to now!! else last.value
    SideEffect { if (visible) last.value = shown }
    AnimatedVisibility(
        visible,
        modifier = modifier,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
    ) {
        shown?.let { (shownCard, shownNow) -> TvNowPlayingBarContent(tv, shownCard, shownNow) }
    }
}

/** The card and episode the bar last showed, kept out of the snapshot: it is only read as the bar leaves. */
private class LastShown {
    var value: Pair<FfiRemoteCard, FfiNowCard>? = null
}

@Composable
private fun TvNowPlayingBarContent(tv: TvRemoteModel, card: FfiRemoteCard, now: FfiNowCard) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val duration = now.duration?.takeIf { it > 0 }
    val live = card.phase == FfiLinkPhase.CONNECTED
    val position by produceState(tv.playhead() ?: now.position, now) {
        while (true) {
            value = tv.playhead() ?: now.position
            if (now.paused || now.loading) break
            delay(1000)
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .background(colors.surfaceContainerHigh)
            .clickable(onClickLabel = stringResource(R.string.tv_show_remote, card.tvName)) { tv.openRemote() },
    ) {
        Box(Modifier.fillMaxWidth().height(2.dp).background(ObiSilk.Rule)) {
            if (duration != null) {
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
                    if (live) stringResource(R.string.tv_status_playing, card.tvName) else TvWords.status(LocalContext.current, card),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            val usable = live && !now.loading
            Box(Modifier.alpha(if (usable) 1f else 0.38f)) {
                TvGlyph(if (now.paused) Icons.PlayRow else Icons.Pause, stringResource(if (now.paused) R.string.action_play else R.string.action_pause), enabled = usable) {
                    tv.send(FfiTvCommand.TogglePause)
                }
            }
        }
    }
}

// Settings

/**
 * The TVs this phone paired with, each with Forget, and Pair a TV; under them, the code a TV lists
 * this phone with. Nothing where the engine has no remote.
 */
@Composable
internal fun TvSettingsGroup() {
    val tv = LocalAppStore.current.tv
    val paired by tv.paired.collectAsStateWithLifecycle()
    val connected by tv.connected.collectAsStateWithLifecycle()
    if (!tv.available) return
    SettingsGroup(
        title = stringResource(R.string.tv_play_on),
        footer = stringResource(R.string.tv_sheet_footer, tv.shortFingerprint),
        rows = paired.map { paired ->
            @Composable { place: Place ->
                InfoRow(
                    place,
                    paired.name,
                    stringResource(if (paired.fingerprint == connected) R.string.tv_status_connected else R.string.tv_paired),
                    trailing = { QuietButton(stringResource(R.string.action_forget)) { tv.forget(paired.fingerprint) } },
                )
            }
        } + listOf(@Composable { place: Place -> ActionRow(place, stringResource(R.string.tv_pair_action), onClick = tv::openPicker) }),
    )
}

/** A problem under a field: the error's dot, then its words in the surface's own colour, never vermilion text. */
@Composable
private fun ProblemLine(text: String) {
    val style = MaterialTheme.typography.bodyMedium
    // The dot sits level with the first line, however many lines the words wrap to.
    val line = with(LocalDensity.current) { style.lineHeight.toDp() }
    Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.height(line), contentAlignment = Alignment.Center) {
            StateDot(MaterialTheme.colorScheme.error)
        }
        Text(text, style = style, color = MaterialTheme.colorScheme.onSurface)
    }
}
