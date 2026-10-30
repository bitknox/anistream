// The actions panel: a long press on a card opens its actions in the 400-wide column on the
// right, as the player's panels are. TV has no anchored menus.

package tv.anistream.tv.ui

import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
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

/** One row of the panel. A [then] sheet opens over the panel instead of closing it, and Back returns from it. */
data class CardAction(val label: String, val selected: Boolean = false, val then: ActionSheet? = null, val supporting: String? = null, val run: () -> Unit = {})

/** A card's actions, under its title; or, with a [body], what it draws in their place. */
data class ActionSheet(val title: String, val actions: List<CardAction> = emptyList(), val width: androidx.compose.ui.unit.Dp = 400.dp, val body: (@Composable () -> Unit)? = null)

/**
 * The panel showing now, if any; the browse UI draws it over the screen. A sheet opened while one
 * shows goes over it, and [back] returns to the one under it, as Back does.
 */
@Stable
class ActionsHost {
    private var stack by mutableStateOf(emptyList<ActionSheet>())

    val sheet: ActionSheet? get() = stack.lastOrNull()

    /** Where focus goes back once the panel closes: the screen's last focused item. */
    var onClosed: () -> Unit = {}

    fun open(sheet: ActionSheet) {
        stack = stack + sheet
    }

    /** One sheet back, or closed when this was the first. */
    fun back() {
        if (stack.size > 1) stack = stack.dropLast(1) else close()
    }

    fun close() {
        stack = emptyList()
        onClosed()
    }
}

val LocalActions = staticCompositionLocalOf<ActionsHost?> { null }

/**
 * The open sheet: a black scrim at 32 % over the screen and the panel on its right, fading in and
 * out together (fade-in, fade-out). Back returns to the sheet under it, or closes the first. Lay it
 * over the whole screen.
 */
@Composable
fun ActionsPanel(host: ActionsHost, modifier: Modifier = Modifier) {
    val open = host.sheet
    var last by remember { mutableStateOf(open) }
    if (open != null) last = open
    if (open != null) BackHandler(onBack = host::back)
    AnimatedVisibility(open != null, modifier.fillMaxSize(), enter = enterFade(), exit = exitFade()) {
        Box(Modifier.fillMaxSize()) {
            Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = SCRIM)))
            last?.let { Sheet(host, it, Modifier.align(Alignment.CenterEnd)) }
        }
    }
}

/** A panel's end padding: its rows stop at the safe area's edge, 48 in from the screen's. */
val PANEL_END = 48.dp

/** The black scrim under a panel over browse screens: 32 %. */
private const val SCRIM = 0.32f

@Composable
private fun Sheet(host: ActionsHost, sheet: ActionSheet, modifier: Modifier) {
    val first = remember(sheet) { FocusRequester() }
    Column(
        modifier
            .width(sheet.width)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            // Rows end on the safe area's edge, 48 in from the screen's.
            .padding(start = 16.dp, end = PANEL_END, top = 27.dp, bottom = 27.dp),
    ) {
        Text(
            sheet.title,
            style = MaterialTheme.typography.headlineSmall,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 16.dp, bottom = 12.dp).semantics { heading() },
        )
        val body = sheet.body
        if (body != null) {
            body()
            return@Column
        }
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
                    supporting = action.supporting,
                    // The chosen option takes focus, else the first.
                    modifier = if (action.selected || (index == 0 && sheet.actions.none { it.selected })) Modifier.focusRequester(first) else Modifier,
                )
            }
        }
    }
    LaunchedEffect(sheet) {
        // A body focuses its own.
        if (sheet.body != null) return@LaunchedEffect
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

/** The five lists, the current one marked: Library's Change status. */
fun statusSheet(context: Context, store: AppStore, id: AnilistId, title: String, current: LibrarySegment?): ActionSheet = ActionSheet(
    title,
    LibrarySegment.entries.map { segment ->
        CardAction(segment.label(context), selected = segment == current) { store.setStatus(id, segment) }
    },
)

/** A title anywhere it is not yet on a list: Add to Planning, Open the title page. */
fun planSheet(context: Context, store: AppStore, entry: Entry, onOpen: (AnilistId) -> Unit): ActionSheet = ActionSheet(
    entry.title,
    listOf(
        CardAction(Strings.tvAddToPlanning(context)) { store.setStatus(entry.id, LibrarySegment.PLANNING) },
        CardAction(Strings.tvOpenTitlePage(context)) { onOpen(entry.id) },
    ),
)
