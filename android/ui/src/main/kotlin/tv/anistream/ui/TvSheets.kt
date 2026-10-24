// Play on TV: the sheet that chooses a TV and pairs with it, the now-playing card that drives it,
// the bar that leads back to the card, and the paired TVs in Settings. The engine's sender says
// what the rows, the pairing's failures and the card read (TvRemoteModel); these only lay them
// out, as the app's other sheets are: SettingsSheet on surfaceContainer, flat rows with haze
// headings, the one filled amber action at the foot.

package tv.anistream.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.provider.MediaStore
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
import tv.anistream.store.TvPairingStep
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.TvRemoteModel

/** What the TV sheets say. */
internal object TvWords {
    fun pickerTitle(request: TvPlayRequest?): String = if (request == null) "Pair a TV" else "Play on TV"

    fun pickerLine(request: TvPlayRequest?, device: String): String = when {
        request == null -> "Open anistream on the TV. It needs to be on the same Wi-Fi as this $device."
        request.title.isEmpty() -> "Episode ${request.episode}"
        else -> "${request.title}, episode ${request.episode}"
    }

    fun pairingLine(name: String, device: String): String =
        "Type the six digits $name shows. After this, this $device connects by itself."

    const val EMPTY = "Looking for TVs on this network"
    const val ADDRESS = "The TV shows its address on its Pair a phone screen."
    const val SCAN = "Point the camera app at the code on the TV. It opens anistream and pairs."
    const val NOTHING = "Choose Play on TV on an episode, or search on the TV from here."

    /** A track as the card names it: its language in this phone's language, and its title. */
    fun track(track: FfiTvTrack): String {
        val language = track.language?.takeIf { it.isNotBlank() }
            ?.let { Locale.forLanguageTag(it).getDisplayLanguage(Locale.getDefault()).takeIf(String::isNotBlank) }
        val title = track.title?.takeIf { it.isNotBlank() }
        return when {
            language != null && title != null -> "$language · $title"
            language != null -> language
            title != null -> title
            else -> "Track ${track.id}"
        }
    }

    fun chosen(tracks: List<FfiTvTrack>, off: Boolean): String =
        if (off) "Off" else tracks.firstOrNull { it.selected }?.let(::track) ?: "None"

    fun volume(now: FfiNowCard): String? = now.volume?.let { "${Math.round(it)} of 100" }

    fun status(card: FfiRemoteCard?): String = card?.phaseLine ?: "Connected"

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

/** "phone" or "tablet": what this device is, as the sheets name it. */
@Composable
internal fun deviceWord(): String = if (LocalConfiguration.current.smallestScreenWidthDp >= 600) "tablet" else "phone"

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
    SheetTitle(TvWords.pickerTitle(request))
    SheetText(TvWords.pickerLine(request, deviceWord()))
    TvHeading("On this network")
    if (rows.isEmpty()) {
        TvSheetRow(TvWords.EMPTY, null, leading = { CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) })
    }
    rows.forEachIndexed { index, row ->
        if (index > 0) TvDivider()
        TvPickerRow(row, chosen = row.fingerprint != null && row.fingerprint == connected) { tv.choose(row) }
    }
    TvHeading("Not listed")
    TvSheetRow(
        "Scan the code on the TV",
        TvWords.SCAN,
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
        "Address",
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
        actions = KeyboardActions(onGo = { address.trim().takeIf(String::isNotEmpty)?.let(tv::begin) }),
        supporting = TvWords.ADDRESS,
    )
    SheetButtons {
        QuietButton("Close", onClick = tv::sheetClosed)
    }
}

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
                row.kind == FfiTvRowKind.NEW -> Text("Pair", style = MaterialTheme.typography.labelLarge, color = colors.onSurface)
                chosen -> Icon(Icons.Check, contentDescription = "Connected", tint = colors.onSurface)
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
    SheetTitle("Pair with a TV")
    if (typing) {
        SheetText(TvWords.pairingLine(step.tvName, deviceWord()))
        TvHeading("Code on ${step.tvName}")
        TvCodeBoxes(code, { code = it }) { digits -> if (!step.working) tv.submit(digits) }
        step.problem?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.error, modifier = Modifier.padding(top = 8.dp)) }
        SheetButtons {
            QuietButton("Back", onClick = tv::cancelPairing)
            QuietButton("Show a new code", enabled = !step.working) {
                code = ""
                tv.requestCodeAgain()
            }
            Button(onClick = { tv.submit(code) }, enabled = code.length == 6 && !step.working) {
                Text(if (step.working) "Pairing" else "Pair")
            }
        }
    } else {
        TvSheetRow(
            if (step.tvName.isEmpty()) "Connecting to the TV" else "Pairing with ${step.tvName}",
            step.problem,
            leading = { if (step.working) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) },
        )
        SheetButtons {
            QuietButton("Back", onClick = tv::cancelPairing)
        }
    }
}

/** Six digits in boxes, typed into one field under them, as the code reads on the TV. */
@Composable
private fun TvCodeBoxes(code: String, onChange: (String) -> Unit, submit: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Box(Modifier.padding(top = 4.dp).clickable(onClickLabel = "Type the code") { runCatching { focus.requestFocus() } }) {
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
                .semantics { contentDescription = "Pairing code" },
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
    val problem by tv.problem.collectAsStateWithLifecycle()
    val card = held?.card
    val now = card?.now
    var search by rememberSaveable { mutableStateOf("") }
    SheetTitle(card?.tvName ?: "TV")
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        StateDot(phaseDot(card?.phase))
        SheetText(TvWords.status(card))
    }
    Spacer(Modifier.height(16.dp))
    if (now != null) {
        TvNowCard(tv, now)
        now.skip?.let { skip ->
            FilledTonalButton(onClick = { tv.send(FfiTvCommand.Skip) }, modifier = Modifier.padding(top = 8.dp)) { Text(skip.label) }
        }
        TvDivider(Modifier.padding(top = 16.dp))
        TvTrackRow("Subtitles", FfiTvTrackKind.SUBTITLE, now.subtitles, now.subtitlesOff, tv)
        TvDivider()
        TvTrackRow("Audio", FfiTvTrackKind.AUDIO, now.audio, off = false, tv)
        TvDivider()
        TvSheetRow(
            "TV volume",
            TvWords.volume(now),
            trailing = {
                Row {
                    TvGlyph(Icons.VolumeDown, "Volume down") { tv.send(FfiTvCommand.Volume(-TvRemoteModel.VOLUME_STEP)) }
                    TvGlyph(Icons.VolumeUp, "Volume up") { tv.send(FfiTvCommand.Volume(TvRemoteModel.VOLUME_STEP)) }
                }
            },
        )
    } else {
        TvSheetRow(if (card?.phase == FfiLinkPhase.CONNECTED) "Nothing playing" else card?.tvName ?: "TV", TvWords.NOTHING)
    }
    TvHeading("Search on the TV")
    SheetField(
        search,
        {
            search = it
            tv.send(FfiTvCommand.Text(it, submit = false))
        },
        "Type a search",
        keyboard = KeyboardOptions(imeAction = ImeAction.Search, autoCorrectEnabled = false),
        actions = KeyboardActions(onSearch = { tv.send(FfiTvCommand.Text(search, submit = true)) }),
    )
    problem?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.error, modifier = Modifier.padding(top = 8.dp)) }
    SheetButtons {
        if (TvWords.lost(card)) {
            QuietButton("Pair again") {
                tv.connected.value?.let(tv::forget)
                tv.openPicker()
            }
        } else {
            QuietButton("Choose another TV", onClick = tv::openPicker)
        }
        QuietButton("Disconnect") {
            tv.disconnect()
            tv.sheetClosed()
        }
    }
}

/** What the TV plays: the cover, the title and episode, the seek line, and the transport. */
@Composable
private fun TvNowCard(tv: TvRemoteModel, now: FfiNowCard) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val entry = store.titles[now.id]
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        entry?.let { CoverArt(it, 40.dp, Modifier.size(40.dp, 60.dp)) }
        Column(Modifier.weight(1f)) {
            Text(now.title, style = ShowTitle.row, color = colors.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                if (now.loading) "Starting on the TV" else now.episodeLine,
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
    TvSeekLine(tv, now)
    Row(
        Modifier.fillMaxWidth().alpha(if (now.loading) 0.38f else 1f),
        horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TvStep(Icons.Replay, "Back 10 seconds", enabled = !now.loading) { tv.send(FfiTvCommand.Seek(-10.0)) }
        TvGlyph(if (now.paused) Icons.PlayRow else Icons.Pause, if (now.paused) "Play" else "Pause", size = 32.dp, enabled = !now.loading) {
            tv.send(FfiTvCommand.TogglePause)
        }
        TvStep(Icons.Ahead, "Forward 10 seconds", enabled = !now.loading) { tv.send(FfiTvCommand.Seek(10.0)) }
        TvGlyph(Icons.SkipNext, "Next episode", enabled = !now.loading) { tv.send(FfiTvCommand.Next) }
    }
}

/** The TV's playhead on the Expressive slider, amber up to the handle; dragged, it seeks on release. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TvSeekLine(tv: TvRemoteModel, now: FfiNowCard) {
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
    val shown = (dragging?.toDouble() ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    val time = MaterialTheme.typography.labelMedium.tabular()
    Column(Modifier.padding(top = 8.dp)) {
        Slider(
            value = shown.toFloat(),
            onValueChange = { dragging = it },
            onValueChangeFinished = {
                dragging?.let { tv.send(FfiTvCommand.SeekTo(it.toDouble())) }
                dragging = null
            },
            valueRange = 0f..maxOf(duration.toFloat(), 1f),
            enabled = duration > 0 && !now.loading,
            colors = SliderDefaults.colors(
                thumbColor = colors.primary,
                activeTrackColor = colors.primary,
                inactiveTrackColor = ObiSilk.Rule,
                disabledInactiveTrackColor = ObiSilk.Rule,
            ),
            modifier = Modifier.fillMaxWidth().semantics {
                contentDescription = "Position on the TV"
                stateDescription = "${TvWords.clock(shown)} of ${TvWords.clock(duration)}"
            },
        )
        Row(Modifier.fillMaxWidth()) {
            Text(TvWords.clock(shown), style = time, color = colors.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            Text(if (duration > 0) TvWords.clock(duration) else "", style = time, color = colors.onSurfaceVariant, textAlign = TextAlign.End)
        }
    }
}

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
private fun TvTrackRow(title: String, kind: FfiTvTrackKind, tracks: List<FfiTvTrack>, off: Boolean, tv: TvRemoteModel) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Box {
        TvSheetRow(
            title,
            TvWords.chosen(tracks, off),
            trailing = { Icon(Icons.DropDown, contentDescription = null, tint = colors.onSurfaceVariant) },
            onClick = if (tracks.isEmpty()) null else ({ open = true }),
        )
        Menu(expanded = open, onDismissRequest = { open = false }) {
            val choices = buildList {
                if (kind == FfiTvTrackKind.SUBTITLE) add(null to "Off")
                tracks.forEach { add(it to TvWords.track(it)) }
            }
            for ((track, label) in choices) {
                val picked = if (track == null) off else !off && track.selected
                DropdownMenuItem(
                    text = { Text(label, style = MaterialTheme.typography.labelLarge) },
                    leadingIcon = {
                        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                            if (picked) Icon(Icons.Check, contentDescription = null, tint = colors.onSurface)
                        }
                    },
                    modifier = Modifier.heightIn(min = 48.dp).semantics { if (picked) stateDescription = "Selected" },
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
    val store = LocalAppStore.current
    val tv = store.tv
    val held by tv.card.collectAsStateWithLifecycle()
    val sheet by tv.sheet.collectAsStateWithLifecycle()
    val connected by tv.connected.collectAsStateWithLifecycle()
    val card = held?.card ?: return
    val now = card.now ?: return
    if (connected == null || sheet != null) return
    val colors = MaterialTheme.colorScheme
    val duration = now.duration?.takeIf { it > 0 }
    val position by produceState(tv.playhead() ?: now.position, now) {
        while (true) {
            value = tv.playhead() ?: now.position
            if (now.paused || now.loading) break
            delay(1000)
        }
    }
    Column(
        modifier
            .fillMaxWidth()
            .background(colors.surfaceContainerHigh)
            .clickable(onClickLabel = "Show the remote") { tv.openRemote() },
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
                    if (card.phase == FfiLinkPhase.CONNECTED) "Playing on ${card.tvName}" else TvWords.status(card),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            TvGlyph(if (now.paused) Icons.PlayRow else Icons.Pause, if (now.paused) "Play" else "Pause", enabled = !now.loading) {
                tv.send(FfiTvCommand.TogglePause)
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
    val device = deviceWord()
    SettingsGroup(
        title = "Play on TV",
        footer = "A TV lists this $device with the code ${tv.shortFingerprint}. Forget a TV here and it forgets this $device too.",
        rows = paired.map { paired ->
            @Composable { place: Place ->
                InfoRow(
                    place,
                    paired.name,
                    if (paired.fingerprint == connected) "Connected" else "Paired",
                    trailing = { QuietButton("Forget") { tv.forget(paired.fingerprint) } },
                )
            }
        } + listOf(@Composable { place: Place -> ActionRow(place, "Pair a TV", onClick = tv::openPicker) }),
    )
}
