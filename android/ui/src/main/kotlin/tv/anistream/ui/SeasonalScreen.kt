// Seasonal: this season's titles from AniList's public data, as a poster grid.

package tv.anistream.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ArtShape
import tv.anistream.design.CoverPlate
import tv.anistream.store.ListState

@Composable
fun SeasonalScreen() {
    val store = LocalAppStore.current
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val caption by store.seasonal.caption.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadSeasonal() }
    ScreenFrame(title = "Seasonal", subtitle = caption?.let(::seasonName)) { padding ->
        val grid = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 24.dp)
        when (val list = state) {
            is ListState.Loaded -> LazyVerticalGrid(
                columns = GridCells.Fixed(3),
                contentPadding = grid,
                horizontalArrangement = Arrangement.spacedBy(13.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                items(list.ids.mapNotNull { titles[it] }, key = { it.id.toLong() }) { PosterCell(it) }
            }
            is ListState.Failed -> Column(Modifier.padding(top = padding.calculateTopPadding())) { Quiet(list.reason) }
            ListState.Empty -> Column(Modifier.padding(top = padding.calculateTopPadding())) {
                Quiet("Nothing this season yet")
            }
            // Static plates while it loads: no shimmer, no stagger.
            else -> LazyVerticalGrid(
                columns = GridCells.Fixed(3),
                contentPadding = grid,
                horizontalArrangement = Arrangement.spacedBy(13.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
                userScrollEnabled = false,
            ) {
                items(9) { PosterPlaceholder() }
            }
        }
    }
}

/** A poster's place before its title arrives: the plate and the two caption lines' height. */
@Composable
private fun PosterPlaceholder() {
    Column {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            CoverPlate(null, Modifier.size(maxWidth, maxWidth * 1.5f).clip(ArtShape.cover(maxWidth)))
        }
        Spacer(Modifier.height(8.dp))
        Box(Modifier.fillMaxWidth(0.8f).height(14.dp).clip(MaterialTheme.shapes.extraSmall)) {
            CoverPlate(null, Modifier.fillMaxWidth().height(14.dp))
        }
    }
}

/**
 * The season, from the caption the engine says above Seasonal: "Fall 2026" out of
 * "fall 2026 · the 40 most popular".
 */
internal fun seasonName(caption: String): String =
    caption.substringBefore('·').trim().split(' ').joinToString(" ") { word ->
        word.replaceFirstChar { it.titlecase() }
    }
