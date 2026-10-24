// The title page: the banner header, the title, its metadata in caps and the action row with the
// amber action focused; then the episodes, focus landing on the next one to play; Related; and the
// details. Back returns to the opener.

package tv.anistream.tv.screens

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import kotlinx.coroutines.launch
import tv.anistream.design.CoverPlate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.AppStore
import tv.anistream.store.label
import tv.anistream.tv.PlayAction
import tv.anistream.tv.cardLine
import tv.anistream.tv.cardTitle
import tv.anistream.tv.metadataLine
import tv.anistream.tv.synopsisWithoutSpoilers
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.ArtCard
import tv.anistream.tv.ui.ArtImage
import tv.anistream.tv.ui.BrowseRow
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.CardText
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.HeldScroll
import tv.anistream.tv.ui.HeroHeader
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.PosterCard
import tv.anistream.tv.ui.ProgressEdge
import tv.anistream.tv.ui.ReleaseList
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.TextBar
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.pixels
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.settlesAt

@Composable
fun TitleScreen(
    id: AnilistId,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
    onPlayFromStart: (AnilistId, UInt) -> Unit = onPlay,
) {
    val store = LocalAppStore.current
    LaunchedEffect(id) {
        store.loadDetail(id)
        store.loadEpisodes(id)
        store.loadDub(id)
    }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val dubs by store.titles.dubLines.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val entry = entries[id]
    val rows = tables[id]
    val spoilers = settings?.let { store.settings.isOn(SettingKey.SPOILER_PROTECTION, it) } ?: true

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val rowTop = with(LocalDensity.current) { 48.dp.roundToPx() }

    if (entry == null) {
        if (placeholdersAfterWait(true)) TitlePlaceholder()
        return
    }
    val action = remember(entry) { PlayAction.of(entry) }
    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            item(key = "header", contentType = "header") {
                Header(entry, rows, action, memory, onPlay, onList = { store.setStatus(id, LibrarySegment.PLANNING) }, Modifier.onFocusChanged {
                    if (it.hasFocus) scope.launch { list.animateScrollToItem(0) }
                })
            }
            if (rows == null || rows.isNotEmpty()) {
                item(key = "episodes", contentType = "row") {
                    Episodes(entry, rows, dubs[id], action, spoilers, memory, onPlay, onPlayFromStart, Modifier.settlesAt(list, scope, "episodes", rowTop))
                }
            }
            val related = entry.detail?.relations.orEmpty()
            if (related.isNotEmpty()) {
                item(key = "related", contentType = "row") {
                    BrowseRow("Related", modifier = Modifier.settlesAt(list, scope, "related", rowTop)) {
                        items(related, key = { it.id.toLong() }, contentType = { "poster" }) { linked ->
                            PosterCard(
                                linked.title,
                                linked.coverUrl,
                                linked.coverColor,
                                onClick = { onOpen(linked.id) },
                                line = linked.relation?.name?.lowercase()?.replace('_', ' ')?.replaceFirstChar { it.uppercase() },
                                modifier = Modifier.remembered(memory, "related/${linked.id}"),
                            )
                        }
                    }
                }
            }
            item(key = "details", contentType = "details") {
                Details(entry, memory, Modifier.settlesAt(list, scope, "details", rowTop))
            }
        }
    }
    EnterFocus(memory, entry = id)
}


@Composable
private fun Header(
    entry: Entry,
    rows: List<EpisodeRow>?,
    action: PlayAction,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onList: () -> Unit,
    modifier: Modifier,
) {
    HeroHeader(entry, rows, modifier) {
        Spacer(Modifier.height(6.dp))
        Text(entry.metadataLine, style = MaterialTheme.typography.labelSmall, color = ObiSilk.Haze, maxLines = 1)
        Spacer(Modifier.height(16.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            AmberButton(
                onClick = { onPlay(entry.id, action.episode) },
                // Still focusable while it waits for the episode, so it can say when it is out.
                enabled = action.playable,
                modifier = Modifier.firstFocus(memory, "play"),
            ) { ButtonLabel(action.label()) }
            Spacer(Modifier.width(16.dp))
            val status = entry.listStatus
            TonalButton(
                onClick = { if (status == null) onList() },
                icon = if (status == null) TvIcons.Library else TvIcons.Check,
                modifier = Modifier.remembered(memory, "status"),
            ) { ButtonLabel(status?.label?.replaceFirstChar { it.uppercase() } ?: "Add to list") }
        }
    }
}

@Composable
private fun Episodes(
    entry: Entry,
    rows: List<EpisodeRow>?,
    dub: String?,
    action: PlayAction,
    spoilers: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
    modifier: Modifier,
) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    val next = rows?.indexOfFirst { it.number == action.episode.toString() }?.takeIf { it >= 0 } ?: 0
    val state = rememberLazyListState(initialFirstVisibleItemIndex = next)
    val nextRequester = remember { FocusRequester() }
    BrowseRow("Episodes", modifier = modifier, subtitle = dub, state = state, fallback = nextRequester) {
        if (rows == null) {
            items(PLACEHOLDER_CARDS) { tv.anistream.tv.ui.CardPlaceholder(Canvas.CardWidth, Canvas.CardHeight) }
            return@BrowseRow
        }
        items(rows, key = { it.number }, contentType = { "episode" }) { row ->
            val index = rows.indexOf(row)
            val playsNext = index == next
            EpisodeCard(
                row = row,
                entry = entry,
                line = row.cardLine(entry.resume, playsNext),
                progress = if (playsNext) entry.resume?.fraction?.toFloat() else null,
                // Unwatched stills after the next one keep their colours and none of their detail.
                // Only an episode's own still can spoil; the show's banner in its place cannot.
                spoiler = spoilers && index > next && !row.completed && row.thumbnail != null,
                onClick = { row.number.toUIntOrNull()?.let { onPlay(entry.id, it) } },
                onLongClick = { actions?.open(episodeSheet(store, entry, rows, row, onPlayFromStart)) },
                modifier = Modifier
                    .then(if (playsNext) Modifier.focusRequester(nextRequester) else Modifier)
                    .remembered(memory, "episode/${row.number}"),
            )
        }
    }
}

@Composable
private fun EpisodeCard(
    row: EpisodeRow,
    entry: Entry,
    line: String?,
    progress: Float?,
    spoiler: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier,
) {
    Column(Modifier.width(Canvas.CardWidth)) {
        ArtCard(onClick, Canvas.CardWidth, Canvas.CardHeight, modifier, onLongClick) {
            ArtImage(row.thumbnail ?: entry.bannerUrl, CoverPlate.parse(entry.coverColor), pixels(Canvas.CardWidth), Modifier.fillMaxSize(), spoiler = spoiler)
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
            if (row.completed) {
                Box(
                    Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).background(ObiSilk.Ground.copy(alpha = 0.72f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(TvIcons.Check, contentDescription = "Watched", tint = ObiSilk.Water, modifier = Modifier.size(16.dp))
                }
            }
        }
        CardText(row.cardTitle, line, titleColor = if (row.completed) ObiSilk.Haze else ObiSilk.Silk)
    }
}

/** The synopsis (six lines at most), then studio, genres and source. Focusable, so it scrolls in. */
@Composable
private fun Details(entry: Entry, memory: FocusMemory, modifier: Modifier) {
    Column(modifier.padding(start = Canvas.Keyline - 16.dp, end = Canvas.SafeX)) {
        Text(
            "Details",
            style = MaterialTheme.typography.titleMedium,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 16.dp, bottom = Canvas.HeaderGap),
        )
        Surface(
            onClick = {},
            modifier = Modifier.widthIn(max = 640.dp).remembered(memory, "details"),
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
            colors = ClickableSurfaceDefaults.colors(
                containerColor = androidx.compose.ui.graphics.Color.Transparent,
                contentColor = ObiSilk.Silk,
                focusedContainerColor = ObiSilk.Raised,
                focusedContentColor = ObiSilk.Silk,
            ),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
        ) {
            Column(Modifier.padding(16.dp)) {
                val synopsis = synopsisWithoutSpoilers(entry.synopsis)
                if (synopsis.isNotEmpty()) {
                    Text(synopsis, style = MaterialTheme.typography.bodyLarge, maxLines = 6, overflow = TextOverflow.Ellipsis)
                    Spacer(Modifier.height(16.dp))
                }
                listOfNotNull(
                    entry.studio?.let { "Studio" to it },
                    entry.genres.takeIf { it.isNotEmpty() }?.let { "Genres" to it.joinToString(", ") },
                    entry.source?.let { "Source" to it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.uppercase() } },
                ).forEach { (label, value) ->
                    Row(Modifier.padding(vertical = 2.dp)) {
                        Text(label, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.width(96.dp))
                        Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 2)
                    }
                }
            }
        }
    }
}

@Composable
private fun TitlePlaceholder() {
    Column(Modifier.fillMaxWidth()) {
        Spacer(Modifier.height(212.dp))
        Column(Modifier.padding(start = Canvas.Keyline)) {
            TextBar(360.dp, height = 32.dp)
            Spacer(Modifier.height(14.dp))
            TextBar(280.dp)
        }
    }
}

private const val PLACEHOLDER_CARDS = 5

/**
 * An episode's actions: play it from the start, mark it watched or every episode up to it, or
 * choose the release it plays from. One AniList alone lists has nothing to play.
 */
fun episodeSheet(
    store: AppStore,
    entry: Entry,
    rows: List<EpisodeRow>,
    row: EpisodeRow,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
): ActionSheet {
    val upTo = rows.takeWhile { it.number != row.number }.map { it.number } + row.number
    val number = row.number.toUIntOrNull()?.takeIf { !row.metadataOnly }
    val title = "${entry.title} · ${row.cardTitle}"
    return ActionSheet(
        title,
        listOfNotNull(
            number?.let { CardAction("Play from the start") { onPlayFromStart(entry.id, it) } },
            if (row.completed) {
                CardAction("Mark unwatched") { store.setWatched(entry.id, listOf(row.number), watched = false) }
            } else {
                CardAction("Mark watched") { store.setWatched(entry.id, listOf(row.number)) }
            },
            CardAction("Mark watched up to here") { store.setWatched(entry.id, upTo) },
            number?.let {
                CardAction("Choose source", then = ActionSheet(title, body = { EpisodeReleases(store, entry.id, row.number) }))
            },
        ),
    )
}

/** The episode's releases, listed as the panel opens; choosing one plays it. */
@Composable
private fun EpisodeReleases(store: AppStore, id: AnilistId, episode: String) {
    val actions = LocalActions.current
    val choices by store.sources.collectAsStateWithLifecycle()
    LaunchedEffect(id, episode) { store.loadSources(id, episode) }
    DisposableEffect(id, episode) { onDispose { store.closeSources() } }
    ReleaseList(
        choices?.takeIf { it.id == id && it.episode == episode }?.candidates,
        onPick = { candidate ->
            actions?.close()
            store.playSource(id, episode, candidate)
        },
    )
}
