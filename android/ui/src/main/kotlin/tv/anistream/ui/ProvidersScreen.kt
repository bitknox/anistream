// Providers (AndroidProviders): the sources in the order failover tries them, each with the one
// quiet line saying where it came from, and its state only when it has a problem, an update or is
// off. A switch turns a plugin off without removing it. Reorder in the top bar swaps the switches
// for handles, and a source dragged by its handle moves through the order with a tick at each
// place it passes. A source's page says its version, response time and the hosts it may contact.

package tv.anistream.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import kotlin.math.roundToInt
import tv.anistream.design.tabular
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.ProviderRow
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.ProviderStanding
import tv.anistream.store.SourceOrder
import tv.anistream.store.standing

internal const val PROVIDERS_ROUTE = "providers"

internal fun providerRoute(id: String) = "provider/${android.net.Uri.encode(id)}"

/** Providers' routes: the order, and a source's page. */
internal fun NavGraphBuilder.providerPages(actions: AppActions) {
    composable(PROVIDERS_ROUTE) { ProvidersScreen() }
    composable("provider/{id}") { entry ->
        val id = entry.arguments?.getString("id") ?: return@composable
        ProviderScreen(id, actions)
    }
}

/** What Providers says of a source: its name, and the quiet line under it. */
internal data class ProviderInfo(val row: ProviderRow) {
    val isTorrent get() = row.kind == "torrent"
    val isPlugin get() = row.kind == "plugin"

    val name: String
        get() = when (row.kind) {
            "torrent" -> "Torrents"
            "remote" -> "Remote"
            else -> row.name.ifEmpty { row.id }
        }

    /** "Hotaru Plugins", "Added by hand", "Built in". */
    val origin: String
        get() = when (val origin = row.origin) {
            is ProviderOrigin.Store -> origin.name ?: "A store you left"
            ProviderOrigin.ByHand -> "Added by hand"
            ProviderOrigin.BuiltIn -> if (row.kind == "remote") "Set in the config file" else "Built in"
        }

    /**
     * The state beside the origin: only for a problem, an update or off. Offline, no source is
     * said to be failing: the one offline line says why nothing answers.
     */
    fun state(offline: Boolean): Pair<String, StateKind>? = when {
            row.disabled -> "Off" to StateKind.Off
            row.standing == ProviderStanding.Failing && !offline -> "Not answering" to StateKind.Problem
            row.standing == ProviderStanding.HeldBack -> "Held back" to StateKind.Off
            row.update != null -> "Update available" to StateKind.Update
            else -> null
        }

    enum class StateKind { Off, Problem, Update }
}

@Composable
internal fun ProvidersScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    val loaded by store.providers.loaded.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    var editing by rememberSaveable { mutableStateOf(false) }
    var sheet by rememberSaveable { mutableStateOf<TorrentSheetMode?>(null) }
    LaunchedEffect(Unit) {
        store.checkProviders()
        store.loadStores()
    }
    val order = view?.sources?.order.orEmpty()
    val sorted = SourceOrder.sorted(rows, order)
    ScreenFrame(
        title = "Providers",
        onBack = navigator::back,
        actions = {
            if (sorted.size > 1) {
                IconButton(onClick = { editing = !editing }) {
                    Icon(if (editing) Icons.Check else Icons.Reorder, contentDescription = if (editing) "Done" else "Reorder")
                }
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState(), enabled = true)
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
        ) {
            when {
                !loaded -> PlaceholderRows()
                sorted.isEmpty() -> EmptyState(
                    title = "No sources yet",
                    line = "Sources come as plugins from a plugin store. $LEGAL_LINE",
                    action = "Join a plugin store",
                    onAction = { store.stores.openJoin() },
                )
                else -> {
                    SourceList(
                        sorted,
                        editing,
                        offline,
                        torrents = store.settings.isOn(SettingKey.TORRENTS, view),
                        onTorrents = { on -> if (on) sheet = TorrentSheetMode.TurnOn else store.setSwitch(SettingKey.TORRENTS, false) },
                    ) { id, target ->
                        SourceOrder.movedTo(order, sorted, id, target)?.let { store.setSetting(SettingKey.SOURCE_ORDER, SettingValue.Strings(it)) }
                    }
                    SettingsFootnote(
                        if (editing) "Drag a source by its handle to change when it is tried."
                        else "Sources are tried top to bottom. To change the order, tap Reorder in the top bar.",
                    )
                    SettingsFootnote(LEGAL_LINE)
                }
            }
        }
    }
    sheet?.let { mode -> TorrentSheet(mode) { sheet = null } }
}

/**
 * The sources, one group. While [editing], each row carries a handle, and a row dragged by it
 * follows the finger, the others making way at each place it passes, with a tick each time; let
 * go, the order is written. Each row also offers Move up and Move down to TalkBack.
 */
@Composable
private fun SourceList(
    rows: List<ProviderRow>,
    editing: Boolean,
    offline: Boolean,
    torrents: Boolean,
    onTorrents: (Boolean) -> Unit,
    move: (id: String, target: Int) -> Unit,
) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val haptics = LocalHapticFeedback.current
    val heights = remember { mutableStateMapOf<String, Int>() }
    var dragging by remember { mutableStateOf<String?>(null) }
    var offset by remember { mutableFloatStateOf(0f) }
    var target by remember { mutableStateOf(-1) }
    val gap = with(androidx.compose.ui.platform.LocalDensity.current) { 2.dp.toPx() }

    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        rows.forEachIndexed { index, row ->
            val info = ProviderInfo(row)
            val from = rows.indexOfFirst { it.id == dragging }
            // Where this row sits while another is dragged past it: one row's height up or down.
            val shift = when {
                dragging == null || dragging == row.id || from < 0 || target < 0 -> 0f
                from < target && index in (from + 1)..target -> -((heights[dragging] ?: 0) + gap)
                from > target && index in target until from -> (heights[dragging] ?: 0) + gap
                else -> 0f
            }
            // Read where it is placed, not here, so making way moves the row and composes nothing.
            val shiftPx = animateFloatAsState(shift, animationSpec = tv.anistream.design.Motion.standard(), label = "shift")
            val lifted = dragging == row.id
            Box(
                Modifier
                    .zIndex(if (lifted) 1f else 0f)
                    .onSizeChanged { heights[row.id] = it.height }
                    .graphicsLayer { translationY = if (lifted) offset else 0f }
                    .offset { IntOffset(0, if (lifted) 0 else shiftPx.value.roundToInt()) }
                    .semantics {
                        customActions = listOfNotNull(
                            if (index > 0) CustomAccessibilityAction("Move up") { move(row.id, index - 1); true } else null,
                            if (index < rows.lastIndex) CustomAccessibilityAction("Move down") { move(row.id, index + 1); true } else null,
                        )
                    },
            ) {
                ProviderListRow(
                    Place.of(index, rows.size),
                    info,
                    editing = editing,
                    offline = offline,
                    lifted = lifted,
                    onOpen = {
                        if (info.isTorrent) navigator.openSettings(SettingsPane.Sources) else navigator.open(providerRoute(row.id))
                    },
                    // Torrents' switch is the setting's: turning it on asks how to protect the connection.
                    switchOn = if (info.isTorrent) torrents else !row.disabled,
                    onToggle = { on -> if (info.isTorrent) onTorrents(on) else store.disablePlugin(row.id, disabled = !on) },
                    handle = Modifier.pointerInput(row.id, rows) {
                        detectDragGestures(
                            onDragStart = {
                                dragging = row.id
                                offset = 0f
                                target = index
                                haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
                            },
                            onDrag = { change, amount ->
                                change.consume()
                                offset += amount.y
                                val next = targetFor(rows, index, offset, heights, gap)
                                if (next != target) {
                                    target = next
                                    haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
                                }
                            },
                            onDragEnd = {
                                val to = target
                                dragging = null
                                offset = 0f
                                target = -1
                                if (to >= 0 && to != index) move(row.id, to)
                            },
                            onDragCancel = {
                                dragging = null
                                offset = 0f
                                target = -1
                            },
                        )
                    },
                )
            }
        }
    }
}

/** The place a row dragged [offset] from [index] would land on, by the rows' heights. */
private fun targetFor(rows: List<ProviderRow>, index: Int, offset: Float, heights: Map<String, Int>, gap: Float): Int {
    var at = index
    var left = offset
    if (left > 0) {
        while (at < rows.lastIndex) {
            val next = (heights[rows[at + 1].id] ?: 0) + gap
            if (left < next / 2) break
            left -= next
            at++
        }
    } else {
        while (at > 0) {
            val previous = (heights[rows[at - 1].id] ?: 0) + gap
            if (-left < previous / 2) break
            left += previous
            at--
        }
    }
    return at
}

/** One source: its name, its origin and state, and the switch, or the handle while reordering. */
@Composable
private fun ProviderListRow(
    place: Place,
    info: ProviderInfo,
    editing: Boolean,
    offline: Boolean,
    lifted: Boolean,
    switchOn: Boolean,
    onOpen: () -> Unit,
    onToggle: (Boolean) -> Unit,
    handle: Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val on = !info.row.disabled
    // Lifted by its handle, the row rises a container role, not onto a shadow.
    RowFrame(
        place,
        onClick = if (editing) null else onOpen,
        modifier = Modifier.heightIn(min = 72.dp),
        container = if (lifted) colors.surfaceContainerHigh else null,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(Modifier.weight(1f)) {
                Text(info.name, style = MaterialTheme.typography.titleMedium, color = if (on) colors.onSurface else colors.onSurfaceVariant)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(info.origin, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                    info.state(offline)?.let { (text, kind) ->
                        StateDot(
                            when (kind) {
                                ProviderInfo.StateKind.Problem -> colors.error
                                ProviderInfo.StateKind.Update -> colors.secondary
                                ProviderInfo.StateKind.Off -> null
                            },
                        )
                        Text(
                            text,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (kind == ProviderInfo.StateKind.Off) colors.onSurfaceVariant else colors.onSurface,
                        )
                    }
                }
            }
            if (editing) {
                Box(
                    handle
                        .size(48.dp)
                        .semantics { contentDescription = "Move ${info.name}" },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.DragHandle, contentDescription = null, tint = colors.onSurfaceVariant)
                }
            } else if (info.isPlugin || info.isTorrent) {
                Box(
                    Modifier.semantics(mergeDescendants = true) { contentDescription = "Use ${info.name}" },
                ) {
                    androidx.compose.material3.Switch(
                        checked = switchOn,
                        onCheckedChange = onToggle,
                        colors = androidx.compose.material3.SwitchDefaults.colors(
                            checkedTrackColor = colors.primary,
                            checkedThumbColor = colors.onPrimary,
                            checkedBorderColor = androidx.compose.ui.graphics.Color.Transparent,
                            uncheckedTrackColor = colors.surfaceContainerHighest,
                            uncheckedThumbColor = colors.onSurfaceVariant,
                            uncheckedBorderColor = colors.onSurfaceVariant,
                        ),
                    )
                }
            }
        }
    }
}

/**
 * A source's page: its name and where it came from, its state, what can be done with it, and its
 * version, response time and the hosts it may contact.
 */
@Composable
internal fun ProviderScreen(id: String, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    val row = rows.firstOrNull { it.id == id }
    val offline by store.offline.collectAsStateWithLifecycle()
    var checking by remember { mutableStateOf(false) }
    // Check again works until the engine says the providers again, or for 15 s at most.
    LaunchedEffect(rows) { checking = false }
    LaunchedEffect(checking) {
        if (checking) {
            kotlinx.coroutines.delay(15_000)
            checking = false
        }
    }
    var removing by remember { mutableStateOf(false) }
    var updating by remember { mutableStateOf(false) }
    val info = row?.let(::ProviderInfo)
    ScreenFrame(title = info?.name ?: "Provider", subtitle = info?.let { if (it.isPlugin && it.row.origin is ProviderOrigin.Store) "From ${it.origin}" else it.origin }, onBack = navigator::back) { padding ->
        if (info == null) return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            stateLine(info, offline)?.let { (text, problem) ->
                Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    StateDot(if (problem) MaterialTheme.colorScheme.error else null)
                    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                info.row.update?.let { update ->
                    Button(onClick = { updating = true }) { Text("Update to ${update.version}") }
                }
                val silk = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)
                if (info.row.standing == ProviderStanding.Failing && !offline) {
                    OutlinedButton(
                        onClick = {
                            checking = true
                            store.checkProviders()
                        },
                        enabled = !checking,
                        colors = silk,
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            WorkRing(checking)
                            Text("Check again")
                        }
                    }
                }
                if (info.isPlugin) OutlinedButton(onClick = { removing = true }, colors = silk) { Text("Remove") }
            }
            if (info.isPlugin) {
                SettingsGroup(
                    footer = "${info.name} can contact only these hosts and cannot read your files.",
                    rows = listOf(
                        { place -> SwitchRow(place, "Use ${info.name}", checked = !info.row.disabled) { store.disablePlugin(info.row.id, disabled = !it) } },
                        { place ->
                            InfoRow(
                                place,
                                "Version",
                                listOfNotNull(info.row.version ?: "Not from a store", info.row.update?.let { "${it.version} available" }).joinToString(" · "),
                            )
                        },
                        { place ->
                            InfoRow(
                                place,
                                "Response time",
                                when {
                                    info.row.standing == ProviderStanding.Failing && !offline -> "No answer"
                                    info.row.latencyMs != null -> "${info.row.latencyMs} ms"
                                    else -> "Not checked yet"
                                },
                            )
                        },
                        { place -> InfoRow(place, "Contacts", info.row.hosts.ifEmpty { listOf("No hosts") }.joinToString("\n")) },
                    ),
                )
            }
        }
    }
    if (removing && info != null) {
        AlertDialog(
            onDismissRequest = { removing = false },
            title = { Text("Remove ${info.name}?") },
            text = { Text("It is deleted from this device. A store can install it again.") },
            confirmButton = {
                QuietButton("Remove") {
                    removing = false
                    store.removePlugin(info.row.id)
                    navigator.back()
                }
            },
            dismissButton = { QuietButton("Cancel") { removing = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
    if (updating && info != null) {
        val update = info.row.update
        val storeId = (info.row.origin as? ProviderOrigin.Store)?.id
        if (update != null && storeId != null) {
            UpdateDialog(info.name, storeName = info.origin, from = info.row.version, to = update.version, newHosts = update.newHosts, onDismiss = { updating = false }) {
                updating = false
                store.installPlugin(storeId, info.row.id)
            }
        }
    }
}

/** The page's state line. The engine's error goes to Messages whole; the page says it plainly. */
private fun stateLine(info: ProviderInfo, offline: Boolean): Pair<String, Boolean>? = when {
    info.row.disabled -> "Off. It keeps its place and is not tried until it is on again." to false
    info.row.standing == ProviderStanding.Failing && offline -> null
    info.row.standing == ProviderStanding.Failing -> ("Not answering." + (info.row.lastError?.let { " ${providerReason(it)}" } ?: "")) to true
    info.row.standing == ProviderStanding.HeldBack -> "Held back until the connection check passes." to false
    else -> null
}

/**
 * The update confirmation: which version replaces which, and the hosts the update adds, named
 * before anything is installed. Update is the one amber action.
 */
@Composable
internal fun UpdateDialog(
    plugin: String,
    storeName: String,
    from: String?,
    to: String,
    newHosts: List<String>,
    onDismiss: () -> Unit,
    onUpdate: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Update $plugin?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Version $to from $storeName" + (from?.let { " replaces $it." } ?: "."))
                if (newHosts.isNotEmpty()) {
                    Text("This update also contacts ${newHosts.joinToString(", ")}.")
                }
            }
        },
        confirmButton = { Button(onClick = onUpdate) { Text("Update") } },
        dismissButton = { QuietButton("Cancel", onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}

/** A provider's error in plain words: what kind of failure it was, never the request chain. */
internal fun providerReason(raw: String): String {
    val lower = raw.lowercase(java.util.Locale.ROOT)
    return when {
        "timed out" in lower || "timeout" in lower -> "It took too long to answer."
        "429" in lower || "too many requests" in lower -> "It is busy. Try again in a minute."
        Regex("\\b5\\d\\d\\b").containsMatchIn(lower) -> "It had a problem."
        Regex("\\b40[34]\\b").containsMatchIn(lower) -> "It turned the request away."
        "dns" in lower || "resolve" in lower -> "Its address could not be found."
        "error sending request" in lower || "connect" in lower -> "It could not be reached."
        else -> firstClause(raw)
    }
}
