// A Continue title's menu (F3), the same on a phone's long press and the tablet queue's More
// button: play, mark the episode watched, go to the title, then Hide from Continue and Drop. Its
// items are the row's TalkBack actions too.

package tv.anistream.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.store.ContinueItem

/** What a Continue title's menu does, for the menu and for TalkBack alike. */
internal class ContinueMenuActions(
    val play: (() -> Unit)?,
    val playLabel: String,
    val markWatched: () -> Unit,
    val markLabel: String,
    val open: () -> Unit,
    val hide: () -> Unit,
    val drop: () -> Unit,
) {
    /** The menu's items as TalkBack's actions on the row. */
    val accessibility: List<CustomAccessibilityAction>
        get() = listOfNotNull(
            play?.let { CustomAccessibilityAction(playLabel) { it(); true } },
            CustomAccessibilityAction(markLabel) { markWatched(); true },
            CustomAccessibilityAction("Hide from Continue") { hide(); true },
            CustomAccessibilityAction("Drop") { drop(); true },
        )
}

/** [item]'s menu actions, opened as a title page from [from] when it goes to the title. */
@Composable
internal fun continueMenuActions(item: ContinueItem, actions: AppActions, from: String, hide: ((ContinueItem) -> Unit)? = null): ContinueMenuActions {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val haptics = LocalHapticFeedback.current
    return ContinueMenuActions(
        play = if (item.playable) ({ actions.play(item.id, item.episode.toString()) }) else null,
        playLabel = item.action,
        markWatched = {
            // Marked watched from the queue plays T9's toggle, as from the title page.
            haptics.performHapticFeedback(HapticFeedbackType.ToggleOn)
            store.setWatched(item.id, listOf(item.episode.toString()))
        },
        markLabel = "Mark E${item.episode} watched",
        open = { navigator.openTitle(item.id, from) },
        hide = { hide?.invoke(item) ?: store.hideFromContinue(item.id) },
        drop = { store.setStatus(item.id, LibrarySegment.DROPPED) },
    )
}

/** The menu itself, on surfaceContainerHigh, its divider in outlineVariant. */
@Composable
internal fun ContinueMenu(menu: ContinueMenuActions, expanded: Boolean, onDismiss: () -> Unit) {
    Menu(expanded = expanded, onDismissRequest = onDismiss) {
        DropdownMenuItem(
            text = { Text(menu.playLabel) },
            leadingIcon = { Icon(Icons.Play, contentDescription = null) },
            enabled = menu.play != null,
            onClick = { onDismiss(); menu.play?.invoke() },
        )
        DropdownMenuItem(
            text = { Text(menu.markLabel) },
            leadingIcon = { Icon(Icons.Check, contentDescription = null) },
            onClick = { onDismiss(); menu.markWatched() },
        )
        DropdownMenuItem(
            text = { Text("Go to title") },
            leadingIcon = { Icon(Icons.External, contentDescription = null) },
            onClick = { onDismiss(); menu.open() },
        )
        HorizontalDivider(Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.outlineVariant)
        DropdownMenuItem(
            text = { Text("Hide from Continue") },
            leadingIcon = { Icon(Icons.HideRow, contentDescription = null) },
            onClick = { onDismiss(); menu.hide() },
        )
        DropdownMenuItem(
            text = { Text("Drop") },
            leadingIcon = { Icon(Icons.Clear, contentDescription = null) },
            onClick = { onDismiss(); menu.drop() },
        )
    }
}

/**
 * A tap opens the title and a long press opens the menu, with the system's long-press haptic,
 * and TalkBack offers the menu's items as the row's own actions.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun Modifier.openOrMenu(menu: ContinueMenuActions, onMenu: () -> Unit): Modifier {
    val haptics = LocalHapticFeedback.current
    return this
        .semantics { customActions = menu.accessibility }
        .combinedClickable(
            onClickLabel = "Open title",
            onLongClickLabel = "More",
            onLongClick = {
                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                onMenu()
            },
            onClick = menu.open,
        )
}

/**
 * A Continue row's swipes (spec, swipe actions): from the start, Watched for its episode; from
 * the end, Hide from Continue, gone at once with Undo in a notice.
 */
@Composable
internal fun SwipeRow(item: ContinueItem, actions: AppActions, leave: (ContinueItem) -> Unit, content: @Composable () -> Unit) {
    val menu = continueMenuActions(item, actions, "new", hide = leave)
    Swipeable(
        start = SwipeAction("Watched", Icons.Check, menu.markWatched),
        end = SwipeAction("Hide", Icons.HideRow, menu.hide),
        content = content,
    )
}
