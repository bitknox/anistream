// Seasonal: this season's titles from AniList's public data, as a poster grid.

package tv.anistream.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.store.ListState
import tv.anistream.store.seasonQuery

@Composable
fun SeasonalScreen() {
    val store = LocalAppStore.current
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val caption by store.seasonal.caption.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val navigator = LocalNavigator.current
    LaunchedEffect(Unit) { store.loadSeasonal() }
    ScreenFrame(
        title = "Seasonal",
        subtitle = caption?.let(::seasonName),
        actions = {
            // The season as a search, to narrow by genre, format or anything else.
            QuietButton("Filter in Search") {
                store.search(seasonQuery())
                navigator.go(Destination.Search)
            }
        },
    ) { padding ->
        val grid = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 24.dp)
        val answered = state.takeIf { it is ListState.Loaded || it is ListState.Empty || it is ListState.Failed }
        val offline by store.offline.collectAsStateWithLifecycle()
        Loading(
            answered,
            placeholders = {
                if (offline) {
                    Column(Modifier.padding(top = padding.calculateTopPadding())) { OfflineState() }
                } else {
                    PosterGrid(grid) { items(9) { PosterPlaceholder() } }
                }
            },
        ) { list ->
            Refreshable({ store.loadSeasonal(force = true) }, top = padding.calculateTopPadding()) {
                when (list) {
                    is ListState.Loaded -> PosterGrid(grid) {
                        items(list.ids.mapNotNull { titles[it] }, key = { it.id.toLong() }) { PosterCell(it, itemMotion(), from = "seasonal") }
                    }
                    is ListState.Failed -> Column(Modifier.padding(top = padding.calculateTopPadding()).verticalScroll(rememberScrollState())) {
                        FailedState("Couldn't load this season", list.reason, retry = { store.loadSeasonal(force = true) })
                    }
                    else -> Column(Modifier.padding(top = padding.calculateTopPadding()).verticalScroll(rememberScrollState())) { Quiet("Nothing this season yet") }
                }
            }
        }
    }
}

/**
 * Posters three to a row on a phone, 13 apart across and 16 down; as many as fit at 140 dp or
 * more on a tablet.
 */
@Composable
internal fun PosterGrid(
    padding: PaddingValues,
    content: androidx.compose.foundation.lazy.grid.LazyGridScope.() -> Unit,
) {
    LazyVerticalGrid(
        columns = if (wideWindow()) GridCells.Adaptive(140.dp) else GridCells.Fixed(3),
        contentPadding = padding,
        horizontalArrangement = Arrangement.spacedBy(13.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content,
    )
}

/**
 * The season, from the caption the engine says above Seasonal: "Fall 2026" out of
 * "fall 2026 · the 40 most popular".
 */
internal fun seasonName(caption: String): String =
    caption.substringBefore('·').trim().split(' ').joinToString(" ") { word ->
        word.replaceFirstChar { it.titlecase() }
    }
