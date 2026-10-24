// Seasonal and the Library: tabs at the top and a six-column poster grid below. Library's tabs are
// the tracker's lists with their counts, selected once focus rests on one.

package tv.anistream.tv.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import java.time.LocalDate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SearchFilters
import tv.anistream.engine.model.Season
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.LibraryModel
import tv.anistream.store.ListState
import tv.anistream.store.label
import tv.anistream.tv.posterLine
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.GRID_COLUMNS
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.PosterGrid
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.planSheet
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.statusSheet

/** A season of a year, as Seasonal's tabs name it: "Fall 2026". */
data class SeasonTab(val season: Season, val year: Int) {
    val label: String get() = "${season.name.lowercase().replaceFirstChar { it.uppercase() }} $year"

    /** [steps] seasons later, or earlier when negative. */
    fun shift(steps: Int): SeasonTab {
        val index = year * 4 + season.ordinal + steps
        return SeasonTab(Season.entries[Math.floorMod(index, 4)], Math.floorDiv(index, 4))
    }

    companion object {
        /** The season [date] falls in: December is the next year's Winter. */
        fun of(date: LocalDate): SeasonTab = when (date.monthValue) {
            12 -> SeasonTab(Season.WINTER, date.year + 1)
            1, 2 -> SeasonTab(Season.WINTER, date.year)
            in 3..5 -> SeasonTab(Season.SPRING, date.year)
            in 6..8 -> SeasonTab(Season.SUMMER, date.year)
            else -> SeasonTab(Season.FALL, date.year)
        }

        /** The tabs: two seasons back, the current one, and the next. */
        fun around(date: LocalDate): List<SeasonTab> = of(date).let { now -> (-2..1).map(now::shift) }
    }
}

/** "Fall 2026", for the season [date] falls in. */
fun seasonLabel(date: LocalDate): String = SeasonTab.of(date).label

/** Seasonal's sorts, under AniList's names. */
enum class SeasonSort(val label: String, val key: String) {
    Popular("Popular", "POPULARITY_DESC"),
    Score("Score", "SCORE_DESC"),
    AirDay("Air day", "START_DATE"),
}

/** Seasonal's formats: every one, or one of AniList's. */
val SeasonFormats: List<Pair<String?, String>> = listOf(null to "All formats", "TV" to "TV", "MOVIE" to "Movie", "ONA" to "ONA", "OVA" to "OVA")

/**
 * What Seasonal asks the engine for: the current season, popular first, in every format is the
 * engine's own seasonal list; anything else a browse by filter.
 */
fun seasonFilter(tab: SeasonTab, current: SeasonTab, sort: SeasonSort, format: String?): SearchFilters? =
    if (tab == current && sort == SeasonSort.Popular && format == null) {
        null
    } else {
        SearchFilters(year = tab.year.toUInt(), season = tab.season.name, format = format, sort = sort.key)
    }

@Composable
fun SeasonalScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    val today = AppClock.now().atZone(AppClock.zone).toLocalDate()
    val current = SeasonTab.of(today)
    val tabs = SeasonTab.around(today)
    var tabIndex by rememberSaveable { mutableIntStateOf(tabs.indexOf(current)) }
    var sortIndex by rememberSaveable { mutableIntStateOf(0) }
    var formatIndex by rememberSaveable { mutableIntStateOf(0) }
    var onList by rememberSaveable { mutableStateOf(false) }
    val tab = tabs[tabIndex]
    val sort = SeasonSort.entries[sortIndex]
    val format = SeasonFormats[formatIndex]
    val filter = seasonFilter(tab, current, sort, format.first)
    LaunchedEffect(filter) { if (filter == null) store.loadSeasonal() else store.browse(filter) }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val browsed by store.browse.list.state.collectAsStateWithLifecycle()
    // A browse answer for another filter, still on screen while the new one loads, is not shown.
    val state = if (filter == null) seasonal else if (store.browse.takes("", filter)) browsed else ListState.Loading
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val signedIn = trackers.values.any { it.connected }
    // On my list: the season's titles already on one of the tracker's lists, which this loads, as
    // a season's own rows do not always say.
    val listed = onList && signedIn
    LaunchedEffect(listed) { if (listed) LibraryModel.order.forEach(store::loadLibrary) }
    val lists = LibraryModel.order.map { store.library.segment(it).state.collectAsStateWithLifecycle().value }
    val waiting = listed && lists.any { it is ListState.Idle || it is ListState.Loading }
    val onLists = lists.flatMap { it.ids }.toSet()
    val shown = state.ids.mapNotNull { entries[it] }.filter { !listed || it.listStatus != null || it.id in onLists }
    val grid = rememberLazyGridState()
    LaunchedEffect(filter) { grid.scrollToItem(0) }
    ToTop(nav, memory, scrolled = { grid.firstVisibleItemIndex > 0 || grid.firstVisibleItemScrollOffset > 0 }) { grid.scrollToItem(0) }
    PosterGrid(
        shown,
        memory,
        keyPrefix = "seasonal/${tab.label}/${sort.name}/${format.first}/$listed",
        onOpen = { onOpen(it.id) },
        state = grid,
        line = { it.posterLine },
        onLongClick = { entry -> actions?.open(planSheet(store, entry, onOpen)) },
        placeholders = if (placeholdersAfterWait(state is ListState.Loading || state is ListState.Idle || waiting)) GRID_COLUMNS * 2 else 0,
    ) {
        item(key = "tabs", span = { GridItemSpan(maxLineSpan) }, contentType = "tabs") {
            Row(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                tabs.forEachIndexed { index, each ->
                    TvTab(
                        each.label,
                        selected = index == tabIndex,
                        onSelect = { tabIndex = index },
                        modifier = Modifier.firstFocus(memory, "tab/${each.label}", index == tabIndex && shown.isEmpty()),
                    )
                }
            }
        }
        item(key = "chips", span = { GridItemSpan(maxLineSpan) }, contentType = "chips") {
            Row(Modifier.padding(bottom = 8.dp).focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvTab(
                    format.second,
                    selected = format.first != null,
                    onSelect = { formatIndex = (formatIndex + 1) % SeasonFormats.size },
                    settle = null,
                    chip = true,
                    modifier = Modifier.remembered(memory, "chip/format"),
                )
                TvTab(
                    "Sort: ${sort.label}",
                    selected = sort != SeasonSort.Popular,
                    onSelect = { sortIndex = (sortIndex + 1) % SeasonSort.entries.size },
                    settle = null,
                    chip = true,
                    modifier = Modifier.remembered(memory, "chip/sort"),
                )
                if (signedIn) {
                    TvTab(
                        "On my list",
                        selected = onList,
                        onSelect = { onList = !onList },
                        settle = null,
                        chip = true,
                        modifier = Modifier.remembered(memory, "chip/list"),
                    )
                }
            }
        }
        if (waiting) {
            // The lists are on their way: no line yet saying there is nothing.
        } else if (listed && state is ListState.Loaded && state.ids.isNotEmpty() && shown.isEmpty()) {
            item(key = "unlisted", span = { GridItemSpan(maxLineSpan) }) {
                Text("Nothing from ${tab.label} is on your lists yet.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        } else if (state is ListState.Empty || (state is ListState.Loaded && shown.isEmpty())) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                Text("Nothing announced for ${tab.label} yet.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
        if (state is ListState.Failed) {
            item(key = "failed", span = { GridItemSpan(maxLineSpan) }) {
                Text("${tab.label} did not load. ${state.reason}", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

@Composable
fun LibraryScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit, onAccounts: () -> Unit) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    var segment by rememberSaveable { mutableStateOf(LibrarySegment.WATCHING) }
    LaunchedEffect(Unit) { LibraryModel.order.forEach(store::loadLibrary) }
    LaunchedEffect(segment) { store.loadLibrary(segment) }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val states = LibraryModel.order.associateWith { store.library.segment(it).state.collectAsStateWithLifecycle().value }
    val state = states.getValue(segment)
    val shown = state.ids.mapNotNull { entries[it] }
    val signedIn = trackers.values.any { it.connected }
    val grid = rememberLazyGridState()
    ToTop(nav, memory, scrolled = { grid.firstVisibleItemIndex > 0 || grid.firstVisibleItemScrollOffset > 0 }) { grid.scrollToItem(0) }
    PosterGrid(
        shown,
        memory,
        keyPrefix = "library/${segment.name}",
        onOpen = { onOpen(it.id) },
        state = grid,
        line = { entry -> entry.progress?.let { "${it.done} / ${entry.episodes ?: "?"}" } },
        onLongClick = { entry -> actions?.open(librarySheet(store, entry, segment, onOpen)) },
        progress = { entry ->
            if (segment != LibrarySegment.WATCHING) return@PosterGrid null
            val done = entry.progress?.done?.toFloat() ?: return@PosterGrid null
            val total = entry.episodes?.toFloat()?.takeIf { it > 0 } ?: return@PosterGrid null
            done / total
        },
        placeholders = if (placeholdersAfterWait(state is ListState.Loading || state is ListState.Idle)) GRID_COLUMNS * 2 else 0,
    ) {
        item(key = "tabs", span = { GridItemSpan(maxLineSpan) }, contentType = "tabs") {
            Row(Modifier.padding(bottom = 8.dp).focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                LibraryModel.order.forEach { each ->
                    val count = states.getValue(each).let { if (it is ListState.Loaded) it.ids.size.toString() else null }
                    TvTab(
                        each.label.replaceFirstChar { it.uppercase() },
                        selected = each == segment,
                        onSelect = { segment = each },
                        count = count,
                        modifier = Modifier.firstFocus(memory, "tab/${each.name}", each == segment && shown.isEmpty()),
                    )
                }
            }
        }
        if (shown.isEmpty() && (state is ListState.Empty || state is ListState.Loaded || state is ListState.Failed)) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }, contentType = "empty") {
                if (!signedIn) {
                    NotSignedIn(onAccounts, memory)
                } else {
                    val name = segment.label.replaceFirstChar { it.uppercase() }
                    Text("Nothing in $name.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 24.dp))
                }
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

@Composable
private fun NotSignedIn(onAccounts: () -> Unit, memory: FocusMemory) {
    Box(Modifier.fillMaxWidth().padding(top = 96.dp), contentAlignment = Alignment.TopStart) {
        Column {
            Text("Your lists live in a tracker. Sign in to see them here.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Spacer(Modifier.height(20.dp))
            TonalButton(onClick = onAccounts, modifier = Modifier.remembered(memory, "connect")) { ButtonLabel("Connect a tracker") }
        }
    }
}

/** A Library card's actions: change its list, mark the next episode watched, or open it. */
fun librarySheet(store: AppStore, entry: Entry, segment: LibrarySegment, onOpen: (AnilistId) -> Unit): ActionSheet = ActionSheet(
    entry.title,
    listOfNotNull(
        CardAction("Change status", then = statusSheet(store, entry.id, entry.title, segment)),
        entry.progress?.next?.let { next -> CardAction("Mark episode $next watched") { store.setWatched(entry.id, listOf(next.toString())) } },
        CardAction("Open the title page") { onOpen(entry.id) },
    ),
)
