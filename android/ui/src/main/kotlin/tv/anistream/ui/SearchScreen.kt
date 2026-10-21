// Search: the search bar at the top, which expands into the full-screen search view; before
// typing, recent searches, this season and the genres; after, filters and the titles found, your
// library's first.

package tv.anistream.ui

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExpandedFullScreenSearchBar
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import tv.anistream.store.ListState
import tv.anistream.store.label
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.SearchFilters

/** AniList's genres, as the filter and the start of Search offer them. */
private val GENRES = listOf(
    "Action", "Adventure", "Comedy", "Drama", "Fantasy", "Horror", "Mecha", "Music", "Mystery",
    "Psychological", "Romance", "Sci-Fi", "Slice of Life", "Sports", "Supernatural", "Thriller",
)

/** A filter: its name, its choices as AniList takes them with the words they show as. */
private enum class SearchFilter(val label: String, val choices: List<Pair<String, String>>) {
    Format("Format", listOf("TV" to "TV", "MOVIE" to "Movie", "OVA" to "OVA", "ONA" to "ONA", "SPECIAL" to "Special")),
    Season("Season", listOf("WINTER" to "Winter", "SPRING" to "Spring", "SUMMER" to "Summer", "FALL" to "Fall")),
    Genre("Genre", GENRES.map { it to it }),
    Status("Status", listOf("RELEASING" to "Airing", "FINISHED" to "Finished", "NOT_YET_RELEASED" to "Not yet aired"));

    fun value(filters: SearchFilters): String? = when (this) {
        Format -> filters.format
        Season -> filters.season
        Genre -> filters.genre
        Status -> filters.status
    }

    fun with(filters: SearchFilters, value: String?): SearchFilters = when (this) {
        Format -> filters.copy(format = value)
        Season -> filters.copy(season = value)
        Genre -> filters.copy(genre = value)
        Status -> filters.copy(status = value)
    }
}

/** The last searches on this device, newest first, at most eight. */
private class RecentSearches(private val context: Context) {
    private val preferences get() = context.getSharedPreferences("anistream", Context.MODE_PRIVATE)

    fun all(): List<String> = preferences.getString(KEY, "").orEmpty().split('\n').filter { it.isNotBlank() }

    fun add(query: String) {
        val kept = (listOf(query) + all().filterNot { it.equals(query, ignoreCase = true) }).take(8)
        preferences.edit().putString(KEY, kept.joinToString("\n")).apply()
    }

    private companion object {
        const val KEY = "recentSearches"
    }
}

/** "Watching · Ep 7", "Planning", or for a title not on a list, "TV · Fall 2024". */
private fun searchDetail(entry: Entry): String? {
    entry.listStatus?.let { status ->
        val label = status.label.replaceFirstChar { it.uppercase() }
        val next = entry.progress?.next
        return if (next != null && status == tv.anistream.engine.model.LibrarySegment.WATCHING) "$label · Ep $next" else label
    }
    val season = listOfNotNull(entry.season?.name?.lowercase()?.replaceFirstChar { it.uppercase() }, entry.year?.toString())
        .joinToString(" ").ifEmpty { null }
    return listOfNotNull(formatLabel(entry.format), season).joinToString(" · ").ifEmpty { null }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(startQuery: String? = null) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val recents = remember { RecentSearches(context) }
    val text = rememberTextFieldState()
    val bar = rememberSearchBarState()
    val scope = rememberCoroutineScope()
    var submitted by rememberSaveable { mutableStateOf<String?>(null) }
    var filters by remember { mutableStateOf(SearchFilters()) }
    var recent by remember { mutableStateOf(recents.all()) }

    fun search(query: String, with: SearchFilters = filters) {
        submitted = query
        filters = with
        if (query.isNotBlank()) {
            recents.add(query.trim())
            recent = recents.all()
        }
        store.search(query.trim(), with)
        scope.launch { bar.animateToCollapsed() }
    }

    LaunchedEffect(startQuery) {
        if (startQuery != null && submitted == null) {
            text.setTextAndPlaceCursorAtEnd(startQuery)
            search(startQuery)
        }
    }

    val input = @Composable {
        SearchBarDefaults.InputField(
            textFieldState = text,
            searchBarState = bar,
            onSearch = { search(it) },
            placeholder = { Text("Search titles") },
            leadingIcon = { Icon(Icons.Search, contentDescription = null) },
            trailingIcon = {
                if (text.text.isNotEmpty()) {
                    IconButton(
                        onClick = {
                            text.clearText()
                            submitted = null
                        },
                        modifier = Modifier.semantics { contentDescription = "Clear search" },
                    ) { Icon(Icons.Clear, contentDescription = null) }
                }
            },
        )
    }
    val colors = SearchBarDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)

    // The destination's name for TalkBack; the bar says the rest.
    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars).semantics { paneTitle = "Search" }) {
        SearchBar(
            state = bar,
            inputField = input,
            colors = colors,
            modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp).fillMaxWidth(),
        )
        val query = submitted
        if (query == null) {
            SearchStart(
                recent = recent,
                onRecent = { text.setTextAndPlaceCursorAtEnd(it); search(it) },
                onGenre = { search("", SearchFilters(genre = it)) },
            )
        } else {
            Filters(filters) { search(query, it) }
            Results(query)
        }
    }
    ExpandedFullScreenSearchBar(state = bar, inputField = input, colors = colors) {
        SearchStart(
            recent = recent,
            onRecent = { text.setTextAndPlaceCursorAtEnd(it); search(it) },
            onGenre = { search("", SearchFilters(genre = it)) },
        )
    }
}

/** Before anything is typed: the recent searches, this season and the genres. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SearchStart(recent: List<String>, onRecent: (String) -> Unit, onGenre: (String) -> Unit) {
    val store = LocalAppStore.current
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadSeasonal() }
    val season = state.ids.mapNotNull { titles[it] }
    PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)) {
        if (recent.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "recent") {
                Column {
                    StartHeading("Recent")
                    for (query in recent.take(4)) {
                        Text(
                            query,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.fillMaxWidth().height(48.dp).clickable { onRecent(query) }.padding(vertical = 12.dp),
                        )
                    }
                }
            }
        }
        if (season.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "season-heading") { StartHeading("This season") }
            item(span = { GridItemSpan(maxLineSpan) }, key = "season") {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
                    items(season, key = { it.id.toLong() }) { PosterCell(it, itemMotion().width(104.dp), from = "search-season") }
                }
            }
        }
        item(span = { GridItemSpan(maxLineSpan) }, key = "genres") {
            Column {
                StartHeading("Genres")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (genre in GENRES) OutlinedChip(genre, onClick = { onGenre(genre) })
                }
            }
        }
    }
}

@Composable
private fun StartHeading(text: String) {
    Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(vertical = 12.dp).semantics { heading() })
}

/** A chip 32 high in a 48 target, outlined, with haze words. */
@Composable
private fun OutlinedChip(text: String, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    Box(Modifier.height(48.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Text(
            text,
            style = MaterialTheme.typography.labelMedium,
            color = colors.onSurfaceVariant,
            modifier = Modifier.height(32.dp).clip(shape).border(1.dp, colors.outline, shape).padding(horizontal = 16.dp, vertical = 8.dp),
        )
    }
}

/** The filters as a row of chips, each opening its choices; a choice applies at once. */
@Composable
private fun Filters(filters: SearchFilters, onChange: (SearchFilters) -> Unit) {
    val colors = MaterialTheme.colorScheme
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 8.dp)) {
        items(SearchFilter.entries) { filter ->
            var open by remember { mutableStateOf(false) }
            val value = filter.value(filters)
            val chosen = filter.choices.firstOrNull { it.first == value }?.second
            val shape = MaterialTheme.shapes.small
            Box(Modifier.height(48.dp).clickable { open = true }.semantics { contentDescription = "${filter.label}: ${chosen ?: "any"}" }, contentAlignment = Alignment.Center) {
                Row(
                    Modifier
                        .height(32.dp)
                        .clip(shape)
                        .then(if (chosen != null) Modifier.background(colors.primaryContainer) else Modifier.border(1.dp, colors.outline, shape))
                        .padding(start = if (chosen != null) 8.dp else 16.dp, end = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    val content = if (chosen != null) colors.onPrimaryContainer else colors.onSurfaceVariant
                    if (chosen != null) Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                    Text(chosen ?: filter.label, style = MaterialTheme.typography.labelMedium, color = content)
                    Icon(Icons.Down, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                }
                Menu(expanded = open, onDismissRequest = { open = false }) {
                    DropdownMenuItem(text = { Text("Any") }, onClick = { open = false; onChange(filter.with(filters, null)) })
                    for ((key, label) in filter.choices) {
                        DropdownMenuItem(
                            text = { Text(label) },
                            leadingIcon = {
                                Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                    if (key == value) Icon(Icons.Check, contentDescription = "Current")
                                }
                            },
                            onClick = { open = false; onChange(filter.with(filters, key)) },
                        )
                    }
                }
            }
        }
    }
}

/** The titles found, your library's first; a line that repeats the query when there are none. */
@Composable
private fun Results(query: String) {
    val store = LocalAppStore.current
    val state by store.search.list.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val grid = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)
    val answered = state.takeIf { it is ListState.Loaded || it is ListState.Empty || it is ListState.Failed }
    Loading(answered, placeholders = { PosterGrid(grid) { items(9) { PosterPlaceholder() } } }) { list ->
        when (list) {
            is ListState.Loaded -> {
                val (yours, rest) = list.ids.mapNotNull { titles[it] }.partition { it.listStatus != null }
                PosterGrid(grid) {
                    items(yours + rest, key = { it.id.toLong() }) { PosterCell(it, itemMotion(), from = "search", detail = searchDetail(it)) }
                }
            }
            is ListState.Failed -> Quiet(list.reason)
            else -> Quiet(if (query.isBlank()) "Nothing matches these filters" else "Nothing matches “$query”")
        }
    }
}
