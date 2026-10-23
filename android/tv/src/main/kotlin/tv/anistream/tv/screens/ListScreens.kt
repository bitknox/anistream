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
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.store.AppClock
import tv.anistream.store.LibraryModel
import tv.anistream.store.ListState
import tv.anistream.store.label
import tv.anistream.tv.posterLine
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.GRID_COLUMNS
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.PosterGrid
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.remembered

/** "Fall 2026", for the season [date] falls in. */
fun seasonLabel(date: LocalDate): String {
    val name = when (date.monthValue) {
        12, 1, 2 -> "Winter"
        in 3..5 -> "Spring"
        in 6..8 -> "Summer"
        else -> "Fall"
    }
    // December belongs to the next year's winter, as AniList counts it.
    val year = if (date.monthValue == 12) date.year + 1 else date.year
    return "$name $year"
}

@Composable
fun SeasonalScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) { store.loadSeasonal() }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val shown = state.ids.mapNotNull { entries[it] }
    val grid = rememberLazyGridState()
    ToTop(nav, memory, scrolled = { grid.firstVisibleItemIndex > 0 || grid.firstVisibleItemScrollOffset > 0 }) { grid.scrollToItem(0) }
    val season = seasonLabel(AppClock.now().atZone(AppClock.zone).toLocalDate())
    PosterGrid(
        shown,
        memory,
        keyPrefix = "seasonal",
        onOpen = { onOpen(it.id) },
        state = grid,
        line = { it.posterLine },
        placeholders = if (placeholdersAfterWait(state is ListState.Loading || state is ListState.Idle)) GRID_COLUMNS * 2 else 0,
    ) {
        item(key = "tabs", span = { GridItemSpan(maxLineSpan) }, contentType = "tabs") {
            Row(Modifier.padding(bottom = 8.dp)) {
                TvTab(season, selected = true, onSelect = {}, settle = null, modifier = Modifier.remembered(memory, "tab"))
            }
        }
        if (state is ListState.Empty) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                Text("Nothing announced for $season yet.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

@Composable
fun LibraryScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit, onAccounts: () -> Unit) {
    val store = LocalAppStore.current
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
