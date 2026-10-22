// Downloads (AndroidDownloads): the queue by title, each title with its cover, how many episodes
// and how much they take, and each episode with where it stands and the one thing to do with it;
// the watched episodes still seeding; and, in the bar, what the downloads take and what is free.

package tv.anistream.ui

import android.os.StatFs
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import java.util.Locale
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.SeedRow
import tv.anistream.store.DownloadGroup
import tv.anistream.store.DownloadsModel
import tv.anistream.store.Wording
import tv.anistream.store.bytesOnDisk

internal const val DOWNLOADS_ROUTE = "downloads"

/** The Downloads screen's route. */
internal fun NavGraphBuilder.downloadPages(actions: AppActions) {
    composable(DOWNLOADS_ROUTE) { DownloadsScreen() }
}

/** The bytes free where downloads are kept: anistream's own space on the device. */
@Composable
internal fun freeBytes(): Long? {
    val context = LocalContext.current
    return remember(context) {
        runCatching { StatFs((context.getExternalFilesDir(null) ?: context.filesDir).path).availableBytes }.getOrNull()
    }
}

/** "3.4 GB used · 38 GB free". */
internal fun storageWords(used: Long, free: Long?): String =
    listOfNotNull("${Wording.bytes(used)} used", free?.let { "${Wording.bytes(it)} free" }).joinToString(" · ")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun DownloadsScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val rows by store.downloads.rows.collectAsStateWithLifecycle()
    val seeding by store.downloads.seeding.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val free = freeBytes()
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { store.loadDownloads(force = true) }
    // The titles' episode names, for the rows that have none of their own.
    LaunchedEffect(rows.map { it.anilistId }.toSet()) {
        for (id in rows.map { it.anilistId }.distinct()) store.loadEpisodes(id)
    }
    val running = rows.any { it.state == DownloadState.ACTIVE || it.state == DownloadState.QUEUED }
    val paused = rows.any { it.state == DownloadState.PAUSED }
    val colors = MaterialTheme.colorScheme
    Scaffold(
        containerColor = colors.surface,
        contentWindowInsets = WindowInsets(0),
        topBar = {
            TopAppBar(
                title = {
                    Column(Modifier.padding(start = 4.dp)) {
                        Text("Downloads", style = MaterialTheme.typography.titleLarge)
                        Text(
                            storageWords(store.downloads.bytes(rows), free),
                            style = MaterialTheme.typography.labelMedium.tabular(),
                            color = colors.onSurfaceVariant,
                        )
                    }
                },
                navigationIcon = { IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = "Back") } },
                actions = {
                    if (running) {
                        IconButton(onClick = store::pauseAllDownloads) { Icon(Icons.Pause, contentDescription = "Pause all downloads") }
                    } else if (paused) {
                        IconButton(onClick = store::resumeAllDownloads) { Icon(Icons.Resume, contentDescription = "Resume all downloads") }
                    }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.More, contentDescription = "More options") }
                        DropdownMenu(
                            expanded = menu,
                            onDismissRequest = { menu = false },
                            shape = MaterialTheme.shapes.extraSmall,
                            containerColor = colors.surfaceContainerHigh,
                        ) {
                            DropdownMenuItem(
                                text = { Text("Clear finished from the list") },
                                enabled = rows.any { it.state == DownloadState.DONE },
                                onClick = {
                                    menu = false
                                    store.clearCompletedDownloads()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text("Download settings") },
                                onClick = {
                                    menu = false
                                    navigator.openSettings(SettingsPane.Downloads)
                                },
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = colors.surface, scrolledContainerColor = colors.surfaceContainer),
            )
        },
    ) { padding ->
        if (rows.isEmpty() && seeding.isEmpty()) {
            Column(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                tv.anistream.design.Nagare(cell = 5)
                Text("No downloads yet", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                SheetText("An episode downloaded from its title page plays here without a connection.", Modifier.padding(top = 4.dp))
            }
            return@Scaffold
        }
        val groups = DownloadsModel.groups(rows)
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 48.dp),
        ) {
            // Rows come and go on standard, every item with the rest (motion-android.md's list
            // insert and remove), so a cancelled or cleared download never leaves a gap that snaps.
            groups.forEachIndexed { index, group ->
                if (index > 0) item(key = "rule-${group.id}", contentType = "rule") { HorizontalDivider(itemMotion().padding(vertical = 8.dp), color = colors.outlineVariant) }
                item(key = "title-${group.id}", contentType = "group") { Box(itemMotion()) { GroupHeader(group, titles[group.id]) } }
                // TODO(engine): AndroidDownloads puts the show's own "Keep next 3 episodes ·
                // Higher quality" row here, with a switch, under its title. Keep next and the
                // download quality are Settings-wide; the row waits on per-show settings for both
                // (`ShowSettingKey` has neither yet).
                items(group.rows, key = { "row-${it.id}" }, contentType = { "download" }) { row ->
                    Box(itemMotion()) { DownloadItem(row, episodes[row.anilistId]?.firstOrNull { it.number == row.episode }) }
                }
            }
            if (seeding.isNotEmpty()) {
                item(key = "seeding", contentType = "seeding") {
                    Column(itemMotion()) {
                        if (groups.isNotEmpty()) HorizontalDivider(Modifier.padding(vertical = 8.dp), color = colors.outlineVariant)
                        SettingsHeader("Seeding after watching", Modifier.padding(start = 0.dp))
                    }
                }
                items(seeding, key = { "seed-${it.name}" }, contentType = { "seed" }) { Box(itemMotion()) { SeedItem(it) } }
                item(key = "seeding-note", contentType = "note") {
                    Box(itemMotion()) { SettingsFootnote("Watched episodes seed while anistream is open, until the ratio or the time limit set in Sources.") }
                }
            }
        }
    }
}

/** A title's head: its cover, its title, and how many episodes and how much. */
@Composable
private fun GroupHeader(group: DownloadGroup, entry: Entry?) {
    Row(
        Modifier.fillMaxWidth().height(72.dp).padding(horizontal = 16.dp).semantics(mergeDescendants = true) { heading() },
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (entry != null) {
            CoverArt(entry, 40.dp, Modifier.size(40.dp, 60.dp))
        } else {
            tv.anistream.design.CoverPlate(null, Modifier.size(40.dp, 60.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(entry?.showTitle ?: group.title, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                "${Wording.plural(group.rows.size, "episode")} · ${Wording.bytes(group.bytesOnDisk)}",
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * An episode: "Episode 8 · A Town Without Night", where it stands, a bar while it runs or waits
 * paused, and its one action: play, pause, resume, remove or retry. A long press offers deleting
 * it, or taking it off the list.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DownloadItem(row: DownloadRow, episode: EpisodeRow?) {
    val store = LocalAppStore.current
    val rates by store.downloads.rates.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    val name = listOfNotNull("Episode ${row.episode}", episode?.title?.takeIf { it.isNotBlank() }).joinToString(" · ")
    val progress = row.state == DownloadState.ACTIVE || row.state == DownloadState.PAUSED
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = if (progress) 88.dp else 72.dp)
                .combinedClickable(onClick = { if (row.state == DownloadState.DONE) store.playDownload(row) }, onLongClick = { menu = true })
                .padding(start = 72.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                // Queued by keep next rather than asked for, and let go of a day after it is watched.
                if (row.ahead) {
                    Text("Downloaded ahead · Removed a day after you watch it", style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant, maxLines = 2)
                }
                StateLine(row, store.downloads.secondsLeft(row, rates), rates[row.id])
                if (progress) {
                    LinearProgressIndicator(
                        progress = { row.fraction.toFloat().coerceIn(0f, 1f) },
                        modifier = Modifier.padding(top = 8.dp).fillMaxWidth().height(4.dp),
                        color = if (row.state == DownloadState.ACTIVE) colors.secondary else colors.onSurfaceVariant,
                        trackColor = colors.outline,
                        gapSize = 4.dp,
                        drawStopIndicator = {},
                    )
                }
            }
            Box(Modifier.heightIn(min = 48.dp).widthIn(min = 48.dp), contentAlignment = Alignment.Center) {
                when (row.state) {
                    DownloadState.DONE -> IconButton(onClick = { store.playDownload(row) }) { Icon(Icons.PlayRow, contentDescription = "Play episode ${row.episode}") }
                    DownloadState.ACTIVE -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Pause, contentDescription = "Pause download") }
                    DownloadState.PAUSED -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Resume, contentDescription = "Resume download") }
                    DownloadState.QUEUED -> IconButton(onClick = { store.cancelDownload(row.id) }) {
                        Icon(Icons.Clear, contentDescription = "Remove from queue", tint = colors.onSurfaceVariant)
                    }
                    DownloadState.FAILED -> QuietButton("Retry") { store.downloadEpisode(row.anilistId, row.episode) }
                }
            }
        }
        DropdownMenu(
            expanded = menu,
            onDismissRequest = { menu = false },
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = colors.surfaceContainerHigh,
        ) {
            if (row.state == DownloadState.DONE || row.path != null) {
                DropdownMenuItem(text = { Text("Delete download") }, onClick = {
                    menu = false
                    store.deleteDownload(row.id)
                })
            }
            if (row.state != DownloadState.DONE) {
                DropdownMenuItem(text = { Text("Remove from the list") }, onClick = {
                    menu = false
                    store.cancelDownload(row.id)
                })
            }
        }
    }
}

/** Where a download stands, in the words the board gives each state. */
@Composable
private fun StateLine(row: DownloadRow, secondsLeft: Long?, rate: Double?) {
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium.tabular()
    val sizes = "${Wording.bytes(row.downloaded.toLong())} of ${Wording.bytes(row.total.toLong())}"
    when (row.state) {
        DownloadState.DONE -> Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(18.dp))
            Text("Downloaded · ${Wording.bytes(row.bytesOnDisk.toLong())}", style = style, color = colors.onSurfaceVariant)
        }
        DownloadState.ACTIVE -> Text(
            listOfNotNull(
                if (row.total > 0u) sizes else Wording.bytes(row.downloaded.toLong()),
                rate?.takeIf { it >= 1 }?.let { "${Wording.bytes(it.toLong())}/s" },
                secondsLeft?.let { "${countdownWords(it)} left" },
            ).joinToString(" · "),
            style = style,
            color = colors.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        DownloadState.PAUSED -> Text("Paused at $sizes", style = style, color = colors.onSurfaceVariant)
        DownloadState.QUEUED -> Text("Queued · starts when a download slot is free", style = style, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        DownloadState.FAILED -> Text(
            "Failed" + (row.error?.let { ": $it" } ?: ""),
            style = style,
            color = colors.error,
        )
    }
}

/** "2m 11s", "1h 4m", "40s": how long a download has left. */
internal fun countdownWords(seconds: Long): String = when {
    seconds >= 3_600 -> "${seconds / 3_600}h ${(seconds % 3_600) / 60}m"
    seconds >= 60 -> "${seconds / 60}m ${seconds % 60}s"
    else -> "${seconds}s"
}

/** A watched episode still seeding: how much has gone up, toward what ratio, and for how long more. */
@Composable
private fun SeedItem(row: SeedRow) {
    Column(Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.Center) {
        Text(row.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(
            listOf(
                "${Wording.bytes(row.uploadedBytes.toLong())} up",
                String.format(Locale.ROOT, "ratio %.1f of %.1f", row.ratio, row.goal),
                "${countdownWords(row.leftSecs.toLong())} left",
            ).joinToString(" · "),
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * Download, in an episode's long-press menu on its title page: queued at once; Downloaded or
 * Downloading once the queue has it.
 */
@Composable
internal fun DownloadMenuItem(entry: Entry, episode: String, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val rows by store.downloads.rows.collectAsStateWithLifecycle()
    val queued = rows.firstOrNull { it.anilistId == entry.id && it.episode == episode }
    DropdownMenuItem(
        text = {
            Text(
                when (queued?.state) {
                    DownloadState.DONE -> "Downloaded"
                    DownloadState.ACTIVE, DownloadState.QUEUED -> "Downloading"
                    DownloadState.PAUSED -> "Download paused"
                    else -> "Download"
                },
            )
        },
        leadingIcon = { Icon(Icons.Downloads, contentDescription = null) },
        enabled = queued == null || queued.state == DownloadState.FAILED,
        onClick = {
            onDismiss()
            store.downloadEpisode(entry.id, episode)
        },
    )
}
