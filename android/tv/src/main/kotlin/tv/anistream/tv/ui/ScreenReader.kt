// Whether a screen reader or switch access is running, kept current as it is turned on and off.

package tv.anistream.tv.ui

import android.accessibilityservice.AccessibilityServiceInfo
import android.view.accessibility.AccessibilityManager
import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/** Whether TalkBack or another spoken service, or Switch Access, is running. */
fun AccessibilityManager?.screenReaderOn(): Boolean {
    if (this == null) return false
    return isTouchExplorationEnabled ||
        getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_SPOKEN).isNotEmpty() ||
        getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK).any { it.id.contains("switchaccess", ignoreCase = true) }
}

/** [screenReaderOn], as state that follows the system setting while this composes. */
@Composable
fun rememberScreenReader(): State<Boolean> {
    val manager = LocalContext.current.getSystemService(AccessibilityManager::class.java)
    val on = remember(manager) { mutableStateOf(manager.screenReaderOn()) }
    DisposableEffect(manager) {
        val touch = AccessibilityManager.TouchExplorationStateChangeListener { on.value = manager.screenReaderOn() }
        val state = AccessibilityManager.AccessibilityStateChangeListener { on.value = manager.screenReaderOn() }
        manager?.addTouchExplorationStateChangeListener(touch)
        manager?.addAccessibilityStateChangeListener(state)
        on.value = manager.screenReaderOn()
        onDispose {
            manager?.removeTouchExplorationStateChangeListener(touch)
            manager?.removeAccessibilityStateChangeListener(state)
        }
    }
    return on
}

/** A code one character at a time, "0, 4, 2, 9, 1, 7", so a screen reader never reads it as a number. */
fun spelled(code: String): String = code.filterNot { it.isWhitespace() }.toList().joinToString(", ") // i18n-skip

/**
 * A block of words and a code as one thing a screen reader can land on and read whole: [description]
 * replaces what is inside, and a change to it is said once, politely. Focusable only while a screen
 * reader runs, so the remote's focus order stays as it was.
 */
@Composable
fun Modifier.spokenBlock(description: String, focus: FocusRequester? = null): Modifier {
    val reader by rememberScreenReader()
    return then(if (focus != null) Modifier.focusRequester(focus) else Modifier)
        .then(if (reader) Modifier.focusable() else Modifier)
        .clearAndSetSemantics {
            contentDescription = description
            liveRegion = LiveRegionMode.Polite
        }
}
