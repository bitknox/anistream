// The rail: six icons in 48 × 48 pills centred at x 72 while collapsed, a 240-wide drawer with
// labels while focus is in it. Moving through it never switches screens; Center does.

package tv.anistream.tv.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.dp
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.NavigationDrawerScope
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.tryFocus

/** The rail's items' requesters, so Back and Left can open it on the current destination. */
@Stable
class RailFocus {
    val requesters: Map<Destination, FocusRequester> = Destination.entries.associateWith { FocusRequester() }

    /** Open the rail with focus on [destination]. */
    fun open(destination: Destination): Boolean = requesters.getValue(destination).tryFocus()
}

/**
 * The rail inside tv-material's modal drawer. Focus entering it from the content lands on the
 * current destination, whichever item is nearest; [onRight] takes focus back to the content.
 */
@Composable
fun NavigationDrawerScope.Rail(
    current: Destination,
    focus: RailFocus,
    onSelect: (Destination) -> Unit,
    modifier: Modifier = Modifier,
) {
    val open = hasFocus
    Column(
        modifier
            .fillMaxHeight()
            .width(if (open) Canvas.Drawer else Canvas.Rail)
            .then(if (open) Modifier.background(ObiSilk.Raised) else Modifier)
            .focusProperties {
                onEnter = { focus.requesters.getValue(current).tryFocus() }
            }
            // A group of its own, so the redirect runs as focus enters it rather than per item.
            .focusGroup()
            .padding(start = if (open) 24.dp else Canvas.SafeX, end = if (open) 16.dp else 0.dp, top = Canvas.SafeY, bottom = Canvas.SafeY),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Destination.entries.forEach { destination ->
            if (destination == Destination.Settings) Spacer(Modifier.weight(1f))
            RailItem(
                destination = destination,
                current = destination == current,
                open = open,
                onClick = { onSelect(destination) },
                modifier = Modifier.focusRequester(focus.requesters.getValue(destination)),
            )
            if (destination == Destination.Search) Spacer(Modifier.height(16.dp))
        }
    }
}

/**
 * One item: an icon in a pill, with its label while the rail is open. The current destination is
 * amber-light on the ground, or the amber container when open; focused, silk with a ground label.
 */
@Composable
private fun RailItem(destination: Destination, current: Boolean, open: Boolean, onClick: () -> Unit, modifier: Modifier) {
    var focused by remember { mutableStateOf(false) }
    val idle = when {
        current && open -> ObiSilk.AmberContainer
        else -> androidx.compose.ui.graphics.Color.Transparent
    }
    val content = when {
        current -> ObiSilk.OnAmberContainer
        open -> ObiSilk.Silk
        else -> ObiSilk.Haze
    }
    Surface(
        onClick = onClick,
        modifier = modifier
            .height(48.dp)
            .then(if (open) Modifier.width(Canvas.Drawer - 40.dp) else Modifier.width(48.dp))
            .onFocusChanged { focused = it.isFocused },
        shape = ClickableSurfaceDefaults.shape(CircleShape),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = idle,
            contentColor = content,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
            pressedContainerColor = ObiSilk.Silk,
            pressedContentColor = ObiSilk.Ground,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(Modifier.fillMaxHeight().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(24.dp)) {
                Icon(destination.icon, contentDescription = if (open) null else destination.label, modifier = Modifier.size(24.dp))
            }
            if (open) {
                Spacer(Modifier.width(16.dp))
                Text(destination.label, style = MaterialTheme.typography.labelLarge, maxLines = 1, modifier = Modifier.weight(1f))
                if (current && focused) {
                    Box(Modifier.padding(end = 16.dp).size(6.dp).background(ObiSilk.Amber, CircleShape))
                }
            }
        }
    }
}
