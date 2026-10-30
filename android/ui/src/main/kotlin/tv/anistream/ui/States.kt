// The states every screen shares beyond its content: failed, with the reason in plain words and
// Try again; the one offline state, as a line under the title or, with nothing to show, in place
// of the screen; work under way in a button or a row; and a job's activity once it runs past 4 s.

package tv.anistream.ui

import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import tv.anistream.store.plainReason
import tv.anistream.store.text
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.store.AppStore
import java.util.Locale
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.design.Motion
import tv.anistream.store.BugContext
import tv.anistream.store.JobsModel

/** The offline line, under a screen's title while it shows what it has. */

/** How long a job runs before its activity is said in words under the title. */
internal const val SLOW_MILLIS = 4_000L

/** The offline line: the wifi-off glyph and the words, in haze, never a coloured banner. */
@Composable
internal fun OfflineLine(modifier: Modifier = Modifier, style: TextStyle = MaterialTheme.typography.bodyMedium) {
    Row(
        modifier.semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.Offline, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        Text(stringResource(R.string.state_offline_line), style = style, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * What stands under a screen's title: the offline line while offline, else a job's activity once
 * it has run past 4 s ("Loading this season"), else the screen's own [subtitle]. `null` for none.
 */
@Composable
internal fun frameLine(subtitle: String?): FrameLine? {
    val store = LocalAppStore.current
    val offline by store.offline.collectAsStateWithLifecycle()
    val slow = slowJob()
    return when {
        offline -> FrameLine.Offline
        slow != null -> FrameLine.Words(slow)
        subtitle != null -> FrameLine.Words(subtitle)
        else -> null
    }
}

internal sealed interface FrameLine {
    data object Offline : FrameLine
    data class Words(val text: String) : FrameLine
}

/** [line] drawn in [style]: the offline line with its glyph, or the words in haze. */
@Composable
internal fun FrameLineText(line: FrameLine, style: TextStyle, modifier: Modifier = Modifier) {
    when (line) {
        FrameLine.Offline -> OfflineLine(modifier, style)
        is FrameLine.Words -> Text(line.text, style = style, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = modifier)
    }
}

/** The newest job that has run past 4 s, in words, or `null`. */
@Composable
private fun slowJob(): String? {
    val store = LocalAppStore.current
    val shown by store.jobs.shown.collectAsStateWithLifecycle()
    val job = shown.firstOrNull()
    var slow by remember { mutableStateOf(false) }
    LaunchedEffect(job?.id) {
        slow = false
        if (job != null) {
            // Shown already means 600 ms gone.
            delay(SLOW_MILLIS - JobsModel.DELAY_MILLIS)
            slow = true
        }
    }
    if (!slow || job == null) return null
    return sentence(job.note?.text(LocalContext.current) ?: job.activity)
}

/** Whether [active] has held for [millis]: work earns its place on screen only past 600 ms. */
@Composable
internal fun after(active: Boolean, millis: Long = JobsModel.DELAY_MILLIS): Boolean {
    var past by remember { mutableStateOf(false) }
    LaunchedEffect(active) {
        past = false
        if (active) {
            delay(millis)
            past = true
        }
    }
    return active && past
}

/**
 * Work under way in a button or a row, past 600 ms: a 16 dp ring in haze, beside the label it
 * leaves as it is.
 */
@Composable
internal fun WorkRing(working: Boolean, modifier: Modifier = Modifier) {
    AnimatedVisibility(after(working), modifier, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        CircularProgressIndicator(
            modifier = Modifier.size(16.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            trackColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0f),
            strokeWidth = 2.dp,
        )
    }
}

/**
 * A screen that could not load: the error glyph in vermilion, [title], the reason's first clause
 * in plain words, Try again, and Report as the quieter action, which opens the bug report with the
 * whole reason. Offline, the screen says so instead, with Show downloads.
 */
@Composable
internal fun FailedState(title: String, reason: String, retry: () -> Unit, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val offline by store.offline.collectAsStateWithLifecycle()
    if (offline) {
        OfflineState(modifier)
        return
    }
    val context = LocalContext.current
    StateBlock(
        glyph = { Icon(Icons.Problem, contentDescription = stringResource(R.string.error_label), tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(32.dp)) },
        title = title,
        line = plainReason(LocalContext.current, reason),
        modifier = modifier,
    ) {
        FilledTonalButton(onClick = retry) { Text(stringResource(R.string.action_try_again)) }
        TextButton(
            onClick = { openBugReport(context, store, BugContext("load", reason)) },
            colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
        ) { Text(stringResource(R.string.action_report)) }
    }
}

/**
 * Over the whole app when its engine did not start or has stopped: every list would sit in its
 * placeholders and every command would be dropped. Try again starts it once more; Quit leaves.
 */
@Composable
internal fun EngineDown() {
    val store = LocalAppStore.current
    val phase by store.phase.collectAsStateWithLifecycle()
    val shown = phase
    if (shown !is AppStore.Phase.Failed && shown !is AppStore.Phase.Stopped) return
    val activity = androidx.activity.compose.LocalActivity.current
    androidx.compose.material3.Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            StateBlock(
                glyph = { Icon(Icons.Problem, contentDescription = stringResource(R.string.error_label), tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(32.dp)) },
                title = stringResource(R.string.engine_down_title),
                line = if (shown is AppStore.Phase.Failed) plainReason(LocalContext.current, shown.reason) else stringResource(R.string.engine_down_stopped),
            ) {
                FilledTonalButton(onClick = store::restartEngine) { Text(stringResource(R.string.action_try_again)) }
                TextButton(onClick = { activity?.finishAffinity() }) { Text(stringResource(R.string.engine_down_quit)) }
            }
        }
    }
}

/** A screen with nothing to show while offline: "You're offline", with Show downloads. */
@Composable
internal fun OfflineState(modifier: Modifier = Modifier) {
    val navigator = LocalNavigator.current
    StateBlock(
        glyph = { Icon(Icons.Offline, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(32.dp)) },
        title = stringResource(R.string.state_offline_title),
        line = stringResource(R.string.state_offline_detail),
        modifier = modifier,
    ) {
        FilledTonalButton(onClick = { navigator.open(DOWNLOADS_ROUTE) }) { Text(stringResource(R.string.state_offline_show_downloads)) }
    }
}

/**
 * An empty answer, once it is known: Nagare at [cell], [title], one line, and the one filled way
 * forward when there is one.
 */
@Composable
internal fun EmptyState(title: String, line: String, action: String?, onAction: () -> Unit, modifier: Modifier = Modifier, cell: Int = 5) {
    StateBlock(glyph = { tv.anistream.design.Nagare(cell = cell) }, title = title, line = line, modifier = modifier) {
        action?.let { androidx.compose.material3.Button(onClick = onAction) { Text(it) } }
    }
}

/** A settings group's rows before their answer: [count] redacted rows, after the 600 ms wait. */
@Composable
internal fun PlaceholderRows(count: Int = 3) {
    AfterWait {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            repeat(count) { index ->
                RowFrame(Place.of(index, count), onClick = null, modifier = Modifier.heightIn(min = 72.dp)) {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        TextPlaceholder(0.45f, 24.dp)
                        TextPlaceholder(0.3f, 20.dp)
                    }
                }
            }
        }
    }
}

/** A button's label with its work ring beside it, past 600 ms, the label left as it is. */
@Composable
internal fun WorkLabel(label: String, working: Boolean) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        WorkRing(working)
        Text(label)
    }
}

/**
 * A page's reading column: at most 600 dp wide, from the start under the title, so a tablet's
 * rows keep the pane's measure instead of running across the window.
 */
internal fun Modifier.readingWidth(): Modifier =
    wrapContentWidth(Alignment.Start).widthIn(max = 600.dp).fillMaxWidth()

@Composable
private fun StateBlock(
    glyph: @Composable () -> Unit,
    title: String,
    line: String,
    modifier: Modifier = Modifier,
    buttons: @Composable () -> Unit,
) {
    Column(
        modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        glyph()
        Text(
            title,
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(top = 16.dp).semantics { heading() },
        )
        Text(
            line,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            modifier = Modifier.padding(top = 4.dp),
        )
        Row(Modifier.padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            androidx.compose.runtime.CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) { buttons() }
        }
    }
}

/** How long a pull to refresh may spin waiting for its job. */
private const val REFRESH_MAX_MILLIS = 15_000L

/**
 * Pull to refresh: [refresh] asks again, and the indicator stays until the jobs it started end,
 * at most 15 s. Silk on a raised disc, since amber is for the one action and for progress.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
internal fun Refreshable(
    refresh: () -> Unit,
    modifier: Modifier = Modifier,
    /** Where the indicator comes down from: under the top bar, which the content scrolls under. */
    top: androidx.compose.ui.unit.Dp = 0.dp,
    content: @Composable () -> Unit,
) {
    val store = LocalAppStore.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    var refreshing by remember { mutableStateOf(false) }
    val state = androidx.compose.material3.pulltorefresh.rememberPullToRefreshState()
    androidx.compose.material3.pulltorefresh.PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = {
            refreshing = true
            val before = store.jobs.running.value.map { it.id }.toSet()
            refresh()
            scope.launch {
                kotlinx.coroutines.withTimeoutOrNull(REFRESH_MAX_MILLIS) {
                    delay(300)
                    store.jobs.running.first { running -> running.none { it.id !in before } }
                }
                refreshing = false
            }
        },
        state = state,
        modifier = modifier,
        indicator = {
            androidx.compose.material3.pulltorefresh.PullToRefreshDefaults.Indicator(
                state = state,
                isRefreshing = refreshing,
                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.align(Alignment.TopCenter).padding(top = top),
            )
        },
    ) { content() }
}

/** One swipe on a row: what it says behind the row as it opens, and what it does. */
internal class SwipeAction(val label: String, val icon: androidx.compose.ui.graphics.vector.ImageVector, val run: () -> Unit)

/**
 * A row that swipes (`SwipeToDismissBox`): [start] from the start edge, [end] from the end edge.
 * Behind it, the action's glyph and words in silk on a raised ground, never a colour. The row
 * always springs back; the action decides whether it goes, as a hide or a delete that asks first.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
internal fun Swipeable(start: SwipeAction?, end: SwipeAction?, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val state = androidx.compose.material3.rememberSwipeToDismissBoxState()
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    androidx.compose.material3.SwipeToDismissBox(
        state = state,
        modifier = modifier,
        enableDismissFromStartToEnd = start != null,
        enableDismissFromEndToStart = end != null,
        onDismiss = { value ->
            when (value) {
                androidx.compose.material3.SwipeToDismissBoxValue.StartToEnd -> start?.run?.invoke()
                androidx.compose.material3.SwipeToDismissBoxValue.EndToStart -> end?.run?.invoke()
                else -> {}
            }
            scope.launch { state.reset() }
        },
        backgroundContent = {
            val action = when (state.dismissDirection) {
                androidx.compose.material3.SwipeToDismissBoxValue.StartToEnd -> start
                androidx.compose.material3.SwipeToDismissBoxValue.EndToStart -> end
                else -> null
            }
            val fromStart = state.dismissDirection == androidx.compose.material3.SwipeToDismissBoxValue.StartToEnd
            androidx.compose.foundation.layout.Box(
                Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .padding(horizontal = 24.dp),
                contentAlignment = if (fromStart) Alignment.CenterStart else Alignment.CenterEnd,
            ) {
                if (action != null) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Icon(action.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface)
                        Text(action.label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurface)
                    }
                }
            }
        },
    ) { content() }
}
