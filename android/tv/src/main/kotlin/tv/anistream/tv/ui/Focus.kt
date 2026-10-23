// Focus as the brief's model has it: one focused item while the browse UI shows, a first focus per
// screen, focus back on the opener when a screen returns, and the focus look on art (scale and a
// silk ring, on quick, snapping while a key is held).

package tv.anistream.tv.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk

/**
 * Whether a key is being held: the remote repeats it, twenty presses a second, and focus scale
 * snaps rather than animating so it never falls behind. The root reads every key event first.
 */
@Stable
class KeyRepeat {
    var held by mutableStateOf(false)
        private set

    /** Read [event] on its way to the focused item; never consumes it. */
    fun saw(event: KeyEvent): Boolean {
        val native = event.nativeKeyEvent
        held = event.type == KeyEventType.KeyDown && native.repeatCount > 0
        return false
    }
}

val LocalKeyRepeat = staticCompositionLocalOf { KeyRepeat() }

/**
 * Where a screen's focus was, so it can be put back: on returning from a title page, closing a
 * panel, or moving Right out of the rail. Items register under a stable key (a title's id, a
 * setting's key), never an index, so a list that changed while away keeps focus on the same item.
 * Held by the root for each screen, so it outlives the screen's composition.
 */
@Stable
class FocusMemory {
    /** The key of the item focused last. */
    var last: String? = null
        private set

    private val requesters = mutableMapOf<String, FocusRequester>()

    /** The screen's first focus, where it starts and where Back at depth returns. */
    val first = FocusRequester()

    /** The key of the item holding the first focus. */
    var firstKey: String? = null
        internal set

    /** Whether focus is on the first focus, or was never anywhere else. */
    val atFirst: Boolean get() = last == null || last == firstKey

    internal fun register(key: String, requester: FocusRequester) {
        requesters[key] = requester
    }

    internal fun unregister(key: String, requester: FocusRequester) {
        if (requesters[key] === requester) requesters.remove(key)
    }

    internal fun focused(key: String) {
        last = key
    }

    /** Forget where focus was, so the screen starts at its first focus. */
    fun reset() {
        last = null
    }

    /** Focus the item focused last; false when there is none on screen. */
    fun restore(): Boolean {
        val key = last ?: return false
        return requesters[key]?.tryFocus() == true
    }

    /** Focus the item remembered under [key]; false when it is not on screen. */
    fun focus(key: String): Boolean = requesters[key]?.tryFocus() == true

    /** Focus the screen's first focus; false when it is not on screen. */
    fun focusFirst(): Boolean = first.tryFocus()

    /** Restore, else the first focus. */
    fun enter(): Boolean = restore() || focusFirst()

    /** The first focus once it is laid out: a few frames, while a list scrolls back to it. */
    suspend fun focusFirstSoon() {
        repeat(FOCUS_ATTEMPTS) {
            androidx.compose.runtime.withFrameNanos { }
            if (focusFirst()) return
        }
    }
}

/** Request focus, quietly false when the requester is not attached to anything on screen. */
fun FocusRequester.tryFocus(): Boolean = runCatching { requestFocus() }.getOrDefault(false)

/** This item is remembered under [key] in [memory], and focused again when the screen returns. */
@Composable
fun Modifier.remembered(memory: FocusMemory, key: String): Modifier {
    val requester = remember(key) { FocusRequester() }
    DisposableEffect(memory, key, requester) {
        memory.register(key, requester)
        onDispose { memory.unregister(key, requester) }
    }
    return this
        .focusRequester(requester)
        .onFocusChanged { if (it.isFocused) memory.focused(key) }
}

/**
 * This item is remembered under [key], and when [isFirst], it is the screen's first focus: where
 * the screen starts, and where Back at depth brings focus.
 */
@Composable
fun Modifier.firstFocus(memory: FocusMemory, key: String, isFirst: Boolean = true): Modifier {
    if (isFirst) {
        androidx.compose.runtime.SideEffect { memory.firstKey = key }
        return focusRequester(memory.first).remembered(memory, key)
    }
    return remembered(memory, key)
}

/**
 * Focus [memory]'s last item once the screen is laid out, else its first focus. Content arriving
 * late never moves focus: this runs once per entry, keyed by [entry].
 */
@Composable
fun EnterFocus(memory: FocusMemory, entry: Any = Unit) {
    LaunchedEffect(memory, entry) {
        // A frame, so the items have attached their requesters; a few more while lists lay out.
        repeat(FOCUS_ATTEMPTS) {
            androidx.compose.runtime.withFrameNanos { }
            if (memory.enter()) return@LaunchedEffect
        }
    }
}

private const val FOCUS_ATTEMPTS = 10

/** The focus look on art, from the brief: the scale for its size and the ring 3 dp outside it. */
object ArtFocus {
    /** 1.05 for anything 100 dp or wider, 1.1 for smaller cards. */
    fun scale(width: Dp): Float = if (width >= 100.dp) 1.05f else 1.1f

    /** The silk ring, 3 dp wide at 3 dp outside the art, its corners the art's plus the gap. */
    fun ring(radius: Dp = 8.dp) = Border(
        border = BorderStroke(3.dp, ObiSilk.Silk),
        inset = 3.dp,
        shape = RoundedCornerShape(radius + 3.dp),
    )
}

/**
 * Scale on focus, on `graphicsLayer` so a focus move never re-lays out a row: on quick, or at once
 * while a key is held.
 */
@Composable
fun Modifier.focusScale(focused: Boolean, scale: Float): Modifier {
    val repeat = LocalKeyRepeat.current
    val value = remember { Animatable(1f) }
    LaunchedEffect(focused) {
        val target = if (focused) scale else 1f
        if (repeat.held) value.snapTo(target) else value.animateTo(target, Motion.quick())
    }
    return graphicsLayer {
        scaleX = value.value
        scaleY = value.value
    }
}
