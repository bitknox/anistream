// The motion every list item and menu shares, as motion-android.md's implementation rules have it.

package tv.anistream.ui

import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.grid.LazyGridItemScope
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import tv.anistream.design.Motion

/**
 * A lazy list's item as it comes, goes and moves: fading in and out and finding its new place on
 * standard, every item at once, never staggered.
 */
fun LazyItemScope.itemMotion(): Modifier =
    Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = Motion.standard(), fadeOutSpec = Motion.standard())

/** [itemMotion] for a lazy grid's item. */
fun LazyGridItemScope.itemMotion(): Modifier =
    Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = Motion.standard(), fadeOutSpec = Motion.standard())

/** A dropdown menu as T7 draws them: surfaceContainerHigh at corners 4, its own motion. */
@Composable
fun Menu(
    expanded: Boolean,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    offset: DpOffset = DpOffset(0.dp, 0.dp),
    content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit,
) {
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        offset = offset,
        shape = MaterialTheme.shapes.extraSmall,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        content = content,
    )
}
