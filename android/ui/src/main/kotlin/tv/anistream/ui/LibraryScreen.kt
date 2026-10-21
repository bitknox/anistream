// The Library: Downloads, the tracker's lists as chips with their counts, the covers of the one
// chosen; or, signed out, where the lists come from.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.util.Locale
import tv.anistream.design.Nagare
import tv.anistream.design.tabular
import tv.anistream.store.LibraryModel
import tv.anistream.store.ListState
import tv.anistream.store.label
import tv.anistream.store.latestAired
import uniffi.anistream_model.Entry
import uniffi.anistream_model.LibrarySegment

/** How the Library orders a list. */
private enum class LibrarySort(val label: String) {
    LastWatched("Last watched"),
    Title("Title"),
    Score("Score"),
}

/** "3.4 GB on this device", or that nothing is. */
internal fun storageLine(bytes: Long): String {
    if (bytes <= 0) return "Nothing on this device"
    val gb = bytes / 1_000_000_000.0
    return if (gb >= 1) String.format(Locale.UK, "%.1f GB on this device", gb)
    else String.format(Locale.UK, "%d MB on this device", (bytes / 1_000_000.0).toLong().coerceAtLeast(1))
}

@Composable
fun LibraryScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val downloads by store.downloads.rows.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    val signedOut = store.accounts.signedOut(trackers)
    var segment by rememberSaveable { mutableStateOf(LibrarySegment.WATCHING) }
    var sort by rememberSaveable { mutableStateOf(LibrarySort.LastWatched) }
    var sorting by remember { mutableStateOf(false) }
    LaunchedEffect(signedOut) {
        store.loadDownloads()
        if (!signedOut) {
            for (each in LibraryModel.order) if (store.library.segment(each).state.value == ListState.Idle) store.loadLibrary(each)
        }
    }
    val states = LibraryModel.order.associateWith { store.library.segment(it).state.collectAsStateWithLifecycle() }

    ScreenFrame(
        title = "Library",
        account = Account(user, actions.openSettings),
        actions = {
            if (!signedOut) {
                Box {
                    androidx.compose.material3.IconButton(onClick = { sorting = true }) {
                        Icon(Icons.Sort, contentDescription = "Sort: ${sort.label}")
                    }
                    DropdownMenu(expanded = sorting, onDismissRequest = { sorting = false }) {
                        for (each in LibrarySort.entries) {
                            DropdownMenuItem(
                                text = { Text(each.label) },
                                leadingIcon = {
                                    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                        if (each == sort) Icon(Icons.Check, contentDescription = "Current")
                                    }
                                },
                                onClick = {
                                    sorting = false
                                    sort = each
                                },
                            )
                        }
                    }
                }
            }
        },
    ) { padding ->
        if (signedOut) {
            Column(Modifier.padding(top = padding.calculateTopPadding()).fillMaxSize()) {
                DownloadsRow(storageLine(store.downloads.bytes(downloads)))
                SignedOutLibrary(actions, Modifier.weight(1f))
            }
            return@ScreenFrame
        }
        val state by states.getValue(segment)
        val entries = state.ids.mapNotNull { titles[it] }.let { list ->
            when (sort) {
                LibrarySort.LastWatched -> list
                LibrarySort.Title -> list.sortedBy { it.showTitle.lowercase(Locale.ROOT) }
                LibrarySort.Score -> list.sortedByDescending { it.score ?: 0u }
            }
        }
        PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 24.dp)) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "downloads") {
                DownloadsRow(storageLine(store.downloads.bytes(downloads)), inset = false)
            }
            item(span = { GridItemSpan(maxLineSpan) }, key = "lists") {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(LibraryModel.order) { each ->
                        val count = states.getValue(each).value.ids.size
                        SegmentChip(each, count, selected = each == segment) { segment = each }
                    }
                }
            }
            when (val list = state) {
                is ListState.Failed -> item(span = { GridItemSpan(maxLineSpan) }) { Quiet(list.reason) }
                ListState.Empty -> item(span = { GridItemSpan(maxLineSpan) }) {
                    Quiet("Nothing on your ${segment.label} list")
                }
                is ListState.Loaded -> items(entries, key = { it.id.toLong() }) { entry ->
                    PosterCell(entry, from = "library", detail = libraryDetail(entry), fresh = hasNew(entry))
                }
                else -> items(9) { AfterWait { PosterPlaceholder() } }
            }
        }
    }
}

/** "Ep 7 of 24", or "Ep 7" while the length is unannounced. */
private fun libraryDetail(entry: Entry): String? {
    val next = entry.progress?.next ?: return entry.genreLine
    val total = entry.episodes?.takeIf { it > 0u }
    return if (total != null) "Ep $next of $total" else "Ep $next"
}

/** Whether the next episode to watch is out: a water dot beside the line. */
private fun hasNew(entry: Entry): Boolean {
    val next = entry.progress?.next ?: return false
    return (entry.latestAired()?.episode ?: 0u) >= next
}

/** The Downloads row, which opens the Downloads page (A2), with what they take on this device. */
@Composable
private fun DownloadsRow(detail: String, inset: Boolean = true) {
    Row(
        Modifier
            .fillMaxWidth()
            .height(72.dp)
            .padding(horizontal = if (inset) 16.dp else 0.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Downloads, contentDescription = null)
        Column(Modifier.weight(1f)) {
            Text("Downloads", style = MaterialTheme.typography.titleMedium)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** A list's chip: its name and count; the selected one on primaryContainer with a check. */
@Composable
private fun SegmentChip(segment: LibrarySegment, count: Int, selected: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    Box(Modifier.height(48.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Row(
            Modifier
                .height(32.dp)
                .clip(shape)
                .then(if (selected) Modifier.background(colors.primaryContainer) else Modifier.border(1.dp, colors.outline, shape))
                .padding(start = if (selected) 8.dp else 16.dp, end = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val content = if (selected) colors.onPrimaryContainer else colors.onSurfaceVariant
            if (selected) Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
            Text(segment.label.replaceFirstChar { it.uppercase() }, style = MaterialTheme.typography.labelMedium, color = content)
            Text(
                "$count",
                style = MaterialTheme.typography.labelMedium.tabular().copy(fontWeight = FontWeight.Normal),
                color = content,
            )
        }
    }
}

/** The Library with no tracker signed in: Nagare, where the lists come from, and the one action. */
@Composable
private fun SignedOutLibrary(actions: AppActions, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(
        modifier.fillMaxWidth().padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Spacer(Modifier.weight(1f))
        Nagare(cell = 6)
        Text("Your lists come from AniList", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 24.dp))
        Text(
            "Connect AniList to see your Watching, Planning and Completed lists here.",
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 6.dp).width(320.dp),
        )
        Button(onClick = actions.connectAniList, modifier = Modifier.padding(top = 20.dp)) { Text("Connect AniList") }
        Spacer(Modifier.weight(1f))
        Text(
            LEGAL_LINE,
            style = MaterialTheme.typography.bodySmall,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(bottom = 24.dp),
        )
    }
}
