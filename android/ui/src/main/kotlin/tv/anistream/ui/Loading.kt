// Loading to content (T8): nothing for 600 ms, then static placeholders held at least 0.3 s, then
// the content in one crossfade. Never a shimmer, a pulse or a stagger.

package tv.anistream.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import tv.anistream.design.Motion

private enum class LoadPhase { Waiting, Placeholders, Content }

/** How long nothing is drawn before placeholders, and how long they stay once shown. */
private const val PLACEHOLDERS_AFTER = 600L
private const val PLACEHOLDERS_HOLD = 300L

/**
 * [content] once [value] is here. While it is not: nothing for the first 600 ms, then
 * [placeholders] in the real layout, fading in, and staying at least 0.3 s so they never blink.
 * A value already here when this first draws shows at once; one that arrives within 600 ms fades
 * in on standard, and one that replaces placeholders crossfades with them on standard.
 *
 * With [returns], a value that goes again brings the placeholders straight back in a crossfade,
 * for a caller that has already waited its 600 ms (Search, asked anew). Otherwise the content
 * stays until the next value. [waited] says the caller waited its 600 ms before this first drew:
 * the placeholders show at once.
 */
@Composable
fun <T : Any> Loading(
    value: T?,
    modifier: Modifier = Modifier,
    returns: Boolean = false,
    waited: Boolean = false,
    placeholders: @Composable () -> Unit,
    content: @Composable (T) -> Unit,
) {
    val here = remember { value != null }
    var phase by remember {
        mutableStateOf(if (here) LoadPhase.Content else if (waited) LoadPhase.Placeholders else LoadPhase.Waiting)
    }
    var fades by remember { mutableStateOf(!here) }
    var shownAt by remember { mutableLongStateOf(System.nanoTime() / 1_000_000) }
    var latest by remember { mutableStateOf(value) }
    if (value != null) latest = value

    LaunchedEffect(Unit) {
        if (phase != LoadPhase.Waiting) return@LaunchedEffect
        delay(PLACEHOLDERS_AFTER)
        if (phase == LoadPhase.Waiting) {
            phase = LoadPhase.Placeholders
            shownAt = System.nanoTime() / 1_000_000
        }
    }
    LaunchedEffect(value != null) {
        if (value == null && returns && phase == LoadPhase.Content) {
            fades = true
            phase = LoadPhase.Placeholders
            shownAt = System.nanoTime() / 1_000_000
        }
        if (value == null || phase == LoadPhase.Content) return@LaunchedEffect
        if (phase == LoadPhase.Placeholders) {
            val shown = System.nanoTime() / 1_000_000 - shownAt
            delay((PLACEHOLDERS_HOLD - shown).coerceAtLeast(0))
        }
        phase = LoadPhase.Content
    }

    Box(modifier) {
        AnimatedVisibility(
            phase == LoadPhase.Placeholders,
            enter = fadeIn(Motion.fadeIn()),
            exit = fadeOut(Motion.standard()),
        ) { placeholders() }
        AnimatedVisibility(
            phase == LoadPhase.Content,
            enter = if (fades) fadeIn(Motion.standard()) else EnterTransition.None,
            exit = if (returns) fadeOut(Motion.standard()) else ExitTransition.None,
        ) { latest?.let { content(it) } }
    }
}

/**
 * A line of text before it arrives: a silk bar at 12 %, [fraction] of the width, centred in the
 * line's [lineHeight] so the rows keep their real height.
 */
@Composable
fun TextPlaceholder(fraction: Float, lineHeight: Dp = 20.dp, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(lineHeight), contentAlignment = Alignment.CenterStart) {
        Box(
            Modifier
                .fillMaxWidth(fraction)
                .height(lineHeight * 0.6f)
                .clip(MaterialTheme.shapes.extraSmall)
                .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f)),
        )
    }
}

/**
 * [content] only once 600 ms have passed, fading in: for placeholders that sit inside a list,
 * where [Loading] cannot wrap them, so a fast answer never shows them at all.
 */
@Composable
fun AfterWait(content: @Composable () -> Unit) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(PLACEHOLDERS_AFTER)
        shown = true
    }
    AnimatedVisibility(shown, enter = fadeIn(Motion.fadeIn()), exit = ExitTransition.None) { content() }
}
