// An episode's row on a title page, and marking it watched (T9): the bar fills, the row dims, the
// check draws itself on, with Undo in a snackbar. When playback marks it, the row shows the result
// at once, with no motion and no haptic.

package tv.anistream.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.Stable
import androidx.compose.runtime.remember
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import tv.anistream.i18n.R
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.tabular
import tv.anistream.store.AirTime
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeKind
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.SettingsCatalog
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.ShowSettingKey
import androidx.compose.ui.platform.LocalContext

/**
 * The episodes the user marked or unmarked on this page, until their row has shown the change:
 * those animate; a change from anywhere else (playback) shows at once.
 */
@Stable
class UserMarks {
    private val pending = mutableSetOf<String>()

    fun mark(number: String) {
        pending += number
    }

    /** Whether [number]'s change was the user's, forgetting it. */
    fun take(number: String): Boolean = pending.remove(number)
}

/**
 * The page's marks. Dynamic rather than static: a season picked in place brings marks of its own,
 * and only the rows reading them recompose, not the whole page under them.
 */
val LocalUserMarks = compositionLocalOf { UserMarks() }

/** How faint an episode no source lists is drawn. */
private const val UNSOURCED_ALPHA = 0.55f

/** "24 min", from the row's duration. */
internal fun minutes(row: EpisodeRow): String? = row.durationSecs?.let { "${maxOf(1, (it.toDouble() / 60).roundToInt())} min" }

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun EpisodeLine(
    entry: Entry,
    row: EpisodeRow,
    current: Boolean,
    unaired: Boolean,
    hidden: Boolean,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val marks = LocalUserMarks.current
    val haptics = LocalHapticFeedback.current
    val resume = entry.resume?.takeIf { current }
    val fraction = resume?.fraction?.toFloat()
    val dim = remember { Animatable(if (row.completed) 1f else 0f) }
    val fill = remember { Animatable(if (row.completed) 1f else fraction ?: 0f) }
    val check = remember { Animatable(if (row.completed) 1f else 0f) }
    LaunchedEffect(row.completed) {
        val watched = row.completed
        if (!marks.take(row.number)) {
            dim.snapTo(if (watched) 1f else 0f)
            check.snapTo(if (watched) 1f else 0f)
            fill.snapTo(if (watched) 1f else fraction ?: 0f)
            return@LaunchedEffect
        }
        haptics.performHapticFeedback(if (watched) HapticFeedbackType.ToggleOn else HapticFeedbackType.ToggleOff)
        if (watched) {
            fill.animateTo(1f, Motion.standard())
            coroutineScope {
                launch { dim.animateTo(1f, Motion.quick()) }
                launch { check.animateTo(1f, tween(300, easing = FastOutSlowInEasing)) }
            }
        } else {
            coroutineScope {
                launch { dim.animateTo(0f, Motion.quick()) }
                launch { check.animateTo(0f, Motion.quick()) }
            }
            fill.animateTo(fraction ?: 0f, Motion.standard())
        }
    }
    val context = androidx.compose.ui.platform.LocalContext.current
    val airs = row.airsAt?.takeIf { unaired }?.let { airsWords(context, java.time.Instant.ofEpochSecond(it)) }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            // One no source lists is dimmed whole and plays nothing.
            .graphicsLayer { alpha = if (row.unsourced) UNSOURCED_ALPHA else 1f }
            // One still to air is no button: nothing plays from it.
            .then(
                if (unaired) {
                    Modifier.semantics(mergeDescendants = true) {
                        contentDescription = "Episode ${row.number}" + (row.title?.let { ", $it" } ?: "") +
                            (airs?.let { ". Airs $it, not out yet" } ?: ". Not out yet")
                    }
                } else {
                    Modifier
                        .combinedClickable(
                            // One no source has opens its options, which offer to add a source.
                            onClickLabel = if (row.metadataOnly || row.unsourced) "Episode options" else "Play",
                            onLongClickLabel = "Episode options",
                            onClick = onPlay,
                            onLongClick = onMenu,
                        )
                        // A keyboard opens the same menu with its menu key, or Shift and F10.
                        .onPreviewKeyEvent { event ->
                            val opens = event.key == Key.Menu || (event.key == Key.F10 && event.isShiftPressed)
                            if (opens && event.type == KeyEventType.KeyUp) onMenu()
                            opens
                        }
                },
            )
            .padding(start = 16.dp, end = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(96.dp, 54.dp).graphicsLayer { alpha = 1f - 0.3f * dim.value }) {
            EpisodeStill(entry, row, hidden = hidden, unaired = unaired)
        }
        Column(Modifier.weight(1f)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Bottom) {
                Text(
                    row.title ?: "Episode ${row.number}",
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (unaired) colors.onSurfaceVariant else lerp(colors.onSurface, colors.onSurfaceVariant, dim.value),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (row.kind == EpisodeKind.FILLER) {
                    Text(
                        "FILLER",
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.padding(bottom = 2.dp),
                    )
                }
            }
            val showsBar = (fraction != null || fill.isRunning) && !unaired && dim.value < 1f
            Row(Modifier.heightIn(min = 20.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (showsBar) {
                    LinearProgressIndicator(
                        progress = { fill.value },
                        modifier = Modifier.width(64.dp).height(4.dp),
                        color = colors.primary,
                        trackColor = colors.outline,
                        gapSize = 4.dp,
                        drawStopIndicator = {},
                    )
                }
                if (unaired) Box(Modifier.size(6.dp).background(colors.secondary, androidx.compose.foundation.shape.CircleShape))
                Text(
                    if (unaired) "E${row.number} \u00b7 " + (airs?.let { "Airs $it" } ?: "Not out yet")
                    else if (row.unsourced) "E${row.number} \u00b7 " + stringResource(R.string.episode_unsourced)
                    else episodeDetail(row, resume = resume.takeIf { !row.completed }, unaired = false),
                    style = MaterialTheme.typography.bodyMedium.tabular(),
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                )
            }
        }
        // The check takes the row's trailing slot, drawn on as it is marked.
        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
            if (check.value > 0f) DrawnCheck(check.value, colors.onSurfaceVariant)
        }
    }
}

/** "E7 · 14 min left", "E6 · 24 min", or "E10 · Airs Thursday 16:30" for one still to air. */
private fun episodeDetail(row: EpisodeRow, resume: tv.anistream.engine.model.ResumePoint?, unaired: Boolean): String {
    val number = "E${row.number}"
    if (unaired) {
        val airs = row.airsAt?.let { AirTime.words(java.time.Instant.ofEpochSecond(it)) }
        return if (airs != null) "$number · Airs $airs" else "$number · Not out yet"
    }
    val fraction = resume?.fraction
    if (resume != null && fraction != null && fraction > 0) {
        val left = maxOf(1, ((resume.position / fraction - resume.position) / 60).roundToInt())
        return "$number · $left min left"
    }
    return minutes(row)?.let { "$number · $it" } ?: number
}

/** The check, drawn along its path to [progress], as the brief's trimPathEnd draw-on does it. */
@Composable
private fun DrawnCheck(progress: Float, color: androidx.compose.ui.graphics.Color) {
    Canvas(Modifier.size(24.dp).semantics { contentDescription = "Watched" }) {
        val unit = size.width / 24f
        val path = Path().apply {
            moveTo(5f * unit, 12.5f * unit)
            lineTo(10f * unit, 17.5f * unit)
            lineTo(19f * unit, 7f * unit)
        }
        val measure = PathMeasure().apply { setPath(path, false) }
        val drawn = Path()
        measure.getSegment(0f, measure.length * progress, drawn, true)
        drawPath(drawn, color, style = Stroke(width = 2f * unit, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/**
 * Whether [id]'s stills past the next episode are hidden (A11, F8): the show's own setting when it
 * sets one, once its settings are asked for, else Settings' Spoiler protection. Hidden until
 * Settings has answered, so a still is never shown and then covered.
 */
@Composable
internal fun spoilerProtected(id: AnilistId): Boolean {
    val store = LocalAppStore.current
    val sheets by store.showSettings.sheets.collectAsStateWithLifecycle()
    val view by store.settings.view.collectAsStateWithLifecycle()
    // The show's row says the value in effect for it, its own or Settings'.
    sheets[id]?.rows?.firstOrNull { it.key == ShowSettingKey.SPOILER_PROTECTION }?.let { return SettingsCatalog.isOnWords(it.value) }
    if (view == null) return true
    return store.settings.isOn(SettingKey.SPOILER_PROTECTION, view)
}

/**
 * An episode's still: its art, blurred with the hidden glyph when spoiler protection hides it, and
 * a plain plate before it airs. Hidden and clear crossfade on standard when the next episode moves.
 */
@Composable
internal fun EpisodeStill(entry: Entry, row: EpisodeRow, hidden: Boolean, unaired: Boolean) {
    val url = row.thumbnail ?: entry.bannerUrl
    Box(Modifier.fillMaxSize().clip(ArtShape.cover(96.dp))) {
        if (unaired) {
            // A flat plate before it airs.
            Box(Modifier.fillMaxSize().background(ObiSilk.Raised))
            return@Box
        }
        Crossfade(hidden, animationSpec = Motion.standard(), label = "still") { blurred -> // i18n-skip
            if (blurred) {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceContainer)) {
                    ArtworkImage(
                        url,
                        plate = null,
                        maxPixelSize = 160,
                        modifier = Modifier.fillMaxSize().graphicsLayer { scaleX = 1.15f; scaleY = 1.15f }.blur(6.dp),
                    )
                    Icon(
                        Icons.Hidden,
                        contentDescription = "Still hidden: spoiler protection",
                        modifier = Modifier.align(Alignment.Center).size(20.dp),
                    )
                }
            } else {
                ArtworkImage(url, plate = null, maxPixelSize = 300, modifier = Modifier.fillMaxSize())
            }
        }
    }
}

/**
 * The row's long-press menu: mark it watched or not, or play it from the start. One no source has,
 * AniList's alone, offers to add a source in place of downloading and playing it.
 */
@Composable
internal fun EpisodeMenu(entry: Entry, row: EpisodeRow, expanded: Boolean, onDismiss: () -> Unit, actions: AppActions) {
    val store = LocalAppStore.current
    val marks = LocalUserMarks.current
    val snackbars = LocalSnackbars.current
    val navigator = LocalNavigator.current
    Menu(expanded = expanded, onDismissRequest = onDismiss) {
        val watched = row.completed
        DropdownMenuItem(
            text = { Text(if (watched) "Mark as unwatched" else "Mark as watched") },
            leadingIcon = { Icon(Icons.Check, contentDescription = null) },
            onClick = {
                onDismiss()
                marks.mark(row.number)
                store.setWatched(entry.id, listOf(row.number), watched = !watched)
                if (!watched) {
                    snackbars.show("Episode ${row.number} marked as watched", "Undo") {
                        marks.mark(row.number)
                        store.setWatched(entry.id, listOf(row.number), watched = false)
                    }
                }
            },
        )
        if (row.unsourced) {
            // Nothing to play or download: marking it watched is all that is left.
        } else if (row.metadataOnly) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.title_button_add_source)) },
                leadingIcon = { Icon(Icons.Providers, contentDescription = null) },
                onClick = {
                    onDismiss()
                    navigator.open(PROVIDERS_ROUTE)
                },
            )
        } else {
            DownloadMenuItem(entry, row.number, onDismiss)
            if (store.tv.available) {
                DropdownMenuItem(
                    text = { Text("Play on TV") },
                    leadingIcon = { Icon(Icons.Tv, contentDescription = null) },
                    onClick = {
                        onDismiss()
                        store.tv.offer(TvPlayRequest(entry.id, entry.showTitle, row.number))
                    },
                )
            }
            DropdownMenuItem(
                text = { Text("Play from the beginning") },
                leadingIcon = { Icon(Icons.Restart, contentDescription = null) },
                onClick = {
                    onDismiss()
                    actions.playFromStart(entry.id, row.number)
                },
            )
        }
    }
}

/**
 * A count that rolls a changed digit: up and out with the new one up from below when it grows,
 * the other way when it shrinks, on standard.
 */
@Composable
fun RollingCount(value: Int, style: androidx.compose.ui.text.TextStyle, color: androidx.compose.ui.graphics.Color) {
    Row {
        value.toString().forEachIndexed { index, digit ->
            AnimatedContent(
                targetState = digit,
                transitionSpec = {
                    val up = targetState > initialState
                    (slideInVertically(Motion.standard()) { if (up) it / 2 else -it / 2 } + fadeIn(Motion.standard())) togetherWith
                        (slideOutVertically(Motion.standard()) { if (up) -it / 2 else it / 2 } + fadeOut(Motion.standard()))
                },
                label = "digit-$index", // i18n-skip
            ) { Text(it.toString(), style = style, color = color) }
        }
    }
}
