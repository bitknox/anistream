// Seasonal and the Library: tabs at the top and a six-column poster grid below. Library's tabs are
// the tracker's lists with their counts, selected once focus rests on one.

package tv.anistream.tv.screens

import tv.anistream.tv.seasonName
import androidx.annotation.StringRes
import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import java.time.LocalDate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.AiringStatus
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.MediaFormat
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.SearchSort
import tv.anistream.store.normalized
import tv.anistream.engine.model.Season
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.LibraryModel
import tv.anistream.store.ListState
import tv.anistream.store.label
import tv.anistream.tv.posterLine
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.GRID_COLUMNS
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.OfflineLine
import tv.anistream.tv.ui.PosterGrid
import tv.anistream.tv.ui.StateBlock
import tv.anistream.tv.ui.isOffline
import tv.anistream.store.plainReason
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.planSheet
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.statusSheet
import androidx.compose.ui.platform.LocalContext

/** A season of a year, as Seasonal's tabs name it: "Fall 2026". */
data class SeasonTab(val season: Season, val year: Int) {
    fun label(context: Context): String = Strings.seasonWithYear(context, seasonName(context, season), year.toString())

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
fun seasonLabel(context: Context, date: LocalDate): String = SeasonTab.of(date).label(context)

/** Seasonal's sorts, under AniList's names. */
enum class SeasonSort(@StringRes val label: Int, val sort: SearchSort) {
    Popular(R.string.sort_popularity, SearchSort.POPULARITY),
    Score(R.string.sort_score, SearchSort.SCORE),
    AirDay(R.string.sort_air_day, SearchSort.OLDEST),
}

/** Seasonal's formats: every one, or one of AniList's. */
val SeasonFormats: List<Pair<MediaFormat?, Int>> =
    listOf(null to R.string.filter_all_formats, MediaFormat.TV to R.string.format_tv, MediaFormat.MOVIE to R.string.format_movie, MediaFormat.ONA to R.string.format_ona, MediaFormat.OVA to R.string.format_ova)

/**
 * What Seasonal asks the engine for: the current season, popular first, in every format is the
 * engine's own seasonal list; anything else a browse by filter.
 */
fun seasonFilter(tab: SeasonTab, current: SeasonTab, sort: SeasonSort, format: MediaFormat?): SearchQuery? =
    if (tab == current && sort == SeasonSort.Popular && format == null) {
        null
    } else {
        SearchQuery(
            sort = sort.sort,
            formats = listOfNotNull(format),
            season = tab.season,
            years = Bounds(min = tab.year.toUInt(), max = tab.year.toUInt()),
        )
    }

@Composable
fun SeasonalScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val actions = LocalActions.current
    val offline = isOffline()
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
    val state = if (filter == null) seasonal else if (store.browse.takes(filter.normalized)) browsed else ListState.Loading
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
        keyPrefix = "seasonal/${tab.label(context)}/${sort.name}/${format.first}/$listed",
        onOpen = { onOpen(it.id) },
        state = grid,
        line = { entry -> listOfNotNull(entry.listStatus?.label(context), entry.posterLine(context)).joinToString(" · ").ifEmpty { null } }, // i18n-skip
        sheet = { entry -> planSheet(context, store, entry, onOpen) },
        // A water dot on the titles airing now.
        dot = { entry -> if (entry.status == AiringStatus.RELEASING) ObiSilk.Water else null },
        placeholders = if (placeholdersAfterWait(state is ListState.Loading || state is ListState.Idle || waiting)) GRID_COLUMNS * 2 else 0,
    ) {
        item(key = "tabs", span = { GridItemSpan(maxLineSpan) }, contentType = "tabs") {
            Row(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                tabs.forEachIndexed { index, each ->
                    TvTab(
                        each.label(context),
                        selected = index == tabIndex,
                        onSelect = { tabIndex = index },
                        modifier = Modifier.firstFocus(memory, "tab/${each.label(context)}", index == tabIndex && shown.isEmpty()),
                    )
                }
            }
        }
        item(key = "chips", span = { GridItemSpan(maxLineSpan) }, contentType = "chips") {
            Row(Modifier.padding(bottom = 8.dp).focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvTab(
                    stringResource(format.second),
                    selected = format.first != null,
                    onSelect = {
                        actions?.open(ActionSheet(Strings.filterFormat(context), SeasonFormats.mapIndexed { index, each -> CardAction(context.getString(each.second), selected = index == formatIndex) { formatIndex = index } }))
                    },
                    settle = null,
                    chip = true,
                    modifier = Modifier.remembered(memory, "chip/format"),
                )
                TvTab(
                    Strings.sortButton(context, context.getString(sort.label)),
                    selected = sort != SeasonSort.Popular,
                    onSelect = {
                        actions?.open(ActionSheet(Strings.tvSortTitle(context), SeasonSort.entries.map { each -> CardAction(context.getString(each.label), selected = each == sort) { sortIndex = each.ordinal } }))
                    },
                    settle = null,
                    chip = true,
                    modifier = Modifier.remembered(memory, "chip/sort"),
                )
                if (signedIn) {
                    TvTab(
                        stringResource(R.string.tv_on_my_list),
                        selected = onList,
                        onSelect = { onList = !onList },
                        settle = null,
                        chip = true,
                        modifier = Modifier.remembered(memory, "chip/list"),
                    )
                }
            }
        }
        if (offline) {
            item(key = "offline", span = { GridItemSpan(maxLineSpan) }, contentType = "line") { OfflineLine() }
        }
        if (waiting) {
            // The lists are on their way: no line yet saying there is nothing.
        } else if (listed && state is ListState.Loaded && state.ids.isNotEmpty() && shown.isEmpty()) {
            item(key = "unlisted", span = { GridItemSpan(maxLineSpan) }) {
                Text(stringResource(R.string.tv_seasonal_unlisted, tab.label(context)), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        } else if (state is ListState.Empty || (state is ListState.Loaded && shown.isEmpty())) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                StateBlock(stringResource(R.string.seasonal_empty_title, tab.label(context)), Modifier.padding(top = 48.dp), nagare = true)
            }
        }
        if (state is ListState.Failed && shown.isEmpty()) {
            item(key = "failed", span = { GridItemSpan(maxLineSpan) }) {
                StateBlock(
                    stringResource(R.string.tv_load_failed_named, tab.label(context)),
                    Modifier.padding(top = 48.dp),
                    detail = plainReason(LocalContext.current, state.reason),
                    action = stringResource(R.string.action_try_again),
                    onAction = { if (filter == null) store.loadSeasonal(force = true) else store.browse(filter) },
                    actionModifier = Modifier.firstFocus(memory, "retry/${tab.label(context)}"),
                )
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

@Composable
fun LibraryScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit, onAccounts: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val offline = isOffline()
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
        line = { entry -> entry.progress?.let { p -> entry.episodes?.let { Strings.tvProgressOf(context, p.done.toInt(), it.toInt()) } ?: Strings.tvProgressUnknown(context, p.done.toInt()) } },
        sheet = { entry -> librarySheet(context, store, entry, segment, onOpen) },
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
                        each.label(context),
                        selected = each == segment,
                        onSelect = { segment = each },
                        count = count,
                        modifier = Modifier.firstFocus(memory, "tab/${each.name}", each == segment && shown.isEmpty()),
                    )
                }
            }
        }
        if (offline) {
            item(key = "offline", span = { GridItemSpan(maxLineSpan) }, contentType = "line") { OfflineLine() }
        }
        if (shown.isEmpty() && (state is ListState.Empty || state is ListState.Loaded || state is ListState.Failed)) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }, contentType = "empty") {
                if (!signedIn) {
                    NotSignedIn(onAccounts, memory)
                } else if (state is ListState.Failed) {
                    val name = segment.label(context)
                    StateBlock(
                        stringResource(R.string.tv_load_failed_named, name),
                        Modifier.padding(top = 48.dp),
                        detail = plainReason(LocalContext.current, state.reason),
                        action = stringResource(R.string.action_try_again),
                        onAction = { store.loadLibrary(segment) },
                        actionModifier = Modifier.firstFocus(memory, "retry/${segment.name}"),
                    )
                } else {
                    val name = segment.label(context)
                    Text(stringResource(R.string.tv_library_empty, name), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 24.dp))
                }
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

/** Not signed in: Nagare, where the lists live, and the way to a tracker, focused. */
@Composable
private fun NotSignedIn(onAccounts: () -> Unit, memory: FocusMemory) {
    StateBlock(
        stringResource(R.string.tv_lists_signed_out),
        Modifier.padding(top = 48.dp),
        nagare = true,
        action = stringResource(R.string.tv_connect_tracker),
        onAction = onAccounts,
        actionModifier = Modifier.firstFocus(memory, "connect"),
    )
}

/** A Library card's actions: change its list, mark the next episode watched, or open it. */
fun librarySheet(context: Context, store: AppStore, entry: Entry, segment: LibrarySegment, onOpen: (AnilistId) -> Unit): ActionSheet = ActionSheet(
    entry.title,
    listOfNotNull(
        CardAction(Strings.tvChangeStatus(context), then = statusSheet(context, store, entry.id, entry.title, segment)),
        entry.progress?.next?.let { next -> CardAction(Strings.episodeMarkNWatched(context, next.toInt())) { store.setWatched(entry.id, listOf(next.toString())) } },
        CardAction(Strings.tvOpenTitlePage(context)) { onOpen(entry.id) },
    ),
)
