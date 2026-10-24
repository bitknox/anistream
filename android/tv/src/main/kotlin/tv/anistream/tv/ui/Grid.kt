// The poster grid Seasonal and the Library share: six columns of 118 from the keyline, the focused
// poster kept inside the safe area, up and down staying in their column. Grids do not remember.

package tv.anistream.tv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import tv.anistream.engine.model.Entry

/** The columns of a full-width poster grid. */
const val GRID_COLUMNS = 6

@Composable
fun PosterGrid(
    entries: List<Entry>,
    memory: FocusMemory,
    keyPrefix: String,
    onOpen: (Entry) -> Unit,
    modifier: Modifier = Modifier,
    state: LazyGridState = rememberLazyGridState(),
    line: (Entry) -> String? = { null },
    progress: (Entry) -> Float? = { null },
    placeholders: Int = 0,
    sheet: ((Entry) -> ActionSheet)? = null,
    dot: (Entry) -> Color? = { null },
    top: LazyGridScope.() -> Unit = {},
) {
    val density = LocalDensity.current
    val spec = remember(density) {
        with(density) { SafeSpec(leading = (Canvas.SafeY + Canvas.FocusRoom).toPx(), trailing = (Canvas.SafeY + 40.dp).toPx()) }
    }
    ScrollRule(spec) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(GRID_COLUMNS),
            state = state,
            modifier = modifier.fillMaxSize(),
            contentPadding = PaddingValues(
                start = Canvas.Keyline,
                end = (960.dp - Canvas.SafeRight).coerceAtLeast(Canvas.SafeX),
                top = Canvas.SafeY,
                bottom = 80.dp,
            ),
            horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            top()
            items(placeholders, key = { "placeholder/$it" }, contentType = { "poster" }) {
                Box(swapFade()) { CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight) }
            }
            // While the placeholders hold, they stand in for what has come; then one crossfade.
            val shown = if (placeholders > 0) emptyList() else entries
            items(shown, key = { it.id.toLong() }, contentType = { "poster" }) { entry ->
                Box(swapFade()) {
                    PosterCard(
                        entry,
                        onClick = { onOpen(entry) },
                        line = line(entry),
                        progress = progress(entry),
                        sheet = sheet?.let { open -> { open(entry) } },
                        dot = dot(entry),
                        modifier = Modifier
                            .firstFocus(memory, "$keyPrefix/${entry.id}", entry === shown.first()),
                    )
                }
            }
        }
    }
}
