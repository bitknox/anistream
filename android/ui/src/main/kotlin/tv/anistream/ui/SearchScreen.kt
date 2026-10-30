// Search: what is typed and every filter as one query, the titles found as posters, and before
// anything is asked, the recent searches and rails of titles. The engine searches as the query
// changes, words every filter, and keeps the recent searches.

package tv.anistream.ui

import tv.anistream.store.plainReason
import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExpandedFullScreenSearchBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RangeSlider
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import tv.anistream.design.Motion
import tv.anistream.design.tabular
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.BrowseRail
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.FilterKey
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.OnList
import tv.anistream.engine.model.Pick
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.mediaFormatLabel
import tv.anistream.engine.model.searchCountries
import tv.anistream.engine.model.searchCountryLabel
import tv.anistream.engine.model.searchFormats
import tv.anistream.engine.model.searchSeasonLabel
import tv.anistream.engine.model.searchSortLabel
import tv.anistream.engine.model.searchSorts
import tv.anistream.engine.model.searchSourceLabel
import tv.anistream.engine.model.searchSources
import tv.anistream.engine.model.searchStatusLabel
import tv.anistream.engine.model.searchStatuses
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.BugContext
import tv.anistream.store.ListState
import tv.anistream.store.SearchModel
import tv.anistream.store.label
import tv.anistream.store.normalized
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.store.percentText
import tv.anistream.store.SearchWords

// Words

/** A recent search in one line: its words, then its filters. "hoshi · Mecha · TV". */
internal val SearchQuery.recentLine: String
    get() {
        val text = normalized.text
        if (text.isEmpty()) return headline()
        val summary = summary()
        return if (summary.isEmpty()) text else "$text · $summary"
    }

/** The year, when the years are a single one: the only case a season narrows. */
internal val SearchQuery.singleYear: UInt?
    get() = years.min?.takeIf { it == years.max }

/** The engine's words for one of the filters set, as its chip says it. */
internal fun SearchQuery.label(key: FilterKey): String? = active().firstOrNull { it.key == key }?.label

/** What a tap makes a genre or tag: off, then in, then out, then off again. */
internal val Pick.next: Pick
    get() = when (this) {
        Pick.OFF -> Pick.INCLUDE
        Pick.INCLUDE -> Pick.EXCLUDE
        Pick.EXCLUDE -> Pick.OFF
    }

private fun Pick.words(context: Context): String? = when (this) {
    Pick.OFF -> null
    Pick.INCLUDE -> Strings.filterIncluded(context)
    Pick.EXCLUDE -> Strings.filterLeftOut(context)
}

private val SEASONS = listOf(Season.WINTER, Season.SPRING, Season.SUMMER, Season.FALL)

/** "Hide obscure titles": at least this many people have the title on a list. The engine's `OBSCURE_BELOW`. */
private const val OBSCURE_BELOW = 1_000u

/** How strongly a tag must apply when nothing else is set. The engine's `DEFAULT_TAG_RANK`. */
private const val DEFAULT_TAG_RANK = 60

/** The first year the year filters offer. */
private const val FIRST_YEAR = 1970

/** Change the search as a filter does: the engine searches once the changes stop. */
private fun AppStore.editSearch(change: (SearchQuery) -> SearchQuery) = searchLive(change(search.status.value.query))

/** "Watching · Ep 7", "Planning", or for a title not on a list, "TV · 2024". */
private fun searchDetail(context: Context, entry: Entry): String? {
    entry.listStatus?.let { status ->
        val label = status.label(context).replaceFirstChar { it.uppercase() }
        val next = entry.progress?.next
        return if (next != null && status == LibrarySegment.WATCHING) Strings.searchDetailEpisode(context, label, next.toInt()) else label
    }
    return listOfNotNull(formatLabel(context, entry.format), entry.year?.toString()).joinToString(" · ").ifEmpty { null } // i18n-skip
}

// The screen

/**
 * Search. [start] is searched as the screen first shows, and [startFilters] opens the filters
 * sheet: the fixtures' `query`, `genre` and `sheet` extras. [open] asks for the field to open for
 * typing as the screen shows, as a link to Search does; [opened] says it has, so coming back later
 * leaves the screen as it was.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(
    start: SearchQuery? = null,
    startFilters: Boolean = false,
    open: Boolean = false,
    opened: () -> Unit = {},
) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val text = rememberTextFieldState(status.query.text)
    val bar = rememberSearchBarState()
    val scope = rememberCoroutineScope()
    var filtersShown by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        store.loadSearchStart()
        if (start != null && store.search.asked == null) store.search(start)
        if (startFilters) filtersShown = true
    }
    LaunchedEffect(open) {
        // Said once it is open: saying it first would restart this and stop the opening.
        if (open) {
            bar.animateToExpanded()
            opened()
        }
    }
    // Each keystroke searches, once the typing stops; the engine waits for the pause.
    // What the field itself gave the store since they last agreed. The store echoes each of these
    // a frame or more later; an echo is not words from outside and must not overwrite what was
    // typed in between.
    val sent = remember { mutableSetOf<String>() }
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).collect {
            sent += it
            store.searchLive(it)
        }
    }
    // A recent search or a rail's See all brings its own words.
    LaunchedEffect(status.query.text) {
        val stored = status.query.text
        val typed = text.text.toString()
        if (stored == typed) {
            sent.clear()
        } else if (stored !in sent) {
            sent.clear()
            text.setTextAndPlaceCursorAtEnd(stored)
        }
    }

    fun submit() {
        store.search(text.text.toString())
        scope.launch { bar.animateToCollapsed() }
    }

    val clearLabel = stringResource(R.string.search_clear)
    val input = @Composable {
        SearchBarDefaults.InputField(
            textFieldState = text,
            searchBarState = bar,
            onSearch = { submit() },
            placeholder = { Text(stringResource(R.string.search_placeholder)) },
            leadingIcon = { Icon(Icons.Search, contentDescription = null) },
            trailingIcon = {
                if (text.text.isNotEmpty()) {
                    IconButton(
                        onClick = { text.clearText() },
                        modifier = Modifier.semantics { contentDescription = clearLabel },
                    ) { Icon(Icons.Clear, contentDescription = null) }
                }
            },
        )
    }
    val colors = SearchBarDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)

    // The destination's name for TalkBack; the bar says the rest.
    val searchTitle = stringResource(R.string.nav_search)
    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars).semantics { paneTitle = searchTitle }) {
        SearchBar(
            state = bar,
            inputField = input,
            colors = colors,
            modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp).fillMaxWidth(),
        )
        FilterRow(status.query, onFilters = { filtersShown = true })
        val overdue = searchOverdue(status)
        // The start stays while a first search is under 600 ms away, then gives way to the
        // results or their placeholders in one crossfade.
        val startShown = status.atStart || (status.answered == null && status.pending && !overdue)
        Crossfade(startShown, Modifier.weight(1f), animationSpec = Motion.standard(), label = "search") { start -> // i18n-skip
            if (start) SearchStart() else SearchResults(status.query, overdue)
        }
    }
    ExpandedFullScreenSearchBar(state = bar, inputField = input, colors = colors) {
        Typing(text.text.toString(), onPicked = {
            text.clearText()
            scope.launch { bar.animateToCollapsed() }
        })
    }
    if (filtersShown) FiltersSheet(onDismiss = { filtersShown = false })
}

/**
 * The search view while typing: the genres and tags the words start, which become filters, the
 * recent searches before anything is typed, and the titles found as the typing pauses.
 */
@Composable
private fun Typing(typed: String, onPicked: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val words = typed.trim()
    val facets = status.facets
    val genres = if (words.length >= 2) facets?.genresMatching(words).orEmpty().take(3) else emptyList()
    val tags = if (words.length >= 2) facets?.tagsMatching(words, 3u).orEmpty() else emptyList()
    Column(Modifier.fillMaxSize()) {
        for (genre in genres) {
            Suggestion(genre, stringResource(R.string.filter_genre)) {
                store.searchLive(status.query.copy(text = "").pickingGenre(genre, Pick.INCLUDE))
                onPicked()
            }
        }
        for (tag in tags) {
            Suggestion(tag, stringResource(R.string.filter_tag)) {
                store.searchLive(status.query.copy(text = "").pickingTag(tag, Pick.INCLUDE))
                onPicked()
            }
        }
        if (genres.isNotEmpty() || tags.isNotEmpty()) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Box(Modifier.weight(1f)) {
            when {
                words.isEmpty() && status.recents.isNotEmpty() -> PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)) {
                    recentSearches(status.recents)
                }
                !status.atStart -> SearchResults(status.query, searchOverdue(status))
                else -> {}
            }
        }
    }
}

/** A genre or tag the words start: a tap makes it a filter in place of the words. */
@Composable
private fun Suggestion(name: String, kind: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(onClickLabel = stringResource(R.string.search_filter_by, name), onClick = onClick).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(kind, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

// The filters up front

/**
 * The filters up front: the sort, genres, year, season, format and status, each a menu, then All
 * filters for the rest. Changing one searches.
 */
@Composable
private fun FilterRow(query: SearchQuery, onFilters: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    LazyRow(
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.padding(top = 8.dp),
    ) {
        item(key = "sort") {
            val current = query.sortInEffect()
            MenuChip(SearchWords.sort(LocalContext.current, current), selected = query.sort != null, describe = stringResource(R.string.sort_label, SearchWords.sort(LocalContext.current, current))) { close ->
                for (sort in searchSorts(!query.isBrowse())) {
                    Choice(SearchWords.sort(LocalContext.current, sort), sort == current) {
                        close()
                        store.editSearch { it.copy(sort = sort) }
                    }
                }
            }
        }
        item(key = "genres") {
            val set = query.genres.size + query.genresExcluded.size
            val title = when (set) {
                0 -> stringResource(R.string.filter_genres)
                else -> pluralStringResource(R.plurals.search_genres_picked, set, set)
            }
            MenuChip(title, selected = set > 0) { _ ->
                if (set > 0) Choice(stringResource(R.string.filter_any_genre), false) { store.editSearch { it.copy(genres = emptyList(), genresExcluded = emptyList()) } }
                val names = status.facets?.genres.orEmpty()
                if (names.isEmpty()) DropdownMenuItem(text = { Text(stringResource(R.string.search_loading_genres)) }, onClick = {}, enabled = false)
                for (name in names) {
                    Choice(name, query.genrePick(name) == Pick.INCLUDE) {
                        val now = store.search.status.value.query
                        store.searchLive(now.pickingGenre(name, if (now.genrePick(name) == Pick.INCLUDE) Pick.OFF else Pick.INCLUDE))
                    }
                }
            }
        }
        item(key = "year") {
            val label = query.label(FilterKey.Years)
            MenuChip(label ?: stringResource(R.string.filter_year), selected = label != null) { close ->
                Choice(stringResource(R.string.filter_any_year), false) {
                    close()
                    store.searchLive(query.inYear(null))
                }
                for (year in (AppClock.now().atZone(AppClock.zone).year + 1) downTo FIRST_YEAR) {
                    Choice(year.toString(), query.singleYear == year.toUInt()) {
                        close()
                        store.searchLive(query.inYear(year.toUInt()))
                    }
                }
            }
        }
        item(key = "season") {
            val season = query.season
            MenuChip(
                season?.let { SearchWords.season(LocalContext.current, it) } ?: stringResource(R.string.filter_season),
                selected = season != null,
                // A season is a season of one year.
                enabled = query.singleYear != null || season != null,
                describe = if (query.singleYear == null && season == null) stringResource(R.string.filter_season) + ", " + stringResource(R.string.filter_season_pick_year) else null, // i18n-skip
            ) { close ->
                Choice(stringResource(R.string.filter_any_season), false) {
                    close()
                    store.editSearch { it.copy(season = null) }
                }
                for (each in SEASONS) {
                    Choice(SearchWords.season(LocalContext.current, each), each == season) {
                        close()
                        store.editSearch { it.copy(season = each) }
                    }
                }
            }
        }
        item(key = "format") {
            val count = query.formats.size
            val title = when {
                count == 0 -> stringResource(R.string.filter_format)
                else -> if (count == 1) SearchWords.format(LocalContext.current, query.formats.first()) else pluralStringResource(R.plurals.search_formats_picked, count, count)
            }
            MenuChip(title, selected = count > 0) { _ ->
                if (count > 0) Choice(stringResource(R.string.filter_any_format), false) { store.editSearch { it.copy(formats = emptyList()) } }
                for (format in searchFormats()) {
                    Choice(SearchWords.format(LocalContext.current, format), format in query.formats) {
                        store.editSearch { it.togglingFormat(format) }
                    }
                }
            }
        }
        item(key = "status") {
            val count = query.statuses.size
            val title = when (count) {
                0 -> stringResource(R.string.filter_status)
                1 -> SearchWords.status(LocalContext.current, query.statuses.first())
                else -> pluralStringResource(R.plurals.search_statuses_picked, count, count)
            }
            MenuChip(title, selected = count > 0) { _ ->
                if (count > 0) Choice(stringResource(R.string.filter_any_status), false) { store.editSearch { it.copy(statuses = emptyList()) } }
                for (each in searchStatuses()) {
                    Choice(SearchWords.status(LocalContext.current, each), each in query.statuses) {
                        store.editSearch { it.togglingStatus(each) }
                    }
                }
            }
        }
        item(key = "filters") {
            val count = query.advancedCount()
            SearchChip(
                if (count == 0u) stringResource(R.string.filters_button) else stringResource(R.string.filters_button_count, count.toInt()),
                selected = count > 0u,
                describe = if (count > 0u) stringResource(R.string.filters_button_count, count.toInt()) else stringResource(R.string.filters_button),
                onClick = onFilters,
            )
        }
    }
}

/** A chip that opens a menu of [choices]; a choice that settles the filter calls the close it is handed. */
@Composable
private fun MenuChip(
    title: String,
    selected: Boolean,
    enabled: Boolean = true,
    describe: String? = null,
    choices: @Composable ColumnScope.(close: () -> Unit) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        SearchChip(title, selected, enabled = enabled, menu = true, describe = describe) { open = true }
        Menu(expanded = open, onDismissRequest = { open = false }, modifier = Modifier.heightIn(max = 420.dp)) {
            choices { open = false }
        }
    }
}

/** One choice in a filter's menu, with a check when it is set. */
@Composable
private fun Choice(text: String, chosen: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(text) },
        leadingIcon = {
            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                if (chosen) Icon(Icons.Check, contentDescription = null)
            }
        },
        onClick = onClick,
        modifier = Modifier.semantics { selected = chosen },
    )
}

/**
 * A filter chip, 32 high in a 48 target: outlined with haze words, or once set, on
 * primaryContainer with a check, as the Library's chips are. [menu] adds the arrow of a chip that
 * opens a menu.
 */
@Composable
private fun SearchChip(
    text: String,
    selected: Boolean,
    enabled: Boolean = true,
    menu: Boolean = false,
    describe: String? = null,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    val content = when {
        !enabled -> colors.onSurfaceVariant.copy(alpha = 0.38f)
        selected -> colors.onPrimaryContainer
        else -> colors.onSurfaceVariant
    }
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clickable(enabled = enabled, onClick = onClick)
            .semantics {
                this.selected = selected
                describe?.let { contentDescription = it }
            },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier
                .heightIn(min = 32.dp)
                .clip(shape)
                .then(if (selected) Modifier.background(colors.primaryContainer) else Modifier.border(1.dp, colors.outline, shape))
                .padding(start = if (selected) 8.dp else 16.dp, end = if (menu) 8.dp else 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (selected) Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
            Text(text, style = MaterialTheme.typography.labelMedium, color = content, maxLines = 1)
            if (menu) Icon(Icons.Down, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
        }
    }
}

/** The filters set, each in the engine's words with a cross that takes it off, and Clear all. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ActiveFilters(query: SearchQuery) {
    val store = LocalAppStore.current
    val active = query.active()
    if (active.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.Center) {
        for (filter in active) {
            val removeLabel = stringResource(R.string.filter_remove, filter.label)
            Box(
                Modifier
                    .heightIn(min = 48.dp)
                    .clickable(onClickLabel = stringResource(R.string.action_remove)) { store.searchLive(store.search.status.value.query.without(filter.key)) }
                    .semantics { contentDescription = removeLabel },
                contentAlignment = Alignment.Center,
            ) {
                Row(
                    Modifier
                        .heightIn(min = 32.dp)
                        .clip(shape)
                        .background(colors.surfaceContainer)
                        .padding(start = 12.dp, end = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(filter.label, style = MaterialTheme.typography.labelMedium, color = colors.onSurface, maxLines = 1)
                    Icon(Icons.Clear, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                }
            }
        }
        if (active.size > 1) {
            Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
                QuietButton(stringResource(R.string.action_clear_all)) { store.searchLive(store.search.status.value.query.cleared()) }
            }
        }
    }
}

// The titles found

/** The filters set and what is said above the titles, as the first rows of the grid. */
private fun LazyGridScope.resultsHeader(query: SearchQuery, caption: String?, livePaused: Boolean) {
    if (query.active().isNotEmpty()) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "active", contentType = "active") { ActiveFilters(query) }
    }
    if (livePaused && !query.isBrowse()) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "paused", contentType = "line") {
            Text(
                stringResource(R.string.search_paused_rate_limit),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    caption?.let {
        item(span = { GridItemSpan(maxLineSpan) }, key = "caption", contentType = "line") {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * Whether the answer to what is asked has been waited for past 600 ms (T8): the results before it
 * then give way to placeholders and the search shows as a job. Once up, the placeholders stay for
 * a further search rather than the earlier results coming back. Never while typing waits for the
 * search key.
 */
@Composable
private fun searchOverdue(status: SearchModel.Status): Boolean {
    var overdue by remember { mutableStateOf(false) }
    val waiting = status.pending && !status.livePaused
    LaunchedEffect(status.asked, waiting) {
        if (!waiting) {
            overdue = false
        } else if (!overdue) {
            delay(SEARCH_QUIET_MILLIS)
            overdue = true
        }
    }
    return overdue
}

private const val SEARCH_QUIET_MILLIS = 600L

/** The search as a job, said where its results will land: Searching for “lantern”. */
private fun SearchQuery.searchingLine(context: Context): String {
    val words = text.trim()
    return if (words.isEmpty()) Strings.searchSearchingFilters(context, headline()) else Strings.searchSearching(context, words)
}

/**
 * The titles found, a page at a time: the next is asked for as its placeholders come into view.
 * Past [overdue], placeholders in place of results that answer an earlier search.
 */
@Composable
private fun SearchResults(query: SearchQuery, overdue: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val state by store.search.list.state.collectAsStateWithLifecycle()
    val caption by store.search.list.caption.collectAsStateWithLifecycle()
    val status by store.search.status.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val grid = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)
    val searching = status.asked?.searchingLine(LocalContext.current)
    val answered = state.takeIf { !overdue && (it is ListState.Loaded || it is ListState.Empty || it is ListState.Failed) }
    Loading(
        answered,
        returns = true,
        waited = overdue,
        placeholders = {
            PosterGrid(grid) {
                resultsHeader(query, searching, status.livePaused)
                items(9, contentType = { "placeholder" }) { PosterPlaceholder() }
            }
        },
    ) { list ->
        PosterGrid(grid) {
            resultsHeader(query, caption, status.livePaused)
            when (list) {
                is ListState.Loaded -> {
                    val entries = list.ids.mapNotNull { titles[it] }
                    items(entries, key = { it.id.toLong() }, contentType = { "poster" }) {
                        PosterCell(it, itemMotion(), from = "search", detail = searchDetail(LocalContext.current, it), onOpen = store::rememberSearch)
                    }
                    if (status.hasNext) {
                        items(3, key = { "more-$it" }, contentType = { "placeholder" }) { index ->
                            if (index == 0) LaunchedEffect(status.pages) { store.searchMore() }
                            PosterPlaceholder()
                        }
                    }
                }
                is ListState.Failed -> item(span = { GridItemSpan(maxLineSpan) }, key = "failed") {
                    Column {
                        Text(plainReason(context, list.reason), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Row {
                            QuietButton(stringResource(R.string.action_try_again)) { store.retrySearch() }
                            QuietButton(stringResource(R.string.action_report)) { openBugReport(context, store, BugContext("load", list.reason)) }
                        }
                    }
                }
                else -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing") { NothingFound(query) }
            }
        }
    }
}

/** Nothing found: what was asked, in the engine's words, and the way out when filters are set. */
@Composable
private fun NothingFound(query: SearchQuery) {
    val store = LocalAppStore.current
    Column(Modifier.padding(top = 8.dp)) {
        Text(query.nothingFound(), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (query.hasFilters()) {
            Box(Modifier.padding(top = 8.dp)) {
                QuietButton(stringResource(R.string.search_clear_filters)) { store.searchLive(store.search.status.value.query.cleared()) }
            }
        }
    }
}

// Search's start

/** Before anything is asked: the recent searches, the rails, and the genres. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SearchStart() {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)) {
        recentSearches(status.recents)
        val rails = status.rails
        if (rails == null) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "rail-placeholder") { RailPlaceholder() }
        } else {
            for (rail in rails.filter { it.entries.isNotEmpty() }) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "rail-${rail.title}") {
                    RailRow(rail, rail.entries.map { titles[it.id] ?: it })
                }
            }
        }
        val genres = status.facets?.genres.orEmpty()
        if (genres.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "genres") {
                Column {
                    StartHeading(stringResource(R.string.search_genres))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        for (genre in genres) SearchChip(genre, selected = false) { store.search(SearchQuery(genres = listOf(genre))) }
                    }
                }
            }
        }
    }
}

/** The recent searches, newest first, each with its filters: a tap searches again, the cross forgets it. */
private fun LazyGridScope.recentSearches(recents: List<SearchQuery>) {
    if (recents.isEmpty()) return
    item(span = { GridItemSpan(maxLineSpan) }, key = "recent") {
        val store = LocalAppStore.current
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { StartHeading(stringResource(R.string.search_recent)) }
                QuietButton(stringResource(R.string.action_clear)) { store.clearRecentSearches() }
            }
            for (query in recents.take(5)) {
                val removeLabel = stringResource(R.string.search_recent_remove, query.recentLine)
                Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        query.recentLine,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f).heightIn(min = 48.dp).clickable { store.search(query) }.padding(vertical = 12.dp),
                    )
                    IconButton(onClick = { store.forgetRecentSearch(query) }, modifier = Modifier.semantics { contentDescription = removeLabel }) {
                        Icon(Icons.Clear, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                    }
                }
            }
        }
    }
}

/** One rail: its heading, what it is, See all, and its posters in a row that scrolls. */
@Composable
private fun RailRow(rail: BrowseRail, entries: List<Entry>) {
    val store = LocalAppStore.current
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                StartHeading(rail.title)
                rail.detail?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
            QuietButton(stringResource(R.string.search_see_all)) { store.search(rail.query) }
        }
        LazyRow(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
            items(entries, key = { it.id.toLong() }) { PosterCell(it, itemMotion().width(104.dp), from = "rail-${rail.title}") }
        }
    }
}

/** Rails to come, once they are 600 ms away: headings and posters where the rails will be. */
@Composable
private fun RailPlaceholder() {
    AfterWait {
        Column {
            repeat(3) {
                TextPlaceholder(0.4f, 24.dp, Modifier.padding(vertical = 12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
                    repeat(4) { Box(Modifier.width(104.dp)) { PosterPlaceholder() } }
                }
            }
        }
    }
}

@Composable
private fun StartHeading(text: String) {
    Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(vertical = 12.dp).semantics { heading() })
}

// Every filter

/**
 * Every filter, in sections, in a sheet over the results: each change searches, so there is
 * nothing to apply. Half the screen at first, so the results show change under it.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun FiltersSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val query = status.query
    val facets = status.facets
    val scoreAny = stringResource(R.string.filter_score_any)
    val colors = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.PartiallyExpanded, SheetValue.Expanded)),
        containerColor = colors.surfaceContainer,
        scrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { BottomSheetDefaults.DragHandle(color = colors.onSurfaceVariant.copy(alpha = 0.4f)) },
        modifier = Modifier.padding(top = 77.dp - 24.dp),
    ) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
            Row(Modifier.padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.filters_title), style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f).semantics { heading() })
                QuietButton(stringResource(R.string.action_reset), enabled = query.hasFilters()) { store.searchLive(query.cleared()) }
            }

            FilterHeading(stringResource(R.string.filter_genres))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (name in facets?.genres.orEmpty()) {
                    PickChip(name, query.genrePick(name)) { store.editSearch { it.pickingGenre(name, it.genrePick(name).next) } }
                }
            }
            FilterNote(stringResource(R.string.filter_hint_touch))

            TagsSection(query)

            FilterHeading(stringResource(R.string.filter_released))
            YearsRange(query.years) { bounds ->
                store.editSearch { now ->
                    val next = now.copy(years = bounds)
                    if (next.singleYear == null) next.copy(season = null) else next
                }
            }

            FilterHeading(stringResource(R.string.filter_length))
            FilterLabel(stringResource(R.string.filter_episodes))
            Presets(
                listOf(Bounds(1u, 1u), Bounds(null, 12u), Bounds(13u, 26u), Bounds(50u, null)),
                query.episodes,
                words = { SearchQuery(episodes = it).label(FilterKey.Episodes) },
            ) { bounds -> store.editSearch { it.copy(episodes = bounds) } }
            FilterLabel(stringResource(R.string.filter_episode_length))
            Presets(
                listOf(Bounds(null, 15u), Bounds(20u, 30u), Bounds(60u, null)),
                query.minutes,
                words = { SearchQuery(minutes = it).label(FilterKey.Minutes) },
            ) { bounds -> store.editSearch { it.copy(minutes = bounds) } }

            FilterHeading(stringResource(R.string.filter_reception))
            StepSlider(
                stringResource(R.string.filter_score_at_least),
                value = query.minScore?.toInt() ?: 0,
                range = 0..95,
                step = 5,
                words = { if (it == 0) scoreAny else percentText(it) },
            ) { score -> store.editSearch { it.copy(minScore = score.takeIf { it > 0 }?.toUByte()) } }
            FilterSwitch(stringResource(R.string.filter_hide_obscure), query.minPopularity != null) { on ->
                store.editSearch { it.copy(minPopularity = if (on) OBSCURE_BELOW else null) }
            }

            FilterHeading(stringResource(R.string.filter_origin))
            FilterLabel(stringResource(R.string.filter_source_material))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (source in searchSources()) {
                    SearchChip(searchSourceLabel(source), source in query.sources) { store.editSearch { it.togglingSource(source) } }
                }
            }
            FilterLabel(stringResource(R.string.filter_country))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (code in searchCountries()) {
                    SearchChip(SearchWords.country(LocalContext.current, code), code in query.countries) { store.editSearch { it.togglingCountry(code) } }
                }
            }

            FilterHeading(stringResource(R.string.filter_where_to_watch))
            val sites = facets?.streamingSites.orEmpty()
            if (sites.isNotEmpty()) {
                FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (site in sites) {
                        SearchChip(site, site in query.streamingOn) { store.editSearch { it.togglingStreaming(site) } }
                    }
                }
            }
            FilterSwitch(stringResource(R.string.filter_licensed_only), query.licensedOnly) { on -> store.editSearch { it.copy(licensedOnly = on) } }

            if (anilistSignedIn()) {
                FilterHeading(stringResource(R.string.calendar_filter_mine))
                val choices = listOf<OnList?>(null, OnList.HIDE, OnList.ONLY)
                ConnectedChoice(
                    listOf(stringResource(R.string.filter_show), stringResource(R.string.filter_hide), stringResource(R.string.filter_only)),
                    selected = choices.indexOf(query.onList),
                    onSelect = { index -> store.editSearch { it.copy(onList = choices[index]) } },
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            FilterNote(stringResource(R.string.filter_live_note))
        }
    }
}

/** The tags: a field to find one, their categories each opening its tags, and how strongly they apply. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TagsSection(query: SearchQuery) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    var find by rememberSaveable { mutableStateOf("") }
    var spoilers by rememberSaveable { mutableStateOf(false) }
    var opened by rememberSaveable { mutableStateOf<String?>(null) }
    val groups = status.facets?.tagGroups.orEmpty()

    FilterHeading(stringResource(R.string.filter_tags))
    Box(Modifier.padding(horizontal = 16.dp)) { SheetField(find, { find = it }, stringResource(R.string.filter_tag_find)) }
    val words = find.trim().lowercase()
    if (words.isNotEmpty()) {
        val found = groups.flatMap { it.tags }.filter { (spoilers || !it.spoiler) && words in it.name.lowercase() }.take(40)
        if (found.isEmpty()) FilterNote(stringResource(R.string.filter_tag_none))
        FlowRow(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (tag in found) {
                PickChip(tag.name, query.tagPick(tag.name)) { store.editSearch { it.pickingTag(tag.name, it.tagPick(tag.name).next) } }
            }
        }
    } else {
        Spacer(Modifier.heightIn(min = 8.dp))
        for (group in groups) {
            val set = group.tags.count { query.tagPick(it.name) != Pick.OFF }
            val isOpen = opened == group.category
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .clickable(onClickLabel = stringResource(if (isOpen) R.string.action_close else R.string.action_open)) { opened = if (isOpen) null else group.category }
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(group.category, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                if (set > 0) {
                    Text(
                        "$set",
                        style = MaterialTheme.typography.bodyMedium.tabular(),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(end = 8.dp),
                    )
                }
                Icon(if (isOpen) Icons.Down else Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (isOpen) {
                FlowRow(Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (tag in group.tags.filter { spoilers || !it.spoiler }) {
                        PickChip(tag.name, query.tagPick(tag.name)) { store.editSearch { it.pickingTag(tag.name, it.tagPick(tag.name).next) } }
                    }
                }
            }
        }
    }
    FilterSwitch(stringResource(R.string.filter_spoiler_tags), spoilers) { spoilers = it }
    val tagged = query.tags.isNotEmpty() || query.tagsExcluded.isNotEmpty()
    StepSlider(
        stringResource(R.string.filter_tag_rank_title),
        value = query.minTagRank?.toInt() ?: DEFAULT_TAG_RANK,
        range = 0..100,
        step = 10,
        enabled = tagged,
        words = { percentText(it) },
    ) { rank -> store.editSearch { it.copy(minTagRank = if (rank == DEFAULT_TAG_RANK) null else rank.toUByte()) } }
}

/**
 * A genre or tag with three states: outlined when off, on primaryContainer with a check when
 * included, and struck through on the raised fill when left out.
 */
@Composable
private fun PickChip(name: String, pick: Pick, onClick: () -> Unit) {
    val words = pick.words(LocalContext.current)
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clickable(onClick = onClick)
            .semantics {
                words?.let { stateDescription = it }
                selected = pick != Pick.OFF
            },
        contentAlignment = Alignment.Center,
    ) {
        val background = when (pick) {
            Pick.OFF -> Modifier.border(1.dp, colors.outline, shape)
            Pick.INCLUDE -> Modifier.background(colors.primaryContainer)
            Pick.EXCLUDE -> Modifier.background(colors.surfaceContainerHighest)
        }
        val content = when (pick) {
            Pick.OFF -> colors.onSurfaceVariant
            Pick.INCLUDE -> colors.onPrimaryContainer
            Pick.EXCLUDE -> colors.onSurface
        }
        Row(
            Modifier.heightIn(min = 32.dp).clip(shape).then(background).padding(start = if (pick == Pick.OFF) 16.dp else 8.dp, end = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when (pick) {
                Pick.INCLUDE -> Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                Pick.EXCLUDE -> Icon(Icons.Clear, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                Pick.OFF -> {}
            }
            Text(
                name,
                style = MaterialTheme.typography.labelMedium.copy(textDecoration = if (pick == Pick.EXCLUDE) TextDecoration.LineThrough else null),
                color = content,
                maxLines = 1,
            )
        }
    }
}

/** The years a title started in, from 1970 to next year, as a range; at both ends, any year. */
@Composable
private fun YearsRange(years: Bounds, onChange: (Bounds) -> Unit) {
    val last = AppClock.now().atZone(AppClock.zone).year + 1
    val low = years.min?.toInt()?.coerceIn(FIRST_YEAR, last) ?: FIRST_YEAR
    val high = years.max?.toInt()?.coerceIn(FIRST_YEAR, last) ?: last
    var shown by remember(low, high) { mutableStateOf(low.toFloat()..high.toFloat()) }
    val from = shown.start.toInt()
    val to = shown.endInclusive.toInt()
    val yearName = stringResource(R.string.filter_year)
    val words = when {
        from == FIRST_YEAR && to == last -> stringResource(R.string.filter_any_year)
        from == to -> "$from"
        from == FIRST_YEAR -> stringResource(R.string.filter_years_up_to, "$to")
        to == last -> stringResource(R.string.filter_years_from, "$from")
        else -> stringResource(R.string.filter_years_between, "$from", "$to")
    }
    val described = yearName + ", " + words // i18n-skip
    Column(Modifier.padding(horizontal = 16.dp)) {
        Row(Modifier.heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.filter_year), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            Text(words, style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        RangeSlider(
            value = shown,
            onValueChange = { shown = it },
            onValueChangeFinished = {
                val a = shown.start.toInt()
                val b = shown.endInclusive.toInt()
                onChange(Bounds(a.takeIf { it > FIRST_YEAR }?.toUInt(), b.takeIf { it < last }?.toUInt()))
            },
            valueRange = FIRST_YEAR.toFloat()..last.toFloat(),
            steps = last - FIRST_YEAR - 1,
            colors = sliderColors(),
            modifier = Modifier.semantics { contentDescription = described },
        )
    }
}

/** A slider that steps by [step] across [range], its value in [words] at the end of its label. */
@Composable
private fun StepSlider(
    title: String,
    value: Int,
    range: IntRange,
    step: Int,
    enabled: Boolean = true,
    words: (Int) -> String,
    onChange: (Int) -> Unit,
) {
    var shown by remember(value) { mutableStateOf(value.toFloat()) }
    val described = title + ", " + words(shown.toInt()) // i18n-skip
    val now = shown.toInt()
    Column(Modifier.padding(horizontal = 16.dp)) {
        Row(Modifier.heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Text(words(now), style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Slider(
            value = shown,
            onValueChange = { shown = it },
            onValueChangeFinished = { onChange(shown.toInt()) },
            valueRange = range.first.toFloat()..range.last.toFloat(),
            steps = (range.last - range.first) / step - 1,
            enabled = enabled,
            colors = sliderColors(),
            modifier = Modifier.semantics { contentDescription = described },
        )
    }
}

/** The brief's slider: the amber track and thumb on outline, with no tick marks. */
@Composable
private fun sliderColors() = SliderDefaults.colors(
    activeTickColor = Color.Transparent,
    inactiveTickColor = Color.Transparent,
    inactiveTrackColor = MaterialTheme.colorScheme.outline,
)

/** Ready-made ranges, each in the engine's words; a tap on the one set takes it off. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Presets(choices: List<Bounds>, current: Bounds, words: (Bounds) -> String?, onPick: (Bounds) -> Unit) {
    FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (bounds in choices) {
            SearchChip(words(bounds).orEmpty(), selected = current == bounds) { onPick(if (current == bounds) Bounds(null, null) else bounds) }
        }
    }
}

/** A switch at the end of its row, its label at the start, the whole row the target. */
@Composable
private fun FilterSwitch(title: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable { onChange(!checked) }
            .padding(horizontal = 16.dp)
            .semantics { selected = checked },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        SettingSwitchMark(checked)
    }
}

/** A section's heading in the sheet, in haze, as the show settings sheet has them. */
@Composable
private fun FilterHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 24.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** What a group of chips under a heading chooses. */
@Composable
private fun FilterLabel(text: String) {
    Text(text, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 4.dp))
}

@Composable
private fun FilterNote(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 4.dp),
    )
}
