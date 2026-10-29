// The player's menus as modal bottom sheets (T7): subtitles and audio, with the picture's
// upscaling, night mode and speed; the episodes, with the sources; and the releases to choose
// from when a start finds nothing that plays.

package tv.anistream.player.ui

import java.text.NumberFormat
import androidx.annotation.StringRes
import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import java.util.Locale
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.PlayerChoice
import tv.anistream.player.PlayerModel

/** Which of the player's sheets is open. */
enum class PlayerSheet { Tracks, Episodes, Sources }

/** The speeds the Speed list offers. */
private val SPEEDS = listOf(0.5, 0.75, 1.0, 1.25, 1.5, 2.0)

/** "1.0×", "1.25×". */
internal fun speedLabel(speed: Double): String {
    val digits = NumberFormat.getNumberInstance().apply { minimumFractionDigits = 1; maximumFractionDigits = 2 }
    return digits.format(speed) + "×" // i18n-skip
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PlayerSheets(model: PlayerModel, sheet: PlayerSheet?, onDismiss: () -> Unit) {
    if (sheet == null) return
    // The controls stay up while a sheet is open.
    DisposableEffect(sheet) {
        model.hold("sheet", true)
        onDispose { model.hold("sheet", false) }
    }
    ModalBottomSheet(
        onDismissRequest = {
            if (sheet == PlayerSheet.Sources) model.cancelChoosingSource()
            onDismiss()
        },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false),
        containerColor = ObiSilk.Raised,
        scrimColor = Color.Black.copy(alpha = 0.32f),
        dragHandle = { BottomSheetDefaults.DragHandle(color = ObiSilk.Haze.copy(alpha = 0.4f)) },
    ) {
        ImmersiveSheet()
        // The Episodes sheet opens on the episode playing, the Tracks sheet on the subtitles in use.
        val current = when (sheet) {
            PlayerSheet.Episodes -> model.menus?.episodes?.indexOfFirst { it.current }?.takeIf { it > 1 } ?: 0
            PlayerSheet.Tracks -> tracksOpeningRow(model.subtitleTracks.map { it.id }, model.subtitleTrack)
            PlayerSheet.Sources -> 0
        }
        LazyColumn(state = rememberLazyListState(initialFirstVisibleItemIndex = current), contentPadding = PaddingValues(bottom = 24.dp)) {
            when (sheet) {
                PlayerSheet.Tracks -> tracks(model)
                PlayerSheet.Episodes -> episodes(model)
                PlayerSheet.Sources -> sources(model, onDismiss)
            }
        }
    }
}

/** The sheet's own window keeps the system bars hidden, as the player's does. */
@Composable
private fun ImmersiveSheet() {
    val view = LocalView.current
    DisposableEffect(view) {
        val window = generateSequence(view.parent) { it.parent }.filterIsInstance<DialogWindowProvider>().firstOrNull()?.window
        if (window != null) {
            WindowCompat.getInsetsController(window, view).apply {
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
        }
        onDispose {}
    }
}

/**
 * The row the Tracks sheet opens on: the one above the subtitles in use, so a list of many
 * tracks shows it. The sheet's title, the Subtitles heading and Off come before the tracks.
 */
internal fun tracksOpeningRow(subtitles: List<Long>, current: Long?): Int {
    val inUse = current?.let { id -> subtitles.indexOf(id).takeIf { it >= 0 }?.let { it + 3 } } ?: 2
    // Near the top, the top shows it already, with the sheet's title.
    return if (inUse > 5) inUse - 1 else 0
}

private fun LazyListScope.tracks(model: PlayerModel) {
    item(key = "title") {
        Text(
            stringResource(R.string.player_subtitles_audio_picture),
            style = MaterialTheme.typography.titleLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 8.dp),
        )
    }
    heading(R.string.player_subtitles)
    item(key = "sub-off") { Choice(stringResource(R.string.value_off), checked = model.subtitleTrack == null) { model.selectSubtitle(null) } }
    items(model.subtitleTracks, key = { "sub-${it.id}" }) { track ->
        Choice(track.name(LocalContext.current), checked = model.subtitleTrack == track.id) { model.selectSubtitle(track.id) }
    }
    divider("tracks-audio")
    heading(R.string.player_audio)
    items(model.audioTracks, key = { "audio-${it.id}" }) { track ->
        Choice(track.name(LocalContext.current), checked = model.audioTrack == track.id) { model.selectAudio(track.id) }
    }
    item(key = "remembered") {
        Text(
            stringResource(R.string.player_remembered),
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Haze,
            modifier = Modifier.padding(start = 56.dp, top = 4.dp, bottom = 8.dp),
        )
    }
    val menus = model.menus
    if (menus != null && menus.upscaling.options.isNotEmpty()) {
        divider("tracks-upscaling")
        heading(R.string.player_upscaling)
        items(menus.upscaling.options, key = { "look-${it.id}" }) { choice ->
            Choice(choice.label, detail = choice.hint, checked = menus.upscaling.current == choice.id) { model.setUpscaling(choice.id) }
        }
    }
    if (menus != null) {
        divider("tracks-night")
        item(key = "night") {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable { model.setNightMode(!menus.nightMode) }.padding(start = 56.dp, end = 24.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.player_night_mode), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
                    Text(stringResource(R.string.player_night_mode_detail), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                }
                Switch(
                    checked = menus.nightMode,
                    onCheckedChange = { model.setNightMode(it) },
                    colors = SwitchDefaults.colors(checkedTrackColor = ObiSilk.Amber, checkedThumbColor = ObiSilk.Ground),
                )
            }
        }
    }
    divider("tracks-speed")
    heading(R.string.player_speed)
    items(SPEEDS, key = { "speed-$it" }) { speed ->
        Choice(speedLabel(speed), checked = kotlin.math.abs(model.speed - speed) < 0.01) { model.playAt(speed) }
    }
}

private fun LazyListScope.episodes(model: PlayerModel) {
    val menus = model.menus
    heading(R.string.player_episodes)
    if (menus == null || menus.episodes.isEmpty()) {
        item(key = "no-episodes") { Note(stringResource(R.string.player_sheet_episodes_loading)) }
    } else {
        items(menus.episodes, key = { "ep-${it.number}" }) { row ->
            Choice(
                row.title?.let { "${row.label} · $it" } ?: row.label,
                detail = if (row.filler) stringResource(R.string.episode_filler) else if (row.watched) stringResource(R.string.episode_watched) else null,
                checked = row.current,
            ) { model.choose(PlayerChoice.Episode(row.number)) }
        }
    }
    if (menus != null && menus.sources.isNotEmpty()) {
        divider("episodes-sources")
        heading(R.string.player_sources)
        items(menus.sources, key = { "source-${it.id}" }) { source ->
            Choice(source.label, detail = source.health ?: source.detail, checked = source.current) {
                model.choose(PlayerChoice.Source(source.id))
            }
        }
    }
}

private fun LazyListScope.sources(model: PlayerModel, onDismiss: () -> Unit) {
    heading(R.string.player_sources)
    val choices = model.sourceChoices
    when {
        choices == null -> item(key = "finding") { Note(stringResource(R.string.player_sheet_finding)) }
        choices.isEmpty() -> item(key = "none") { Note(stringResource(R.string.player_sheet_none)) }
        else -> items(choices, key = { "${it.providerId}-${it.id}" }) { candidate ->
            val detail = listOfNotNull(
                candidate.quality?.let { "${it}p" }, // i18n-skip
                candidate.size,
                if (candidate.dubbed) stringResource(R.string.audio_dubbed) else null,
                if (candidate.dualAudio) stringResource(R.string.player_dual_audio) else null,
            ).joinToString(" · ").ifEmpty { null }
            Choice(candidate.title, detail = detail, checked = false) {
                model.play(candidate)
                onDismiss()
            }
        }
    }
}

private fun LazyListScope.heading(@StringRes title: Int) {
    item(key = "heading-$title") {
        Text(
            stringResource(title),
            style = MaterialTheme.typography.titleMedium,
            color = ObiSilk.Haze,
            modifier = Modifier.padding(start = 56.dp, top = 12.dp, bottom = 4.dp),
        )
    }
}

private fun LazyListScope.divider(key: String) {
    item(key = key) { HorizontalDivider(Modifier.padding(vertical = 8.dp), color = ObiSilk.FaintRule) }
}

@Composable
private fun Note(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 56.dp, top = 8.dp, bottom = 8.dp))
}

/** A row with the 24 dp check slot every row keeps, so the labels line up; the current one checked in silk. */
@Composable
private fun Choice(title: String, detail: String? = null, checked: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .semantics {
                selected = checked
                role = Role.RadioButton
            }
            .clickable(onClick = onClick)
            .padding(start = 16.dp, end = 24.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(24.dp)) {
            if (checked) Icon(Check, contentDescription = null, tint = ObiSilk.Silk)
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.Center) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
            detail?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
    }
}

private val Check: ImageVector = ImageVector.Builder("Check", 24.dp, 24.dp, 24f, 24f).apply {
    addPath(
        pathData = addPathNodes("M5 12.5 L10 17.5 L19 7"),
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 2f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    )
}.build()
