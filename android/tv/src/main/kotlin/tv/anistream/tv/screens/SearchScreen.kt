// Search: the field across the top, our keyboard on the left (a to z, 1 to 0, Space, Delete,
// Clear, and a key for the system keyboard), results on the right, 300 ms after the last change.
// Above the results, chips for the sort and the filters: with nothing typed they browse.

package tv.anistream.tv.screens

import tv.anistream.store.SearchWords
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import java.time.LocalDate
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.FilterKey
import tv.anistream.engine.model.Pick
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.searchFormats
import tv.anistream.engine.model.searchSorts
import tv.anistream.store.AppStore
import tv.anistream.store.ListState
import tv.anistream.tv.posterLine
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardPlaceholder
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.OfflineLine
import tv.anistream.tv.ui.PosterCard
import tv.anistream.tv.ui.SafeSpec
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.StateBlock
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.isOffline
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.store.plainReason
import tv.anistream.tv.ui.planSheet
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.tryFocus
import androidx.compose.ui.platform.LocalContext

/** Our keyboard's keys and what each does to the text. */
object SearchKeys {
    /** The character keys, row by row in six columns: a to z, then 1 to 0. */
    val characters: List<Char> = ('a'..'z').toList() + ('1'..'9').toList() + '0'

    const val COLUMNS = 6

    sealed interface Key {
        data class Character(val char: Char) : Key
        data object Space : Key
        data object Delete : Key
        data object Clear : Key
    }

    /** [text] after [key]: never a leading space nor two in a row. */
    fun type(text: String, key: Key): String = when (key) {
        is Key.Character -> text + key.char
        Key.Space -> if (text.isEmpty() || text.endsWith(' ')) text else "$text "
        Key.Delete -> text.dropLast(1)
        Key.Clear -> ""
    }
}

/** How long after the last change the search runs. */
private const val SEARCH_DELAY = 300L

/** How long a phone's submit waits for the first result: 40 tries 100 ms apart. */
private const val SUBMIT_TRIES = 40
private const val SUBMIT_PAUSE = 100L

private val KeySize = 48.dp

@Composable
fun SearchScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val text = nav.searchText
    LaunchedEffect(Unit) {
        store.loadSeasonal()
        store.loadSearchFacets()
    }
    LaunchedEffect(text) {
        val words = text.trim()
        if (words == store.search.status.value.query.text) return@LaunchedEffect
        delay(SEARCH_DELAY)
        // Clearing the words keeps the filters, which then browse.
        store.search(words)
    }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val results by store.search.list.state.collectAsStateWithLifecycle()
    val status by store.search.status.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val field = remember { FocusRequester() }
    val remote = LocalRemoteHost.current
    // A phone's Enter: focus the first result once the search has them.
    val submits = nav.searchSubmits
    LaunchedEffect(submits) {
        if (submits == 0) return@LaunchedEffect
        repeat(SUBMIT_TRIES) {
            val first = store.search.list.state.value.ids.firstOrNull()
            if (first != null && memory.focus("result/$first")) {
                remote?.typing = false
                return@LaunchedEffect
            }
            delay(SUBMIT_PAUSE)
        }
    }
    DisposableEffect(remote) { onDispose { remote?.typing = false } }

    DisposableEffect(nav) {
        val mine = {
            // Back from the results returns to the keyboard; from the keyboard, the rail opens.
            val onKeyboard = memory.last?.startsWith("key/") ?: true
            if (!onKeyboard) {
                memory.reset()
                memory.focusFirst()
            }
            !onKeyboard
        }
        nav.toTop = mine
        onDispose { if (nav.toTop === mine) nav.toTop = null }
    }

    Column(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY)) {
        QueryField(
            TextFieldValue(text, TextRange(nav.searchCursor)),
            onText = {
                remote?.typing = false
                nav.typeSearch(it.text, it.selection.end)
            },
            focus = field,
            typing = remote?.typing == true,
            modifier = Modifier.width(Canvas.SafeRight - Canvas.Keyline),
        )
        Spacer(Modifier.height(24.dp))
        Row(Modifier.fillMaxSize()) {
            Column(Modifier.width(288.dp)) {
                Keyboard(memory, onKey = {
                    remote?.typing = false
                    nav.typeSearch(SearchKeys.type(nav.searchText, it))
                })
                Spacer(Modifier.height(16.dp))
                val keyboard = LocalSoftwareKeyboardController.current
                TonalButton(
                    onClick = {
                        if (field.tryFocus()) keyboard?.show()
                    },
                    icon = TvIcons.Keyboard,
                    modifier = Modifier.remembered(memory, "key/system"),
                ) { ButtonLabel(stringResource(R.string.tv_search_system_keyboard)) }
            }
            Spacer(Modifier.width(32.dp))
            Results(status.query.copy(text = text.trim()), status, results, seasonal, entries, memory, onOpen)
        }
    }
    EnterFocus(memory)
}

/**
 * The field across the top: what is typed, or a quiet prompt. The system keyboard types here, and
 * focus on it fills it silk, as on any row. "Typing from a phone" sits at its end, so the note
 * coming and going moves nothing under it.
 */
@Composable
internal fun QueryField(value: TextFieldValue, onText: (TextFieldValue) -> Unit, focus: FocusRequester, typing: Boolean, modifier: Modifier) {
    val text = value.text
    var focused by remember { mutableStateOf(false) }
    val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
    val quiet = if (focused) ObiSilk.Ground else ObiSilk.Haze
    Box(
        modifier.height(56.dp).background(if (focused) ObiSilk.Silk else ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(TvIcons.Search, contentDescription = null, tint = quiet, modifier = Modifier.size(24.dp))
            Spacer(Modifier.width(16.dp))
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (text.isEmpty()) {
                    Text(stringResource(R.string.search_placeholder), style = MaterialTheme.typography.bodyLarge, color = quiet)
                }
                BasicTextField(
                    value = value,
                    onValueChange = onText,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = ink),
                    cursorBrush = SolidColor(if (focused) ObiSilk.Ground else ObiSilk.Amber),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { onText(value) }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focus).onFocusChanged { focused = it.isFocused },
                )
            }
            if (typing) {
                Spacer(Modifier.width(16.dp))
                Icon(TvIcons.Phone, contentDescription = null, tint = quiet, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.tv_search_typing_phone), style = MaterialTheme.typography.bodyMedium, color = quiet, maxLines = 1)
            }
        }
    }
}

/** Six columns of 48 × 48 keys, then Space, Delete and Clear two columns each. */
@Composable
internal fun Keyboard(memory: FocusMemory, onKey: (SearchKeys.Key) -> Unit) {
    Column {
        SearchKeys.characters.chunked(SearchKeys.COLUMNS).forEach { line ->
            Row {
                line.forEach { char ->
                    KeyCap(
                        label = char.toString(),
                        width = KeySize,
                        onClick = { onKey(SearchKeys.Key.Character(char)) },
                        modifier = Modifier
                            .firstFocus(memory, "key/$char", char == 'a'),
                    )
                }
            }
        }
        Row {
            KeyCap(stringResource(R.string.tv_search_space), KeySize * 2, onClick = { onKey(SearchKeys.Key.Space) }, modifier = Modifier.remembered(memory, "key/space"))
            KeyCap(
                stringResource(R.string.action_delete),
                KeySize * 2,
                onClick = { onKey(SearchKeys.Key.Delete) },
                icon = TvIcons.Delete,
                // Holding Center deletes on, a character each repeat.
                modifier = Modifier
                    .onPreviewKeyEvent { event ->
                        val center = event.key == Key.DirectionCenter || event.key == Key.Enter
                        if (center && event.type == KeyEventType.KeyDown && event.nativeKeyEvent.repeatCount > 0) {
                            onKey(SearchKeys.Key.Delete)
                            true
                        } else {
                            false
                        }
                    }
                    .remembered(memory, "key/delete"),
            )
            KeyCap(stringResource(R.string.action_clear), KeySize * 2, onClick = { onKey(SearchKeys.Key.Clear) }, modifier = Modifier.remembered(memory, "key/clear"))
        }
    }
}

/** A tonal key; focus fills it silk. */
@Composable
private fun KeyCap(
    label: String,
    width: Dp,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    Box(Modifier.size(width, KeySize).padding(2.dp)) {
        Surface(
            onClick = onClick,
            modifier = modifier.fillMaxSize(),
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
            colors = ClickableSurfaceDefaults.colors(
                containerColor = ObiSilk.RaisedSelected,
                contentColor = ObiSilk.Silk,
                focusedContainerColor = ObiSilk.Silk,
                focusedContentColor = ObiSilk.Ground,
                pressedContainerColor = ObiSilk.Silk,
                pressedContentColor = ObiSilk.Ground,
            ),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
        ) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                if (icon != null) {
                    Icon(icon, contentDescription = label, modifier = Modifier.size(20.dp))
                } else {
                    Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1)
                }
            }
        }
    }
}

/**
 * The right side: the sort and filter chips, then before anything is typed or set, This season;
 * then the results in three columns of posters, the next page as focus nears the end, or a line
 * naming what found nothing.
 */
@Composable
private fun Results(
    query: SearchQuery,
    status: tv.anistream.store.SearchModel.Status,
    results: ListState,
    seasonal: ListState,
    entries: Map<AnilistId, tv.anistream.engine.model.Entry>,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val leading = with(LocalDensity.current) { (Canvas.FocusRoom * 2 + RingRoom).toPx() }
    val trailing = with(LocalDensity.current) { 120.dp.toPx() }
    val spec = remember(leading, trailing) { SafeSpec(leading, trailing) }
    val offline = isOffline()
    val searching = !query.isBlank()
    val found = if (searching) results.ids.mapNotNull { entries[it] } else emptyList()
    val season = seasonal.ids.mapNotNull { entries[it] }
    val placeholders = placeholdersAfterWait(searching && results is ListState.Loading)
    val shown = if (searching) found else season
    // One line over the posters, always there, so results arriving never move the chips. It
    // changes with them: a new heading over the last search's posters would name the wrong ones.
    val asked = when {
        !searching -> stringResource(R.string.continue_this_season)
        query.isBrowse() -> query.headline()
        else -> query.summary()
    }
    var heading by remember { mutableStateOf(asked) }
    if (!searching || results !is ListState.Loading) heading = asked
    val grid = rememberLazyGridState()
    // The next page, once focus nears the last row.
    LaunchedEffect(grid, searching) {
        if (!searching) return@LaunchedEffect
        snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index to grid.layoutInfo.totalItemsCount }
            .distinctUntilChanged()
            .collect { (last, total) ->
                if (last != null && total > 0 && last >= total - NEAR_END) store.searchMore()
            }
    }
    ScrollRule(spec) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            state = grid,
            // The grid reaches up into the gap under the field, so a focused chip in the first
            // row has room for its ring while still lining up with the keyboard.
            modifier = Modifier.width(Canvas.SafeRight - 444.dp).fillMaxHeight().offset(y = -RingRoom),
            contentPadding = PaddingValues(top = Canvas.FocusRoom + RingRoom, bottom = 80.dp, start = Canvas.FocusRoom, end = Canvas.FocusRoom),
            horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            item(key = "chips", span = { GridItemSpan(maxLineSpan) }, contentType = "chips") {
                FilterChips(store, query, status, memory)
            }
            item(key = "heading", span = { GridItemSpan(maxLineSpan) }, contentType = "heading") {
                Text(
                    heading,
                    style = MaterialTheme.typography.titleMedium,
                    color = ObiSilk.Silk,
                    minLines = 1,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (offline) {
                item(key = "offline", span = { GridItemSpan(maxLineSpan) }, contentType = "line") { OfflineLine() }
            }
            if (searching && found.isEmpty()) {
                when (results) {
                    is ListState.Failed -> item(key = "failed", span = { GridItemSpan(maxLineSpan) }, contentType = "state") {
                        StateBlock(
                            stringResource(R.string.search_unreachable),
                            Modifier.padding(top = 48.dp),
                            detail = plainReason(LocalContext.current, results.reason),
                            action = stringResource(R.string.action_try_again),
                            onAction = { store.retrySearch() },
                            actionModifier = Modifier.remembered(memory, "result/retry"),
                        )
                    }
                    // An answer with no titles in it, whichever way the engine says so.
                    // A small empty: one line under the heading and the way out, no Nagare.
                    is ListState.Empty, is ListState.Loaded -> item(key = "empty", span = { GridItemSpan(maxLineSpan) }, contentType = "state") {
                        Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                            Text(query.nothingFound(), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
                            if (query.hasFilters() || query.sort != null) {
                                TonalButton(
                                    onClick = {
                                        memory.focus("chip/sort")
                                        store.searchLive(SearchFilters.settle(store.search.status.value.query.copy(text = query.text).cleared().copy(sort = null)))
                                    },
                                    modifier = Modifier.remembered(memory, "result/clear"),
                                ) { ButtonLabel(stringResource(R.string.search_clear_filters)) }
                            }
                        }
                    }
                    else -> if (placeholders) {
                        items(6, key = { "placeholder/$it" }, contentType = { "poster" }) {
                            CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight)
                        }
                    }
                }
            }
            items(shown, key = { it.id.toLong() }, contentType = { "poster" }) { entry ->
                PosterCard(
                    entry,
                    onClick = { onOpen(entry.id) },
                    line = entry.posterLine(LocalContext.current),
                    sheet = { planSheet(context, store, entry, onOpen) },
                    modifier = Modifier.remembered(memory, "result/${entry.id}"),
                )
            }
            if (searching && status.loadingMore) {
                items(3, key = { "more/$it" }, contentType = { "poster" }) {
                    CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight)
                }
            }
        }
    }
}

/** How many items from the end the next page is asked for. */
private const val NEAR_END = 6

// The chips

/**
 * The sort, then the genres, year, season and format, each a panel on the right, and Clear once
 * any is set. A choice searches at once, with the words typed or, with none, as a browse.
 */
@Composable
private fun FilterChips(store: AppStore, query: SearchQuery, status: tv.anistream.store.SearchModel.Status, memory: FocusMemory) {
    val actions = LocalActions.current
    val filter = { change: (SearchQuery) -> SearchQuery ->
        val now = store.search.status.value.query.copy(text = query.text)
        store.searchLive(SearchFilters.settle(change(now)))
    }
    // Wider than the column: the row scrolls as focus moves along it, with room after the last
    // chip so it is never cut through its label at the end.
    Row(
        Modifier.padding(bottom = 4.dp).focusRestorer().horizontalScroll(rememberScrollState()).padding(end = 48.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Clear leads, where it is seen, once anything is set.
        if (query.hasFilters() || query.sort != null) {
            TvTab(
                stringResource(R.string.action_clear),
                selected = false,
                onSelect = {
                    memory.focus("chip/sort")
                    filter { it.cleared().copy(sort = null) }
                },
                settle = null,
                chip = true,
                modifier = Modifier.remembered(memory, "chip/clear"),
            )
        }
        val sort = query.sortInEffect()
        val context = LocalContext.current
        TvTab(
            Strings.sortButton(context, SearchWords.sort(context, sort)),
            selected = query.sort != null,
            onSelect = {
                actions?.open(
                    ActionSheet(Strings.tvSortTitle(context), searchSorts(!query.isBrowse()).map { each -> CardAction(SearchWords.sort(context, each), selected = each == sort) { filter { it.copy(sort = each) } } }),
                )
            },
            settle = null,
            chip = true,
            // As wide as its longest label, so "Relevance" and "Popularity" trading places as the
            // words come and go moves no chip after it.
            modifier = Modifier.widthIn(min = sortChipWidth()).remembered(memory, "chip/sort"),
        )
        val genres = query.genres.size + query.genresExcluded.size
        TvTab(
            SearchFilters.genresChip(context, query),
            selected = genres > 0,
            onSelect = { actions?.open(ActionSheet(Strings.searchGenres(context), body = { GenrePanel(store, filter) })) },
            settle = null,
            chip = true,
            modifier = Modifier.remembered(memory, "chip/genres"),
        )
        val year = SearchFilters.singleYear(query)
        TvTab(
            query.active().firstOrNull { it.key == FilterKey.Years }?.label ?: stringResource(R.string.filter_year),
            selected = year != null,
            onSelect = {
                actions?.open(
                    ActionSheet(
                        Strings.filterYear(context),
                        listOf(CardAction(Strings.filterAnyYear(context), selected = year == null) { filter { it.inYear(null) } }) +
                            SearchFilters.years().map { each -> CardAction(each.toString(), selected = year == each.toUInt()) { filter { it.inYear(each.toUInt()) } } },
                    ),
                )
            },
            settle = null,
            chip = true,
            modifier = Modifier.remembered(memory, "chip/year"),
        )
        val chosen = query.season
        TvTab(
            chosen?.let { SearchWords.season(context, it) } ?: stringResource(R.string.filter_season),
            selected = chosen != null,
            onSelect = {
                // A season is a season of one year: without one, the year comes first.
                actions?.open(
                    if (year != null) {
                        seasonSheet(context, year, chosen, filter)
                    } else {
                        ActionSheet(Strings.filterYear(context), SearchFilters.years().map { each -> CardAction(each.toString(), then = seasonSheet(context, each.toUInt(), null, filter)) })
                    },
                )
            },
            settle = null,
            chip = true,
            modifier = Modifier.remembered(memory, "chip/season"),
        )
        TvTab(
            SearchFilters.formatChip(context, query),
            selected = query.formats.isNotEmpty(),
            onSelect = { actions?.open(ActionSheet(Strings.filterFormat(context), body = { FormatPanel(store, filter) })) },
            settle = null,
            chip = true,
            modifier = Modifier.remembered(memory, "chip/format"),
        )
    }
}

/** The Sort chip's width with its longest label: the label, the dot's slot and the padding. */
@Composable
private fun sortChipWidth(): Dp {
    val context = LocalContext.current
    val measurer = rememberTextMeasurer()
    val style = MaterialTheme.typography.labelLarge
    val density = LocalDensity.current
    return remember(measurer, style, density, context) {
        val widest = (searchSorts(true) + searchSorts(false)).maxOf { measurer.measure(Strings.sortButton(context, SearchWords.sort(context, it)), style, maxLines = 1).size.width }
        with(density) { widest.toDp() } + 32.dp + 14.dp
    }
}

/** The four seasons of [year]. */
private fun seasonSheet(context: Context, year: UInt, chosen: Season?, filter: ((SearchQuery) -> SearchQuery) -> Unit): ActionSheet = ActionSheet(
    Strings.tvSearchSeasonIn(context, year.toInt()),
    listOf(CardAction(Strings.filterAnySeason(context), selected = chosen == null) { filter { it.copy(season = null) } }) +
        SearchFilters.SEASONS.map { each -> CardAction(SearchWords.season(context, each), selected = each == chosen) { filter { it.inYear(year).copy(season = each) } } },
)

/**
 * The genres, each a row that stays open: Center includes a genre, again leaves it out, again
 * turns it off. "Any genre" clears them.
 */
@Composable
private fun GenrePanel(store: AppStore, filter: ((SearchQuery) -> SearchQuery) -> Unit) {
    val status by store.search.status.collectAsStateWithLifecycle()
    val query = status.query
    val names = status.facets?.genres
    val first = remember { FocusRequester() }
    val offline = isOffline()
    androidx.compose.foundation.lazy.LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item(key = "any") {
            TvListRow(
                stringResource(R.string.filter_any_genre),
                onClick = { filter { it.copy(genres = emptyList(), genresExcluded = emptyList()) } },
                selected = query.genres.isEmpty() && query.genresExcluded.isEmpty(),
                modifier = Modifier.focusRequester(first),
            )
        }
        item(key = "note") {
            Text(
                stringResource(R.string.filter_hint_remote),
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        if (names == null) {
            // A quiet line, not a row, so focus has nothing row-shaped to skip.
            item(key = "waiting") {
                Text(
                    if (offline) stringResource(R.string.tv_search_genres_offline) else stringResource(R.string.search_loading_genres),
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
        items(names.orEmpty().size, key = { "genre/${names!![it]}" }) { index ->
            val name = names!![index]
            val pick = query.genrePick(name)
            TvListRow(
                name,
                onClick = { filter { it.pickingGenre(name, SearchFilters.next(it.genrePick(name))) } },
                value = SearchFilters.pickWords(LocalContext.current, pick),
                selected = pick == Pick.INCLUDE,
            )
        }
    }
    LaunchedEffect(Unit) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

/** The formats, each a row that stays open and turns on or off. */
@Composable
private fun FormatPanel(store: AppStore, filter: ((SearchQuery) -> SearchQuery) -> Unit) {
    val status by store.search.status.collectAsStateWithLifecycle()
    val query = status.query
    val first = remember { FocusRequester() }
    androidx.compose.foundation.lazy.LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item(key = "any") {
            TvListRow(
                stringResource(R.string.filter_any_format),
                onClick = { filter { it.copy(formats = emptyList()) } },
                selected = query.formats.isEmpty(),
                modifier = Modifier.focusRequester(first),
            )
        }
        val formats = searchFormats()
        items(formats.size, key = { "format/${formats[it].name}" }) { index ->
            val format = formats[index]
            TvListRow(
                SearchWords.format(LocalContext.current, format),
                onClick = { filter { it.togglingFormat(format) } },
                selected = format in query.formats,
            )
        }
    }
    LaunchedEffect(Unit) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

/** The chips' words and the rules between the filters, apart from the screen for the tests. */
object SearchFilters {
    /** The earliest year the year panel offers. */
    const val FIRST_YEAR = 1970

    val SEASONS = listOf(Season.WINTER, Season.SPRING, Season.SUMMER, Season.FALL)

    /** Next year down to [FIRST_YEAR]. */
    fun years(today: LocalDate = LocalDate.now()): List<Int> = ((today.year + 1) downTo FIRST_YEAR).toList()

    /** The year, when the years are a single one: the only case a season narrows. */
    fun singleYear(query: SearchQuery): UInt? = query.years.min?.takeIf { it == query.years.max }

    /** A query whose years are no longer one keeps no season. */
    fun settle(query: SearchQuery): SearchQuery = if (singleYear(query) == null && query.season != null) query.copy(season = null) else query

    /** What Center makes a genre: off, then in, then out, then off again. */
    fun next(pick: Pick): Pick = when (pick) {
        Pick.OFF -> Pick.INCLUDE
        Pick.INCLUDE -> Pick.EXCLUDE
        Pick.EXCLUDE -> Pick.OFF
    }

    fun pickWords(context: Context, pick: Pick): String? = when (pick) {
        Pick.OFF -> null
        Pick.INCLUDE -> Strings.filterIncluded(context)
        Pick.EXCLUDE -> Strings.filterLeftOut(context)
    }

    /** "Genres", the one genre's words, or "3 genres". */
    fun genresChip(context: Context, query: SearchQuery): String {
        val set = query.genres.size + query.genresExcluded.size
        val resources = context.resources
        return when (set) {
            0 -> Strings.filterGenres(context)
            1 -> query.active().firstOrNull { it.key is FilterKey.Genre || it.key is FilterKey.NotGenre }?.label ?: resources.getQuantityString(R.plurals.search_genres_picked, 1, 1)
            else -> resources.getQuantityString(R.plurals.search_genres_picked, set, set)
        }
    }

    /** "Format", one or two formats' words, or "3 formats". */
    fun formatChip(context: Context, query: SearchQuery): String {
        val count = query.formats.size
        return when {
            count == 0 -> Strings.filterFormat(context)
            count <= 2 -> query.active().firstOrNull { it.key == FilterKey.Formats }?.label ?: Strings.filterFormat(context)
            else -> context.resources.getQuantityString(R.plurals.search_formats_picked, count, count)
        }
    }
}

/** How far the results grid reaches above its row, for the first row's focus ring. */
private val RingRoom = 10.dp
