// The title page: the banner header, the title, its metadata in caps and the action row with the
// amber action focused; then the franchise's seasons as a row of tabs, when it has more than one;
// the episodes, focus landing on the next one to play; Related; and the details. Back returns to
// the opener, whichever season the tabs went to.

package tv.anistream.tv.screens

import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.R
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import tv.anistream.i18n.Strings
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.semantics.clearAndSetSemantics
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
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.AppStore
import tv.anistream.store.label
import tv.anistream.store.line
import tv.anistream.tv.PlayAction
import tv.anistream.tv.cardLine
import tv.anistream.tv.cardTitle
import tv.anistream.tv.metadataLine
import tv.anistream.tv.synopsisWithoutSpoilers
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.DiscussionPanel
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
import tv.anistream.tv.ui.StateBlock
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TonalIconButton
import tv.anistream.tv.ui.cardLabel
import tv.anistream.tv.ui.focusSoon
import tv.anistream.tv.ui.showSettingsSheet
import tv.anistream.tv.ui.sourcePickerSheet
import tv.anistream.tv.ui.statusSheet
import tv.anistream.tv.ui.swapFade
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.TvTab
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
    onAddStore: () -> Unit = {},
    /** Another season's page in this one's place. */
    onSeason: (AnilistId) -> Unit = onOpen,
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
    val failed by store.titles.failed.collectAsStateWithLifecycle()
    // Another season comes in place: the one showing stays until the new one's detail arrives,
    // so the page never blanks, the list keeps its place and focus stays on the chip pressed.
    var shownId by remember { mutableStateOf(id) }
    if (shownId != id && (entries[id]?.detail != null || id in failed)) shownId = id
    val switching = shownId != id
    val entry = entries[shownId]
    val rows = tables[shownId]
    val spoilers = settings?.let { store.settings.isOn(SettingKey.SPOILER_PROTECTION, it) } ?: true

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val rowTop = with(LocalDensity.current) { 48.dp.roundToPx() }

    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    // No source can play anything: the amber action leads to a plugin store instead.
    val noSources = providersLoaded && providers.none { it.usable }
    val waiting = placeholdersAfterWait(entry == null && id !in failed)
    if (entry == null) {
        if (id in failed) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                StateBlock(
                    stringResource(R.string.title_load_failed),
                    action = stringResource(R.string.action_try_again),
                    onAction = { store.loadDetail(id) },
                    actionModifier = Modifier.firstFocus(memory, "retry"),
                )
            }
            EnterFocus(memory, entry = "failed/$id")
        } else if (waiting) {
            TitlePlaceholder()
        }
        return
    }
    val action = remember(entry, rows) { PlayAction.of(entry, rows = rows) }
    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            item(key = "header", contentType = "header") {
                Header(entry, rows, action, noSources, memory, onPlay, onAddStore, swapFade().onFocusChanged {
                    if (it.hasFocus) scope.launch { list.animateScrollToItem(0) }
                })
            }
            val seasons = entry.detail?.seasons.orEmpty()
            if (seasons.size > 1) {
                item(key = "seasons", contentType = "seasons") {
                    // The chip pressed shows chosen at once, before its season arrives.
                    Seasons(id, seasons, memory, onSeason, swapFade().settlesAt(list, scope, "seasons", rowTop))
                }
            }
            if (rows == null || rows.isNotEmpty()) {
                item(key = "episodes", contentType = "row") {
                    // Each season's row starts at its own next episode.
                    key(shownId) {
                        Episodes(entry, rows, if (switching) stringResource(R.string.state_loading) else dubs[shownId]?.line(LocalContext.current), action, spoilers, memory, onPlay, onPlayFromStart, swapFade().settlesAt(list, scope, "episodes", rowTop))
                    }
                }
            }
            val related = entry.detail?.relations.orEmpty()
            if (related.isNotEmpty()) {
                item(key = "related", contentType = "row") {
                    BrowseRow(stringResource(R.string.title_related), modifier = swapFade().settlesAt(list, scope, "related", rowTop)) {
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
                Details(entry, memory, swapFade().settlesAt(list, scope, "details", rowTop))
            }
        }
    }
    // Once per page: a season coming in place keeps the focus it has.
    EnterFocus(memory)
}


@Composable
private fun Header(
    entry: Entry,
    rows: List<EpisodeRow>?,
    action: PlayAction,
    noSources: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onAddStore: () -> Unit,
    modifier: Modifier,
) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    val context = LocalContext.current
    HeroHeader(entry, rows, modifier) {
        Spacer(Modifier.height(6.dp))
        Text(entry.metadataLine(context), style = MaterialTheme.typography.labelSmall, color = ObiSilk.Haze, maxLines = 1)
        Spacer(Modifier.height(16.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (noSources) {
                AmberButton(onClick = onAddStore, icon = null, modifier = Modifier.firstFocus(memory, "play")) { ButtonLabel(stringResource(R.string.tv_add_store_to_play)) }
            } else {
                AmberButton(
                    onClick = { onPlay(entry.id, action.episode) },
                    // Still focusable while it waits for the episode, so it can say when it is out.
                    enabled = action.playable,
                    modifier = Modifier.firstFocus(memory, "play"),
                ) { ButtonLabel(if (action.unsourced) Strings.episodeUnsourced(context) else action.label(context)) }
            }
            Spacer(Modifier.width(16.dp))
            val status = entry.listStatus
            TonalButton(
                onClick = { actions?.open(statusSheet(context, store, entry.id, entry.title, status)) },
                icon = if (status == null) TvIcons.Library else TvIcons.Check,
                modifier = Modifier.remembered(memory, "status"),
            ) { ButtonLabel(status?.label(context) ?: stringResource(R.string.library_add_to_list)) }
            Spacer(Modifier.width(16.dp))
            TonalIconButton(
                TvIcons.More,
                stringResource(R.string.action_more),
                onClick = { actions?.open(titleSheet(context, store, entry, rows, action, Strings.sourcePickerTitle(context))) },
                modifier = Modifier.remembered(memory, "more"),
            )
        }
    }
}

/**
 * The season picker: the franchise's seasons in watch order as tabs, "Season 1" first, the one on
 * screen selected; a film or an OVA, which is none of them, has none selected. Focus comes in on
 * the selected tab, and Center on another shows that season's page in this one's place.
 */
@Composable
private fun Seasons(
    id: AnilistId,
    seasons: List<LinkedTitle>,
    memory: FocusMemory,
    onSeason: (AnilistId) -> Unit,
    modifier: Modifier,
) {
    val at = seasons.indexOfFirst { it.id == id }
    // The season before this one shows too, so the row says there are earlier ones.
    val state = rememberLazyListState(initialFirstVisibleItemIndex = (at - 1).coerceAtLeast(0))
    val selected = remember { FocusRequester() }
    BrowseRow(
        stringResource(R.string.title_seasons),
        modifier = modifier,
        state = state,
        subtitle = if (at >= 0) stringResource(R.string.tv_seasons_position, at + 1, seasons.size) else pluralStringResource(R.plurals.tv_seasons_count, seasons.size, seasons.size),
        fallback = if (at >= 0) selected else FocusRequester.Default,
    ) {
        items(seasons.size, key = { seasons[it].id.toLong() }, contentType = { "season" }) { index ->
            val season = seasons[index]
            val chosen = season.id == id
            TvTab(
                stringResource(R.string.season_number, index + 1),
                selected = chosen,
                onSelect = { if (!chosen) onSeason(season.id) },
                settle = null,
                chip = true,
                count = season.year?.toString(),
                modifier = Modifier
                    .then(if (chosen) Modifier.focusRequester(selected) else Modifier)
                    .remembered(memory, "season/${season.id}"),
            )
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
    val host = LocalActions.current
    val context = LocalContext.current
    val next = rows?.indexOfFirst { it.number == action.episode.toString() }?.takeIf { it >= 0 } ?: 0
    val state = rememberLazyListState(initialFirstVisibleItemIndex = next)
    val nextRequester = remember { FocusRequester() }
    val placeholders = placeholdersAfterWait(rows == null)
    val found by store.discussion.found.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val discussions = settings == null || store.settings.isOn(tv.anistream.engine.model.SettingKey.EPISODE_DISCUSSIONS, settings)
    BrowseRow(stringResource(R.string.title_episodes), modifier = modifier, subtitle = dub, state = state, fallback = nextRequester) {
        if (rows == null) {
            if (placeholders) {
                items(PLACEHOLDER_CARDS, key = { "placeholder/$it" }) { Box(swapFade()) { tv.anistream.tv.ui.CardPlaceholder(Canvas.CardWidth, Canvas.CardHeight) } }
            }
            return@BrowseRow
        }
        items(rows, key = { it.number }, contentType = { "episode" }) { row ->
            val index = rows.indexOf(row)
            val playsNext = index == next
            Box(swapFade()) {
            EpisodeCard(
                row = row,
                entry = entry,
                line = if (row.unsourced) Strings.episodeUnsourced(context) else row.cardLine(context, entry.resume, playsNext),
                progress = if (playsNext) entry.resume?.fraction?.toFloat() else null,
                // The quiet line under a watched episode that has a thread: not focusable.
                discussion = row.number.toUIntOrNull()?.takeIf { row.completed && discussions }
                    ?.let { found[tv.anistream.store.DiscussionKey(entry.id, it)] }
                    ?.let { Strings.discussionRow(context, it.toInt()) },
                // Unwatched stills after the next one keep their colours and none of their detail.
                // Only an episode's own still can spoil; the show's banner in its place cannot.
                spoiler = spoilers && index > next && !row.completed && row.thumbnail != null,
                // One no source lists plays nothing: Center offers what can be done instead.
                onClick = {
                    if (row.unsourced) host?.open(episodeSheet(context, store, entry, rows, row, onPlayFromStart))
                    else row.number.toUIntOrNull()?.let { onPlay(entry.id, it) }
                },
                sheet = { episodeSheet(context, store, entry, rows, row, onPlayFromStart) },
                modifier = Modifier
                    .then(if (playsNext) Modifier.focusRequester(nextRequester) else Modifier)
                    .remembered(memory, "episode/${row.number}"),
            )
            }
        }
    }
}

@Composable
private fun EpisodeCard(
    row: EpisodeRow,
    entry: Entry,
    line: String?,
    progress: Float?,
    discussion: String?,
    spoiler: Boolean,
    onClick: () -> Unit,
    sheet: () -> ActionSheet,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val base = cardLabel(context, row.cardTitle(context), line, progress)
    val label = if (row.completed) Strings.tvCardLabelWatched(context, base) else base
    Column(Modifier.width(Canvas.CardWidth)) {
        ArtCard(onClick, Canvas.CardWidth, Canvas.CardHeight, modifier, sheet, label) {
            // One no source lists is dimmed under its focus ring, which stays as bright as any.
            ArtImage(
                row.thumbnail ?: entry.bannerUrl,
                CoverPlate.parse(entry.coverColor),
                pixels(Canvas.CardWidth),
                Modifier.fillMaxSize().alpha(if (row.unsourced) 0.45f else 1f),
                spoiler = spoiler,
            )
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
            if (row.completed) {
                Box(
                    Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).background(ObiSilk.Ground.copy(alpha = 0.72f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(TvIcons.Check, contentDescription = null, tint = ObiSilk.Water, modifier = Modifier.size(16.dp))
                }
            }
        }
        CardText(row.cardTitle(context), line, titleColor = if (row.completed || row.unsourced) ObiSilk.Haze else ObiSilk.Silk)
        if (discussion != null) {
            Text(
                discussion,
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Haze,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp).clearAndSetSemantics {},
            )
        }
    }
}

/** The synopsis (six lines at most), then studio, genres and source. Focusable, so it scrolls in. */
@Composable
private fun Details(entry: Entry, memory: FocusMemory, modifier: Modifier) {
    Column(modifier.padding(start = Canvas.Keyline - 16.dp, end = Canvas.SafeX)) {
        val actions = LocalActions.current
        Text(
            stringResource(R.string.title_details),
            style = MaterialTheme.typography.titleMedium,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 16.dp, bottom = Canvas.HeaderGap).semantics { heading() },
        )
        val full = synopsisWithoutSpoilers(entry.synopsis)
        Surface(
            // Center opens the whole synopsis in a panel.
            onClick = { if (full.isNotEmpty()) actions?.open(ActionSheet(entry.title, body = { Synopsis(full) })) },
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
                    entry.studio?.let { stringResource(R.string.title_studio) to it },
                    entry.genres.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.filter_genres) to it.joinToString(", ") }, // i18n-skip
                    entry.source?.let { stringResource(R.string.title_based_on) to it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.uppercase() } },
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

/** The whole synopsis in a panel: focusable, so Up and Down scroll it. */
@Composable
private fun Synopsis(text: String) {
    val scroll = rememberScrollState()
    val focus = remember { FocusRequester() }
    val scope = rememberCoroutineScope()
    val step = with(LocalDensity.current) { 120.dp.toPx() }
    Box(
        Modifier
            .fillMaxSize()
            .focusRequester(focus)
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (event.key) {
                    Key.DirectionDown -> { scope.launch { scroll.animateScrollBy(step) }; true }
                    Key.DirectionUp -> { scope.launch { scroll.animateScrollBy(-step) }; true }
                    else -> false
                }
            }
            .focusable()
            .verticalScroll(scroll),
    ) {
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.padding(horizontal = 16.dp))
    }
    LaunchedEffect(Unit) { focus.focusSoon() }
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
    context: Context,
    store: AppStore,
    entry: Entry,
    rows: List<EpisodeRow>,
    row: EpisodeRow,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
): ActionSheet {
    val upTo = rows.takeWhile { it.number != row.number }.map { it.number } + row.number
    val number = row.number.toUIntOrNull()?.takeIf { !row.metadataOnly && !row.unsourced }
    val title = "${entry.title} · ${row.cardTitle(context)}" // i18n-skip
    val episode = row.number.toUIntOrNull()
    val comments = episode?.takeIf { row.completed && (store.settings.view.value == null || store.discussionsOn()) }?.let { store.discussion.count(entry.id, it) }
    return ActionSheet(
        title,
        listOfNotNull(
            number?.let { CardAction(Strings.episodePlayFromBeginning(context)) { onPlayFromStart(entry.id, it) } },
            if (row.completed) {
                CardAction(Strings.episodeMarkUnwatched(context)) { store.setWatched(entry.id, listOf(row.number), watched = false) }
            } else {
                CardAction(Strings.episodeMarkWatched(context)) { store.setWatched(entry.id, listOf(row.number)) }
            },
            CardAction(Strings.tvEpisodeMarkUpToHere(context)) { store.setWatched(entry.id, upTo) },
            if (comments != null && episode != null) {
                CardAction(
                    Strings.discussionOpen(context),
                    supporting = Strings.discussionCount(context, comments.toInt()),
                    then = ActionSheet(
                        Strings.discussionTitleEpisode(context, episode.toInt()),
                        width = 560.dp,
                        body = { DiscussionPanel(entry, episode) },
                    ),
                )
            } else {
                null
            },
            number?.let {
                CardAction(Strings.playerChooseSource(context), then = ActionSheet(title, body = { EpisodeReleases(store, entry.id, row.number) }))
            },
        ),
    )
}

/**
 * The title page's More: Show settings, Mark watched up to the episode it would play, Hide from
 * Continue, Drop (keeps progress).
 */
fun titleSheet(context: Context, store: AppStore, entry: Entry, rows: List<EpisodeRow>?, action: PlayAction, sourcesLabel: String): ActionSheet {
    val before = rows?.takeWhile { it.number != action.episode.toString() }?.filter { !it.completed }?.map { it.number }.orEmpty()
    return ActionSheet(
        entry.title,
        listOfNotNull(
            CardAction(Strings.showSettingsTitle(context), then = showSettingsSheet(context, store, entry.id, entry.title)),
            CardAction(sourcesLabel, then = sourcePickerSheet(store, entry, sourcesLabel)),
            before.takeIf { it.isNotEmpty() }?.let { upTo ->
                CardAction(Strings.episodeMarkUpTo(context, (action.episode - 1u).toInt())) { store.setWatched(entry.id, upTo) }
            },
            CardAction(Strings.continueHide(context)) { store.hideFromContinue(entry.id) },
            CardAction(Strings.continueDrop(context)) { store.setStatus(entry.id, LibrarySegment.DROPPED) },
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
