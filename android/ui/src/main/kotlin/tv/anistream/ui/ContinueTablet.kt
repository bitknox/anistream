// Continue on a tablet, as AndroidTabletHome draws it: the queue as a list beside the selected
// title's preview, in Material's list-detail scaffold. The preview's content crossfades as the
// selection moves (AT2).

package tv.anistream.ui

import androidx.compose.ui.platform.LocalContext
import tv.anistream.store.text
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDragHandle
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.layout.AnimatedPane
import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffold
import androidx.compose.material3.adaptive.layout.PaneExpansionAnchor
import androidx.compose.material3.adaptive.layout.rememberPaneExpansionState
import androidx.compose.material3.adaptive.navigation.rememberListDetailPaneScaffoldNavigator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.store.ContinueItem
import tv.anistream.store.ContinueQueue

/**
 * What the queue keeps through a fold, a turn or a resize (AT3): the title selected, whether the
 * viewer picked it, and the list's scroll. Held by Continue, above the layouts it switches
 * between, so neither is lost when the window crosses 600 dp and back.
 */
@Stable
internal class QueueState(selected: Long?, picked: Boolean, val list: LazyListState) {
    var selected by mutableStateOf(selected)
    /** Whether the viewer chose the selection, rather than it falling on the first title. */
    var picked by mutableStateOf(picked)

    companion object {
        val Saver: Saver<QueueState, Any> = listSaver(
            save = { listOf(it.selected ?: -1L, it.picked, it.list.firstVisibleItemIndex, it.list.firstVisibleItemScrollOffset) },
            restore = {
                QueueState(
                    (it[0] as Long).takeIf { id -> id >= 0 },
                    it[1] as Boolean,
                    LazyListState(it[2] as Int, it[3] as Int),
                )
            },
        )
    }
}

@Composable
internal fun rememberQueueState(): QueueState = rememberSaveable(saver = QueueState.Saver) {
    QueueState(selected = null, picked = false, list = LazyListState())
}

/** Selections closer together than this, a held arrow key or a run of taps, skip the crossfade. */
private const val QUICK_RESELECT_MILLIS = 150L

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
internal fun TabletQueue(queue: ContinueQueue, actions: AppActions, state: QueueState) {
    val items = queue.all
    val current = items.firstOrNull { it.id.toLong() == state.selected } ?: items.firstOrNull() ?: return
    val panes = rememberListDetailPaneScaffoldNavigator<Long>()
    // The divider settles on the list's 380 or on half the content (AT2).
    val split = rememberPaneExpansionState(
        anchors = listOf(
            PaneExpansionAnchor.Offset.fromStart(380.dp),
            PaneExpansionAnchor.Proportion(0.5f),
        ),
    )
    // When the selection last moved, so a quick run of them lands without the crossfade.
    var lastPick by remember { mutableStateOf(0L) }
    var quick by remember { mutableStateOf(false) }
    ListDetailPaneScaffold(
        directive = panes.scaffoldDirective,
        value = panes.scaffoldValue,
        modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars),
        paneExpansionState = split,
        paneExpansionDragHandle = { handle ->
            val interaction = remember { MutableInteractionSource() }
            VerticalDragHandle(
                modifier = Modifier
                    .paneExpansionDraggable(handle, LocalMinimumInteractiveComponentSize.current, interaction)
                    .semantics { contentDescription = "Resize the queue" },
                interactionSource = interaction,
            )
        },
        listPane = {
            AnimatedPane(Modifier.preferredWidth(380.dp)) {
                QueueList(
                    queue,
                    current,
                    state.list,
                    onSelect = {
                        val now = System.nanoTime() / 1_000_000
                        quick = now - lastPick < QUICK_RESELECT_MILLIS
                        lastPick = now
                        state.selected = it.id.toLong()
                        state.picked = true
                    },
                    actions,
                )
            }
        },
        detailPane = {
            AnimatedPane {
                // The preview's content crossfades with the selection, keyed by the title, both
                // ways on fade-in; a quick run of selections lands at once.
                AnimatedContent(
                    current,
                    transitionSpec = {
                        if (quick) EnterTransition.None togetherWith ExitTransition.None
                        else fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeIn())
                    },
                    contentKey = { it.id },
                    label = "preview",
                ) { item -> QueuePreview(item, actions) }
            }
        },
    )
}

@Composable
private fun QueueList(
    queue: ContinueQueue,
    current: ContinueItem,
    list: LazyListState,
    onSelect: (ContinueItem) -> Unit,
    actions: AppActions,
) {
    // Under the title, as on a phone: the offline line, a job past 4 s, or the engine's status.
    val status by LocalAppStore.current.notices.status.collectAsStateWithLifecycle()
    val line = frameLine(status?.text(LocalContext.current))
    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
        // The headings move with the rows, so a row never slides over a heading that snapped.
        item(key = "title", contentType = "title") {
            Column(itemMotion().heightIn(min = 64.dp).padding(horizontal = 12.dp), verticalArrangement = Arrangement.Center) {
                Text("Continue", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.semantics { heading() })
                line?.let { FrameLineText(it, MaterialTheme.typography.bodyMedium, Modifier.padding(bottom = 8.dp)) }
            }
        }
        if (queue.inProgress.isNotEmpty()) {
            item(key = "progress", contentType = "heading") { ListHeading("In progress", itemMotion()) }
            items(queue.inProgress, key = { "p-${it.id}" }, contentType = { "row" }) { Box(itemMotion()) { QueueRow(it, it.id == current.id, onSelect, actions) } }
        }
        if (queue.newThisWeek.isNotEmpty()) {
            item(key = "new", contentType = "heading") { ListHeading("New this week", itemMotion().padding(top = 16.dp)) }
            items(queue.newThisWeek, key = { "n-${it.id}" }, contentType = { "row" }) { Box(itemMotion()) { QueueRow(it, it.id == current.id, onSelect, actions) } }
        }
    }
}

@Composable
private fun ListHeading(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        modifier = modifier.padding(horizontal = 12.dp, vertical = 8.dp).semantics { heading() },
    )
}

/** A row of the queue: selected on primaryContainer, with its menu at the end. */
@Composable
private fun QueueRow(item: ContinueItem, selected: Boolean, onSelect: (ContinueItem) -> Unit, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    var menu by remember { mutableStateOf(false) }
    val menuActions = continueMenuActions(item, actions, "queue")
    // The fill leaves the old row and arrives on the new one on quick (AT2).
    val fill by animateColorAsState(
        if (selected) colors.primaryContainer else colors.primaryContainer.copy(alpha = 0f),
        Motion.quick(),
        label = "selection",
    )
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clip(MaterialTheme.shapes.large)
            .background(fill)
            // Arrow keys and a D-pad walk the selection with the focus (AT2); this watches the
            // row's own focus, so it stands ahead of it.
            .onFocusChanged { if (it.isFocused && !selected) onSelect(item) }
            .selectable(selected = selected, onClick = { onSelect(item) })
            .semantics { customActions = menuActions.accessibility }
            .padding(start = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CoverArt(item.entry, 40.dp, Modifier.size(40.dp, 60.dp))
        Column(Modifier.weight(1f)) {
            Text(
                item.entry.showTitle,
                style = ShowTitle.row,
                color = if (selected) colors.primary else colors.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (item.isNew) Box(Modifier.size(6.dp).background(colors.secondary, CircleShape))
                Text(
                    item.action,
                    style = MaterialTheme.typography.bodyMedium.tabular(),
                    color = if (selected) colors.onPrimaryContainer else colors.onSurfaceVariant,
                    maxLines = 1,
                )
            }
            item.fraction?.let {
                LinearProgressIndicator(
                    progress = { it.toFloat() },
                    modifier = Modifier.padding(top = 6.dp).fillMaxWidth().height(4.dp),
                    color = colors.primary,
                    trackColor = colors.outline,
                    gapSize = 4.dp,
                    drawStopIndicator = {},
                )
            }
        }
        Box {
            IconButton(onClick = { menu = true }, modifier = Modifier.padding(end = 4.dp).semantics { contentDescription = "More for ${item.entry.showTitle}" }) {
                Box(Modifier.size(40.dp).background(if (selected) colors.surfaceContainerHigh else colors.surfaceContainer, CircleShape), contentAlignment = Alignment.Center) {
                    Icon(Icons.More, contentDescription = null)
                }
            }
            ContinueMenu(menuActions, expanded = menu, onDismiss = { menu = false })
        }
    }
}

/** The selected title: its banner, what plays and how far into it, the next episode, the synopsis. */
@Composable
private fun QueuePreview(item: ContinueItem, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    LaunchedEffect(item.id) {
        store.loadEpisodes(item.id)
        store.loadShowSettings(item.id)
    }
    val rows = episodes[item.id]
    val name = rows?.firstOrNull { it.number == item.episode.toString() }?.title
    val upNext = rows?.firstOrNull { it.number == (item.episode + 1u).toString() }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(top = 12.dp, end = 24.dp, bottom = 24.dp)) {
        BannerArt(item.entry, Modifier.fillMaxWidth().height(300.dp).clip(ArtShape.banner))
        Text(item.entry.showTitle, style = ShowTitle.page, modifier = Modifier.padding(top = 16.dp))
        item.entry.alternateTitle?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
        if (item.entry.metadataLine.isNotEmpty()) {
            Text(item.entry.metadataLine, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
        }
        Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(name?.let { "Episode ${item.episode} · $it" } ?: "Episode ${item.episode}", style = MaterialTheme.typography.bodyMedium)
                item.minutesLeft?.let { Text("$it min left", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
            }
            item.fraction?.let {
                LinearProgressIndicator(
                    progress = { it.toFloat() },
                    modifier = Modifier.fillMaxWidth().height(4.dp),
                    color = colors.primary,
                    trackColor = colors.outline,
                    gapSize = 4.dp,
                    drawStopIndicator = {},
                )
            }
        }
        Row(Modifier.padding(top = 16.dp).heightIn(min = 56.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = { actions.play(item.id, item.episode.toString()) },
                enabled = item.playable,
                // The action gives way before Episodes does: it is cut at the end, never wrapped.
                modifier = Modifier.weight(1f, fill = false).heightIn(min = 56.dp),
                contentPadding = PaddingValues(horizontal = 24.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Text(item.action, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            FilledTonalButton(onClick = { navigator.openTitle(item.id, "preview") }, contentPadding = PaddingValues(horizontal = 24.dp)) {
                Text("Episodes", maxLines = 1, softWrap = false)
            }
        }
        upNext?.let { row ->
            HorizontalDivider(Modifier.padding(top = 8.dp), color = colors.outlineVariant)
            // The episode after the one under way: its still hidden, as the title page hides it.
            Row(
                Modifier.heightIn(min = 80.dp).semantics(mergeDescendants = true) {},
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.size(96.dp, 54.dp)) {
                    EpisodeStill(item.entry, row, hidden = spoilerProtected(item.id) && !row.completed, unaired = false)
                }
                Column {
                    row.title?.let { Text(it, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                    Text(
                        listOfNotNull("E${row.number}", "Up next", minutes(row)).joinToString(" · "),
                        style = MaterialTheme.typography.bodyMedium.tabular(),
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
            HorizontalDivider(color = colors.outlineVariant)
        }
        if (item.entry.synopsis.isNotEmpty()) {
            Text(item.entry.synopsis, style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 12.dp))
        }
        if (item.entry.availableOn.isNotEmpty()) {
            Text(
                "Also on " + item.entry.availableOn.joinToString(", "),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}
