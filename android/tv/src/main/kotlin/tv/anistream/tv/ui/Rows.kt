// List rows, tabs and chips with the brief's focus look: no fill and a silk label at rest, silk
// with a ground label when focused, the amber container when selected. No scale on any of them.

package tv.anistream.tv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import tv.anistream.design.ObiSilk

/**
 * A list row, [height] high: the label in silk, a supporting line in haze, the current value in
 * haze on the right, or a switch. Focused, all of it turns ground on silk.
 */
@Composable
fun TvListRow(
    headline: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    supporting: String? = null,
    value: String? = null,
    switch: Boolean? = null,
    selected: Boolean = false,
    icon: ImageVector? = null,
    enabled: Boolean = true,
    height: Dp = if (supporting != null) 72.dp else 56.dp,
) {
    var focused by remember { mutableStateOf(false) }
    Surface(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.fillMaxWidth().heightIn(min = height).onFocusChanged { focused = it.isFocused },
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = if (selected) ObiSilk.AmberContainer else Color.Transparent,
            contentColor = if (selected) ObiSilk.OnAmberContainer else ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
            pressedContainerColor = ObiSilk.Silk,
            pressedContentColor = ObiSilk.Ground,
            disabledContainerColor = Color.Transparent,
            disabledContentColor = ObiSilk.Silk.copy(alpha = 0.38f),
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = height).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (icon != null) {
                Icon(icon, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(16.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(headline, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (supporting != null) {
                    val quiet = if (focused) LocalContentColor.current else ObiSilk.Haze
                    Text(supporting, style = MaterialTheme.typography.bodyMedium, color = quiet, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (value != null) {
                Spacer(Modifier.width(16.dp))
                val quiet = if (focused) LocalContentColor.current else ObiSilk.Haze
                Text(value, style = MaterialTheme.typography.bodyMedium, color = quiet, maxLines = 1)
            }
            if (switch != null) {
                Spacer(Modifier.width(16.dp))
                SwitchMark(switch, focused)
            }
            if (selected && focused) {
                Spacer(Modifier.width(12.dp))
                Box(Modifier.size(6.dp).background(ObiSilk.Amber, CircleShape))
            }
        }
    }
}

/** A switch drawn as the row's state: a track and a thumb, amber when on. Not focusable itself. */
@Composable
private fun SwitchMark(on: Boolean, focused: Boolean) {
    val track = when {
        on -> ObiSilk.Amber
        focused -> ObiSilk.Ground.copy(alpha = 0.3f)
        else -> ObiSilk.Rule
    }
    Box(Modifier.size(44.dp, 24.dp).background(track, CircleShape).padding(3.dp)) {
        Box(
            Modifier.align(if (on) Alignment.CenterEnd else Alignment.CenterStart)
                .size(18.dp)
                .background(if (on) ObiSilk.Ground else ObiSilk.Silk, CircleShape),
        )
    }
}

/**
 * A pill tab or chip: [selected] in the amber container, focused silk. Tabs select on focus once
 * focus rests on them [settle] ms, so passing through does not load each.
 */
@Composable
fun TvTab(
    label: String,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
    settle: Long? = 250,
    count: String? = null,
) {
    var focused by remember { mutableStateOf(false) }
    if (settle != null) {
        LaunchedEffect(focused, selected) {
            if (focused && !selected) {
                delay(settle)
                onSelect()
            }
        }
    }
    Surface(
        onClick = onSelect,
        modifier = modifier.height(40.dp).onFocusChanged { focused = it.isFocused },
        shape = ClickableSurfaceDefaults.shape(CircleShape),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = if (selected) ObiSilk.AmberContainer else Color.Transparent,
            contentColor = if (selected) ObiSilk.OnAmberContainer else ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
            pressedContainerColor = ObiSilk.Silk,
            pressedContentColor = ObiSilk.Ground,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(Modifier.padding(PaddingValues(horizontal = 16.dp)).height(40.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1)
            if (count != null) {
                Spacer(Modifier.width(8.dp))
                Text(count, style = MaterialTheme.typography.labelMedium, maxLines = 1, color = LocalContentColor.current.copy(alpha = 0.72f))
            }
            if (selected && focused) {
                Spacer(Modifier.width(8.dp))
                Box(Modifier.size(6.dp).background(ObiSilk.Amber, CircleShape))
            }
        }
    }
}

/** A grid or list's placeholder text: a silk bar at 12 %. */
@Composable
fun TextBar(width: Dp, modifier: Modifier = Modifier, height: Dp = 12.dp) {
    Box(modifier.size(width, height).background(ObiSilk.Silk.copy(alpha = 0.12f), RoundedCornerShape(4.dp)))
}

/**
 * Whether a loading screen shows its placeholders yet: nothing for 600 ms, then placeholders, so a
 * fast answer never flashes them.
 */
@Composable
fun placeholdersAfterWait(loading: Boolean): Boolean {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(loading) {
        if (!loading) {
            shown = false
        } else {
            delay(PLACEHOLDER_WAIT)
            shown = true
        }
    }
    return loading && shown
}

private const val PLACEHOLDER_WAIT = 600L
