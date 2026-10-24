// Continue: the queue first. The hero is the next episode's show with Resume focused; below it the
// rows, Continue watching, New this week and This season. The page scrolls by row: once focus
// leaves the hero, the focused row's header sits at y 48.

package tv.anistream.tv.screens

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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.launch
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.store.AppStore
import tv.anistream.store.ContinueItem
import tv.anistream.store.ListState
import tv.anistream.tv.heroAction
import tv.anistream.tv.heroLine
import tv.anistream.tv.posterLine
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.BrowseRow
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.CardPlaceholder
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.HeldScroll
import tv.anistream.tv.ui.HeroHeader
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.PosterCard
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.TextBar
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.WideCard
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.planSheet
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.settlesAt

/** The y a focused row's header scrolls to. */
private val RowTop = 48.dp

@Composable
fun ContinueScreen(
    nav: TvNavigator,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
) {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) {
        store.loadContinue()
        store.loadSeasonal()
    }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val state by store.continuing.list.state.collectAsStateWithLifecycle()
    val hidden by store.continuing.hidden.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val queue = remember(entries, state, hidden) { store.continuing.queue(entries) }
    val actions = LocalActions.current
    val lead = queue.lead
    LaunchedEffect(lead?.id) { lead?.let { store.loadEpisodes(it.id) } }

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val rowTop = with(LocalDensity.current) { RowTop.roundToPx() }
    ToTop(nav, memory, scrolled = { list.firstVisibleItemIndex > 0 || list.firstVisibleItemScrollOffset > 0 }) { list.scrollToItem(0) }

    val loading = state is ListState.Loading || state is ListState.Idle
    val placeholders = placeholdersAfterWait(loading && queue.isEmpty)
    // The hero's show leads the page, so its rows leave it out.
    val inProgress = queue.inProgress.filter { it.id != lead?.id }
    val newThisWeek = queue.newThisWeek.filter { it.id != lead?.id }
    val season = seasonal.ids.mapNotNull { entries[it] }.take(SEASON_CARDS)

    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            if (lead != null) {
                item(key = HERO, contentType = "hero") {
                    Hero(lead, tables[lead.id], memory, onOpen, onPlay, Modifier.onFocusChanged {
                        if (it.hasFocus) scope.launch { list.animateScrollToItem(0) }
                    })
                }
            } else if (placeholders) {
                item(key = "placeholder", contentType = "hero") { HeroPlaceholder() }
            }
            var firstRow = lead == null
            if (inProgress.isNotEmpty()) {
                val first = firstRow
                firstRow = false
                item(key = "continue", contentType = "row") {
                    BrowseRow(
                        "Continue watching",
                        modifier = Modifier.settlesAt(list, scope, "continue", rowTop),
                    ) {
                        items(inProgress, key = { it.id.toLong() }, contentType = { "wide" }) { item ->
                            val firstCard = first && item === inProgress.first()
                            WideCard(
                                item.entry,
                                onClick = { onOpen(item.id) },
                                line = item.action,
                                progress = item.fraction?.toFloat(),
                                onLongClick = { actions?.open(continueSheet(store, item, onOpen)) },
                                modifier = Modifier
                                    .firstFocus(memory, "continue/${item.id}", firstCard),
                            )
                        }
                    }
                }
            }
            if (newThisWeek.isNotEmpty()) {
                val first = firstRow
                firstRow = false
                item(key = "new", contentType = "row") {
                    BrowseRow("New this week", modifier = Modifier.settlesAt(list, scope, "new", rowTop)) {
                        items(newThisWeek, key = { it.id.toLong() }, contentType = { "wide" }) { item ->
                            WideCard(
                                item.entry,
                                onClick = { onOpen(item.id) },
                                line = item.action,
                                newDot = true,
                                onLongClick = { actions?.open(continueSheet(store, item, onOpen)) },
                                modifier = Modifier
                                    .firstFocus(memory, "new/${item.id}", first && item === newThisWeek.first()),
                            )
                        }
                    }
                }
            }
            if (season.isNotEmpty()) {
                val first = firstRow
                item(key = "season", contentType = "row") {
                    BrowseRow("This season", modifier = Modifier.settlesAt(list, scope, "season", rowTop)) {
                        items(season, key = { it.id.toLong() }, contentType = { "poster" }) { entry ->
                            PosterCard(
                                entry,
                                onClick = { onOpen(entry.id) },
                                line = entry.posterLine,
                                onLongClick = { actions?.open(planSheet(store, entry, onOpen)) },
                                modifier = Modifier
                                    .firstFocus(memory, "season/${entry.id}", first && entry === season.first()),
                            )
                        }
                    }
                }
            } else if (placeholders) {
                item(key = "season-placeholder", contentType = "row") { RowPlaceholder() }
            }
            if (!loading && lead == null && season.isEmpty() && seasonal !is ListState.Loading) {
                item(key = "empty", contentType = "empty") { EmptyQueue(state, onRetry = { store.loadContinue(force = true); store.loadSeasonal(force = true) }, memory) }
            }
        }
    }
    EnterFocus(memory, entry = lead != null || season.isNotEmpty())
}


@Composable
private fun Hero(
    item: ContinueItem,
    rows: List<tv.anistream.engine.model.EpisodeRow>?,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
    modifier: Modifier,
) {
    HeroHeader(item.entry, rows, modifier) {
        Spacer(Modifier.height(4.dp))
        Text(item.heroLine(rows), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1)
        Spacer(Modifier.height(16.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            AmberButton(
                onClick = { onPlay(item.id, item.episode) },
                enabled = item.playable,
                modifier = Modifier.firstFocus(memory, HERO),
            ) { ButtonLabel(item.heroAction) }
            Spacer(Modifier.width(16.dp))
            TonalButton(
                onClick = { onOpen(item.id) },
                icon = TvIcons.Info,
                modifier = Modifier.remembered(memory, "hero/title"),
            ) { ButtonLabel("Title page") }
        }
    }
}

@Composable
private fun HeroPlaceholder() {
    Column(Modifier.fillMaxWidth()) {
        Spacer(Modifier.height(212.dp))
        Column(Modifier.padding(start = Canvas.Keyline)) {
            TextBar(320.dp, height = 32.dp)
            Spacer(Modifier.height(12.dp))
            TextBar(260.dp)
        }
    }
}

@Composable
private fun RowPlaceholder() {
    Column(Modifier.padding(start = Canvas.Keyline)) {
        TextBar(140.dp, height = 16.dp)
        Spacer(Modifier.height(Canvas.HeaderGap))
        Row(horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap)) {
            repeat(5) { CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight) }
        }
    }
}

/** Nothing to show and nothing cached: what failed, and one way forward, focused. */
@Composable
private fun EmptyQueue(state: ListState, onRetry: () -> Unit, memory: FocusMemory) {
    Box(Modifier.fillMaxWidth().padding(top = 160.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            val line = if (state is ListState.Failed) "anistream can't reach AniList right now." else "Nothing to continue yet."
            Text(line, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Spacer(Modifier.height(20.dp))
            TonalButton(onClick = onRetry, modifier = Modifier.firstFocus(memory, "retry")) { ButtonLabel("Try again") }
        }
    }
}

private const val HERO = "hero"

/** This season's row stops at twenty posters; Seasonal has the rest. */
private const val SEASON_CARDS = 20

/** A Continue card's actions: mark its episode watched, hide it, drop the show, or open it. */
fun continueSheet(store: AppStore, item: ContinueItem, onOpen: (AnilistId) -> Unit): ActionSheet = ActionSheet(
    item.entry.title,
    listOfNotNull(
        CardAction("Mark episode ${item.episode} watched") { store.setWatched(item.id, listOf(item.episode.toString())) }
            .takeIf { item.playable },
        CardAction("Hide from Continue") { store.hideFromContinue(item.id) },
        CardAction("Drop") { store.setStatus(item.id, LibrarySegment.DROPPED) },
        CardAction("Open the title page") { onOpen(item.id) },
    ),
)
