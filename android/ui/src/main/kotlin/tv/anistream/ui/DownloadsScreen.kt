// Downloads (AndroidDownloads): the queue by title, each title with its cover, how many episodes
// and how much they take, and each episode with where it stands and the one thing to do with it;
// the watched episodes still seeding; and, in the bar, what the downloads take and what is free.

package tv.anistream.ui

import tv.anistream.store.plainReason
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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.TextButton
import androidx.compose.foundation.background
import androidx.compose.ui.semantics.customActions
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
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import tv.anistream.i18n.Strings
import android.content.Context

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
internal fun storageWords(context: Context, used: Long, free: Long?): String =
    if (free == null) Strings.downloadsStorageUsed(context, Wording.bytes(context, used))
    else Strings.downloadsStorageUsedFree(context, Wording.bytes(context, used), Wording.bytes(context, free))

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun DownloadsScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val rows by store.downloads.rows.collectAsStateWithLifecycle()
    val seeding by store.downloads.seeding.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val loaded by store.downloads.loaded.collectAsStateWithLifecycle()
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
                        Text(stringResource(R.string.downloads_title), style = MaterialTheme.typography.titleLarge)
                        Text(
                            storageWords(LocalContext.current, store.downloads.bytes(rows), free),
                            style = MaterialTheme.typography.labelMedium.tabular(),
                            color = colors.onSurfaceVariant,
                        )
                    }
                },
                navigationIcon = { IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = stringResource(R.string.action_back)) } },
                actions = {
                    if (running) {
                        IconButton(onClick = store::pauseAllDownloads) { Icon(Icons.Pause, contentDescription = stringResource(R.string.downloads_pause_all_label)) }
                    } else if (paused) {
                        IconButton(onClick = store::resumeAllDownloads) { Icon(Icons.Resume, contentDescription = stringResource(R.string.downloads_resume_all_label)) }
                    }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.More, contentDescription = stringResource(R.string.action_more)) }
                        DropdownMenu(
                            expanded = menu,
                            onDismissRequest = { menu = false },
                            shape = MaterialTheme.shapes.extraSmall,
                            containerColor = colors.surfaceContainerHigh,
                        ) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.downloads_clear_finished)) },
                                enabled = rows.any { it.state == DownloadState.DONE },
                                onClick = {
                                    menu = false
                                    store.clearCompletedDownloads()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.action_open_settings)) },
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
        // Before the first answer, nothing for 600 ms and then the rows' places; never the empty
        // state ahead of the answer (A6).
        if (!loaded && rows.isEmpty()) {
            Column(Modifier.padding(top = padding.calculateTopPadding())) { AfterWait { DownloadPlaceholders() } }
            return@Scaffold
        }
        if (rows.isEmpty() && seeding.isEmpty()) {
            Column(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                tv.anistream.design.Nagare(cell = 5)
                Text(stringResource(R.string.downloads_empty_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                SheetText(stringResource(R.string.downloads_empty_detail), Modifier.padding(top = 4.dp))
            }
            return@Scaffold
        }
        val groups = DownloadsModel.groups(rows)
        Refreshable({ store.loadDownloads(force = true) }, top = padding.calculateTopPadding()) {
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
                        SettingsHeader(stringResource(R.string.downloads_seeding_after_watching), Modifier.padding(start = 0.dp))
                    }
                }
                items(seeding, key = { "seed-${it.name}" }, contentType = { "seed" }) { Box(itemMotion()) { SeedItem(it) } }
                item(key = "seeding-note", contentType = "note") {
                    Box(itemMotion()) { SettingsFootnote(stringResource(R.string.downloads_seeding_note)) }
                }
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
                pluralStringResource(R.plurals.downloads_group_summary, group.rows.size, group.rows.size, Wording.bytes(LocalContext.current, group.bytesOnDisk)),
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
    // Deleting a download and its file asks first, from the menu or a swipe.
    var deleting by remember { mutableStateOf(false) }
    val name = listOfNotNull(stringResource(R.string.downloads_episode_name, row.episode.toInt()), episode?.title?.takeIf { it.isNotBlank() }).joinToString(" · ")
    val progress = row.state == DownloadState.ACTIVE || row.state == DownloadState.PAUSED
    val hasFile = row.state == DownloadState.DONE || row.path != null
    val remove = { store.cancelDownload(row.id) }
    val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
    // The menu's items, for TalkBack too.
    val talkback = listOfNotNull(
        if (row.state == DownloadState.QUEUED) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_start_now)) { store.startDownloadNow(row.id); true } else null,
        if (hasFile) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_delete_one_title)) { deleting = true; true } else null,
        if (row.state != DownloadState.DONE) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_remove_from_list)) { remove(); true } else null,
    )
    Swipeable(
        start = null,
        // From the end: Delete for a file, which asks first, or Cancel for one not done.
        end = if (hasFile) SwipeAction(stringResource(R.string.action_delete), Icons.Clear) { deleting = true } else SwipeAction(stringResource(R.string.action_cancel), Icons.Clear, remove),
    ) {
    Box(Modifier.background(colors.surface)) {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = if (progress) 88.dp else 72.dp)
                .semantics { customActions = talkback }
                .combinedClickable(
                    onClick = { if (row.state == DownloadState.DONE) store.playDownload(row) },
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = {
                        haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                        menu = true
                    },
                )
                .padding(start = 72.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                // Queued by keep next rather than asked for, and let go of a day after it is watched.
                if (row.ahead) {
                    Text(stringResource(R.string.downloads_caption_ahead), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant, maxLines = 2)
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
                    DownloadState.DONE -> IconButton(onClick = { store.playDownload(row) }) { Icon(Icons.PlayRow, contentDescription = stringResource(R.string.downloads_play_episode, row.episode.toInt())) }
                    DownloadState.ACTIVE -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Pause, contentDescription = stringResource(R.string.downloads_pause_one)) }
                    DownloadState.PAUSED -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Resume, contentDescription = stringResource(R.string.downloads_resume_one)) }
                    DownloadState.QUEUED -> Row(verticalAlignment = Alignment.CenterVertically) {
                        QuietButton(stringResource(R.string.downloads_start_now)) { store.startDownloadNow(row.id) }
                        // Where Pause sat a moment ago: a started download is not dropped by a repeated tap, it asks.
                        IconButton(onClick = { if (hasFile || row.downloaded > 0uL) deleting = true else remove() }) {
                            Icon(Icons.Clear, contentDescription = stringResource(R.string.downloads_remove_from_queue), tint = colors.onSurfaceVariant)
                        }
                    }
                    DownloadState.FAILED -> QuietButton(stringResource(R.string.action_try_again)) { store.downloadEpisode(row.anilistId, row.episode) }
                }
            }
        }
        DropdownMenu(
            expanded = menu,
            onDismissRequest = { menu = false },
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = colors.surfaceContainerHigh,
        ) {
            if (row.state == DownloadState.QUEUED) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_start_now)) }, onClick = {
                    menu = false
                    store.startDownloadNow(row.id)
                })
            }
            if (hasFile) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_delete_one_title)) }, onClick = {
                    menu = false
                    deleting = true
                })
            }
            if (row.state != DownloadState.DONE) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_remove_from_list)) }, onClick = {
                    menu = false
                    remove()
                })
            }
        }
    }
    }
    if (deleting) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text(stringResource(R.string.downloads_delete_title, stringResource(R.string.downloads_episode_name, row.episode.toInt()))) },
            text = { Text(stringResource(R.string.downloads_delete_detail)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = false
                        store.deleteDownload(row.id)
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                ) { Text(stringResource(R.string.action_delete)) }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { deleting = false } },
            containerColor = colors.surfaceContainerHigh,
        )
    }
}

/** The rows' places before the downloads first answer: a show's head and two episodes. */
@Composable
private fun DownloadPlaceholders() {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(Modifier.heightIn(min = 72.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            tv.anistream.design.CoverPlate(null, Modifier.size(40.dp, 60.dp))
            Column(Modifier.weight(1f)) {
                TextPlaceholder(0.6f, 24.dp)
                TextPlaceholder(0.35f, 20.dp)
            }
        }
        repeat(2) {
            Column(Modifier.padding(start = 56.dp).heightIn(min = 72.dp), verticalArrangement = Arrangement.Center) {
                TextPlaceholder(0.7f, 24.dp)
                TextPlaceholder(0.45f, 20.dp)
            }
        }
    }
}

/** Where a download stands, in the words the board gives each state. */
@Composable
private fun StateLine(row: DownloadRow, secondsLeft: Long?, rate: Double?) {
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium.tabular()
    val context = LocalContext.current
    val sizes = Strings.downloadsLiveProgress(context, Wording.bytes(context, row.downloaded.toLong()), Wording.bytes(context, row.total.toLong()))
    when (row.state) {
        DownloadState.DONE -> Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(18.dp))
            Text(stringResource(R.string.downloads_caption_done_size, Wording.bytes(context, row.bytesOnDisk.toLong())), style = style, color = colors.onSurfaceVariant)
        }
        DownloadState.ACTIVE -> Text(
            listOfNotNull(
                if (row.total > 0u) sizes else Wording.bytes(context, row.downloaded.toLong()),
                rate?.takeIf { it >= 1 }?.let { Strings.downloadsRate(context, Wording.bytes(context, it.toLong())) },
                secondsLeft?.let { Strings.downloadsTimeLeft(context, Wording.countdown(context, it)) },
            ).joinToString(" · "), // i18n-skip
            style = style,
            color = colors.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        DownloadState.PAUSED -> Text(stringResource(R.string.downloads_caption_paused, sizes), style = style, color = colors.onSurfaceVariant)
        DownloadState.QUEUED -> Text(stringResource(R.string.downloads_caption_queued_slot), style = style, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        // The dot is the error's; the words are haze, the first clause in plain words. The whole
        // error goes to the bug report from the notice the engine posted.
        DownloadState.FAILED -> Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            StateDot(colors.error)
            Text(
                row.error?.let { stringResource(R.string.downloads_caption_failed_why, plainReason(LocalContext.current, it)) } ?: stringResource(R.string.downloads_caption_failed),
                style = style,
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * A watched episode still seeding, as the board says it: "450 MB · 0.2 of 1.0 · 18h left · 90 MB
 * up". The size is what was downloaded, which the ratio is of.
 */
@Composable
private fun SeedItem(row: SeedRow) {
    Column(Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.Center) {
        Text(row.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        val context = LocalContext.current
        Text(
            listOfNotNull(
                row.ratio.takeIf { it >= 0.01 }?.let { Wording.bytes(context, (row.uploadedBytes.toDouble() / it).toLong()) },
                Strings.downloadsRatioOf(context, String.format(Locale.getDefault(), "%.1f", row.ratio), String.format(Locale.getDefault(), "%.1f", row.goal)),
                Strings.downloadsTimeLeft(context, Wording.countdown(context, row.leftSecs.toLong())),
                Strings.downloadsUploaded(context, Wording.bytes(context, row.uploadedBytes.toLong())),
            ).joinToString(" · "), // i18n-skip
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
                    DownloadState.DONE -> stringResource(R.string.episode_download_done)
                    DownloadState.ACTIVE, DownloadState.QUEUED -> stringResource(R.string.downloads_caption_running_plain)
                    DownloadState.PAUSED -> stringResource(R.string.episode_menu_download_paused)
                    else -> stringResource(R.string.episode_menu_download)
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
