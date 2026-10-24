// The actions panel: a long press on a card opens its actions in the 400-wide column on the
// right, as the player's panels are. TV has no anchored menus.

package tv.anistream.tv.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.Entry
import tv.anistream.store.AppStore
import tv.anistream.store.label

/** One row of the panel. A [then] sheet replaces the panel instead of closing it. */
data class CardAction(val label: String, val selected: Boolean = false, val then: ActionSheet? = null, val run: () -> Unit = {})

/** A card's actions, under its title. */
data class ActionSheet(val title: String, val actions: List<CardAction>)

/** The panel showing now, if any; the browse UI draws it over the screen. */
@Stable
class ActionsHost {
    var sheet by mutableStateOf<ActionSheet?>(null)
        private set

    /** Where focus goes back once the panel closes: the screen's last focused item. */
    var onClosed: () -> Unit = {}

    fun open(sheet: ActionSheet) {
        this.sheet = sheet
    }

    fun close() {
        sheet = null
        onClosed()
    }
}

val LocalActions = staticCompositionLocalOf<ActionsHost?> { null }

/** The open sheet, right-aligned in [modifier]'s box; Back closes it. */
@Composable
fun ActionsPanel(host: ActionsHost, modifier: Modifier = Modifier) {
    val sheet = host.sheet ?: return
    BackHandler(onBack = host::close)
    val first = remember(sheet) { FocusRequester() }
    Column(
        modifier
            .width(400.dp)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(horizontal = 16.dp, vertical = 27.dp),
    ) {
        Text(sheet.title, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            itemsIndexed(sheet.actions, key = { index, action -> "$index/${action.label}" }) { index, action ->
                TvListRow(
                    action.label,
                    onClick = {
                        if (action.then != null) {
                            host.open(action.then)
                        } else {
                            host.close()
                            action.run()
                        }
                    },
                    selected = action.selected,
                    modifier = if (index == 0) Modifier.focusRequester(first) else Modifier,
                )
            }
        }
    }
    LaunchedEffect(sheet) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

/** The five lists, the current one marked: Library's Change status. */
fun statusSheet(store: AppStore, id: AnilistId, title: String, current: LibrarySegment?): ActionSheet = ActionSheet(
    title,
    LibrarySegment.entries.map { segment ->
        CardAction(segment.label.replaceFirstChar { it.uppercase() }, selected = segment == current) { store.setStatus(id, segment) }
    },
)

/** A title anywhere it is not yet on a list: Add to Planning, Open the title page. */
fun planSheet(store: AppStore, entry: Entry, onOpen: (AnilistId) -> Unit): ActionSheet = ActionSheet(
    entry.title,
    listOf(
        CardAction("Add to Planning") { store.setStatus(entry.id, LibrarySegment.PLANNING) },
        CardAction("Open the title page") { onOpen(entry.id) },
    ),
)
