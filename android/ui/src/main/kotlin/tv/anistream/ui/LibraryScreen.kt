// The Library: Downloads, the tracker's lists as chips with their counts, the covers of the one
// chosen; or, signed out, where the lists come from.

package tv.anistream.ui

import androidx.compose.ui.platform.LocalContext
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
import androidx.compose.foundation.layout.heightIn
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
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.store.Wording

/** How the Library orders a list. */
private enum class LibrarySort(val labelRes: Int) {
    LastWatched(R.string.library_sort_last_watched),
    Title(R.string.library_sort_title),
    Score(R.string.library_sort_score),
}

/** "3.4 GB on this device", or that nothing is. */
internal fun storageLine(context: Context, bytes: Long): String =
    if (bytes <= 0) Strings.libraryStorageNone(context) else Strings.libraryStorageOnDevice(context, Wording.bytes(context, bytes))

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
        title = stringResource(R.string.nav_library),
        account = Account(user, actions.openSettings),
        actions = {
            if (!signedOut) {
                Box {
                    androidx.compose.material3.IconButton(onClick = { sorting = true }) {
                        Icon(Icons.Sort, contentDescription = stringResource(R.string.sort_button, stringResource(sort.labelRes)))
                    }
                    Menu(expanded = sorting, onDismissRequest = { sorting = false }) {
                        for (each in LibrarySort.entries) {
                            DropdownMenuItem(
                                text = { Text(stringResource(each.labelRes)) },
                                leadingIcon = {
                                    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                        if (each == sort) Icon(Icons.Check, contentDescription = stringResource(R.string.library_sort_current))
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
                DownloadsRow(storageLine(LocalContext.current, store.downloads.bytes(downloads)))
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
        val offline by store.offline.collectAsStateWithLifecycle()
        // Pulled, the list loads again and the tracker syncs.
        Refreshable({ store.loadLibrary(segment); store.syncNow() }, top = padding.calculateTopPadding()) {
        PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 24.dp)) {
            // Every item moves with the rest, the rows above the posters too, so switching lists
            // never slides a poster over a line that snapped into place.
            item(span = { GridItemSpan(maxLineSpan) }, key = "downloads", contentType = "downloads") {
                DownloadsRow(storageLine(LocalContext.current, store.downloads.bytes(downloads)), inset = false, modifier = itemMotion())
            }
            item(span = { GridItemSpan(maxLineSpan) }, key = "lists", contentType = "lists") {
                LazyRow(itemMotion(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(LibraryModel.order, key = { it.name }) { each ->
                        val count = states.getValue(each).value.ids.size
                        SegmentChip(each, count, selected = each == segment) { segment = each }
                    }
                }
            }
            when (val list = state) {
                is ListState.Failed -> item(span = { GridItemSpan(maxLineSpan) }, key = "quiet", contentType = "quiet") {
                    Box(itemMotion()) { FailedState(stringResource(R.string.library_list_load_failed, segment.label(LocalContext.current)), list.reason, retry = { store.loadLibrary(segment) }) }
                }
                ListState.Empty -> item(span = { GridItemSpan(maxLineSpan) }, key = "quiet", contentType = "quiet") {
                    Box(itemMotion()) { Quiet(stringResource(R.string.library_list_empty, segment.label(LocalContext.current))) }
                }
                is ListState.Loaded -> items(entries, key = { it.id.toLong() }, contentType = { "poster" }) { entry ->
                    PosterCell(entry, itemMotion(), from = "library", detail = libraryDetail(LocalContext.current, entry), fresh = hasNew(entry))
                }
                else -> if (offline) {
                    item(span = { GridItemSpan(maxLineSpan) }, key = "offline", contentType = "quiet") { Box(itemMotion()) { AfterWait { OfflineState() } } }
                } else {
                    items(9, key = { "placeholder-$it" }, contentType = { "placeholder" }) { Box(itemMotion()) { AfterWait { PosterPlaceholder() } } }
                }
            }
        }
        }
    }
}

/** "Ep 7 of 24", or "Ep 7" while the length is unannounced. */
private fun libraryDetail(context: Context, entry: Entry): String? {
    val next = entry.progress?.next ?: return entry.genreLine
    val total = entry.episodes?.takeIf { it > 0u }
    return if (total != null) Strings.libraryDetailOf(context, next.toInt(), total.toInt()) else Strings.libraryDetailNext(context, next.toInt())
}

/** Whether the next episode to watch is out: a water dot beside the line. */
private fun hasNew(entry: Entry): Boolean {
    val next = entry.progress?.next ?: return false
    return (entry.latestAired()?.episode ?: 0u) >= next
}

/** The Downloads row, which opens the Downloads page (A2), with what they take on this device. */
@Composable
private fun DownloadsRow(detail: String, inset: Boolean = true, modifier: Modifier = Modifier) {
    val navigator = LocalNavigator.current
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clickable { navigator.open(DOWNLOADS_ROUTE) }
            .padding(horizontal = if (inset) 16.dp else 0.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Downloads, contentDescription = null)
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.downloads_title), style = MaterialTheme.typography.titleMedium)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** A list's chip: its name and count; the selected one on primaryContainer with a check. */
@Composable
private fun SegmentChip(segment: LibrarySegment, count: Int, selected: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    Box(Modifier.heightIn(min = 48.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Row(
            Modifier
                .heightIn(min = 32.dp)
                .clip(shape)
                .then(if (selected) Modifier.background(colors.primaryContainer) else Modifier.border(1.dp, colors.outline, shape))
                .padding(start = if (selected) 8.dp else 16.dp, end = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val content = if (selected) colors.onPrimaryContainer else colors.onSurfaceVariant
            if (selected) Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
            Text(segment.label(LocalContext.current).replaceFirstChar { it.uppercase() }, style = MaterialTheme.typography.labelMedium, color = content)
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
        Nagare(cell = 5)
        Text(stringResource(R.string.library_signed_out_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 24.dp))
        Text(
            stringResource(R.string.library_signed_out_detail),
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 6.dp).width(320.dp),
        )
        Button(onClick = actions.connectAniList, modifier = Modifier.padding(top = 20.dp)) { Text(stringResource(R.string.library_signed_out_action)) }
        Spacer(Modifier.weight(1f))
        Text(
            stringResource(R.string.legal_hosts_no_content),
            style = MaterialTheme.typography.bodySmall,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(bottom = 24.dp),
        )
    }
}
