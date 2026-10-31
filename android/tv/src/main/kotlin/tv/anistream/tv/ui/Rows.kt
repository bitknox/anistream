// List rows, tabs and chips with the brief's focus look: no fill and a silk label at rest, silk
// with a ground label when focused, the amber container when selected. No scale on any of them.

package tv.anistream.tv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.grid.LazyGridItemScope
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.withFrameMillis
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
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
import tv.anistream.design.LanguageFlag
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk

/**
 * A list row, [height] high: the label in silk, a supporting line in haze, the current value in
 * haze on the right, or a switch. Focused, all of it turns ground on silk. Selected, it sits in the
 * amber container with its lines in the container's own colour, and its amber dot's place kept,
 * so focus arriving shows the dot without moving the value.
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
    /** A state's dot before the supporting line: vermilion for a problem, water for a pass. */
    dot: Color? = null,
    /** Lines the supporting text may wrap to. */
    supportingLines: Int = 1,
    /** A flag before the headline (a language's), decoration only. */
    flagRes: Int? = null,
    /** A flag before the value. */
    valueFlagRes: Int? = null,
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
            if (flagRes != null) {
                LanguageFlag(flagRes)
                Spacer(Modifier.width(12.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(headline, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (supporting != null) {
                    val quiet = if (focused || selected) LocalContentColor.current else ObiSilk.Haze
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (dot != null) {
                            // Level with the first line, however many the words wrap to.
                            val line = with(LocalDensity.current) { MaterialTheme.typography.bodyMedium.lineHeight.toDp() }
                            Box(Modifier.height(line), contentAlignment = Alignment.Center) {
                                Box(Modifier.size(8.dp).background(dot, CircleShape))
                            }
                        }
                        Text(supporting, style = MaterialTheme.typography.bodyMedium, color = quiet, maxLines = supportingLines, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
            if (value != null) {
                Spacer(Modifier.width(16.dp))
                val quiet = if (focused || selected) LocalContentColor.current else ObiSilk.Haze
                if (valueFlagRes != null) {
                    LanguageFlag(valueFlagRes)
                    Spacer(Modifier.width(8.dp))
                }
                Text(value, style = MaterialTheme.typography.bodyMedium, color = quiet, maxLines = 1)
            }
            if (switch != null) {
                Spacer(Modifier.width(16.dp))
                SwitchMark(switch, focused)
            }
            if (selected) {
                Spacer(Modifier.width(12.dp))
                Box(Modifier.size(6.dp).background(if (focused) ObiSilk.Amber else Color.Transparent, CircleShape))
            }
        }
    }
}

/**
 * A row's words with nothing to do on Center: laid out as [TvListRow], silk headline and haze
 * supporting text and value, but focus passes it by.
 */
@Composable
fun TvInfoRow(headline: String, modifier: Modifier = Modifier, supporting: String? = null, value: String? = null) {
    Row(
        modifier.fillMaxWidth().heightIn(min = if (supporting != null) 72.dp else 56.dp).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(headline, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (supporting != null) {
                Text(supporting, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (value != null) {
            Spacer(Modifier.width(16.dp))
            Text(value, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1)
        }
    }
}

/**
 * A switch drawn as the row's state, not focusable itself, in tv-material's colours: on, an amber
 * track with a ground handle; off, a #343A57 track with a haze edge and a haze handle. On a
 * focused (silk) row the off switch draws in the ground colour.
 */
@Composable
private fun SwitchMark(on: Boolean, focused: Boolean) {
    val track = if (on) ObiSilk.Amber else if (focused) Color.Transparent else ObiSilk.RaisedSelected
    val edge = if (focused) ObiSilk.Ground else ObiSilk.Haze
    val handle = when {
        on -> ObiSilk.Ground
        focused -> ObiSilk.Ground
        else -> ObiSilk.Haze
    }
    Box(
        Modifier.size(44.dp, 24.dp)
            .background(track, CircleShape)
            .then(if (on) Modifier else Modifier.border(2.dp, edge, CircleShape))
            .padding(if (on) 3.dp else 5.dp),
    ) {
        Box(
            Modifier.align(if (on) Alignment.CenterEnd else Alignment.CenterStart)
                .size(if (on) 18.dp else 14.dp)
                .background(handle, CircleShape),
        )
    }
}

/**
 * A pill tab or chip: [selected] in the amber container, focused silk, with an amber dot whose
 * place a selected one always keeps, so focus arriving never widens it. Tabs select on focus once
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
    chip: Boolean = false,
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
            containerColor = when {
                selected -> ObiSilk.AmberContainer
                chip -> ObiSilk.Raised
                else -> Color.Transparent
            },
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
            if (selected) {
                Spacer(Modifier.width(8.dp))
                Box(Modifier.size(6.dp).background(if (focused) ObiSilk.Amber else Color.Transparent, CircleShape))
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
 * Whether a loading screen shows its placeholders: nothing for 600 ms, so a fast answer never
 * flashes them, then placeholders, held at least 0.3 s once shown, so they never flicker away.
 * While this holds, the screen shows the placeholders and not the content that has come.
 */
@Composable
fun placeholdersAfterWait(loading: Boolean): Boolean {
    var shown by remember { mutableStateOf(false) }
    var since by remember { mutableLongStateOf(0L) }
    LaunchedEffect(loading) {
        if (loading) {
            if (!shown) {
                delay(PLACEHOLDER_WAIT)
                since = withFrameMillis { it }
                shown = true
            }
        } else if (shown) {
            val left = PLACEHOLDER_HOLD - (withFrameMillis { it } - since)
            if (left > 0) delay(left)
            shown = false
        }
    }
    return shown
}

private const val PLACEHOLDER_WAIT = 600L
private const val PLACEHOLDER_HOLD = 300L

/**
 * Placeholders and content swap in one crossfade on standard: the placeholders fade out as the
 * items fade in, in place. Nothing moves, and with Remove animations they swap at once.
 */
@Composable
fun LazyItemScope.swapFade(): Modifier =
    if (LocalReducedMotion.current) Modifier else Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = null, fadeOutSpec = Motion.standard())

/** The same crossfade, in a grid. */
@Composable
fun LazyGridItemScope.swapFade(): Modifier =
    if (LocalReducedMotion.current) Modifier else Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = null, fadeOutSpec = Motion.standard())
