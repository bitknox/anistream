// Notices: the engine's, and the screens' own (marking watched, with Undo), as snackbars above
// the navigation bar.

package tv.anistream.ui

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarData
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.engine.model.NoticeHint

/**
 * How long a snackbar stays: 4 s, 8 s with a button, plus 1 s for every 10 words past 12, so a
 * longer one stays long enough to read.
 */
fun snackbarMillis(text: String, hasAction: Boolean): Long {
    val words = text.split(Regex("\\s+")).count { it.isNotEmpty() }
    val extra = if (words > 12) ((words - 12) + 9) / 10 * 1_000L else 0L
    return (if (hasAction) 8_000L else 4_000L) + extra
}

/** The snackbars, for the screens to post their own. */
@Stable
class Snackbars(val host: SnackbarHostState, private val scope: CoroutineScope) {
    /**
     * Show [text], with [action] as its button: dismissed after [snackbarMillis], and [onAction]
     * run when the button is pressed. A newer snackbar replaces this one.
     */
    fun show(text: String, action: String? = null, onAction: () -> Unit = {}) {
        scope.launch {
            host.currentSnackbarData?.dismiss()
            val result = withTimeoutOrNull(snackbarMillis(text, action != null)) {
                host.showSnackbar(text, actionLabel = action, duration = SnackbarDuration.Indefinite)
            }
            if (result == null) host.currentSnackbarData?.dismiss()
            if (result == SnackbarResult.ActionPerformed) onAction()
        }
    }
}

val LocalSnackbars = staticCompositionLocalOf<Snackbars> { error("no Snackbars in this composition") }

/**
 * A snackbar as AndroidFoundations draws it: surfaceContainerHigh with silk text at corners 4,
 * its button a silk text button.
 */
@Composable
internal fun NoticeSnackbar(data: SnackbarData) {
    val colors = MaterialTheme.colorScheme
    Snackbar(
        data,
        shape = MaterialTheme.shapes.extraSmall,
        containerColor = colors.surfaceContainerHigh,
        contentColor = colors.onSurface,
        actionColor = colors.onSurface,
        actionContentColor = colors.onSurface,
        dismissActionContentColor = colors.onSurface,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
    )
}

/** The words on a notice's button, for what can be done about it. */
fun NoticeHint.label(): String = when (this) {
    NoticeHint.STOP_PLAYBACK -> "Stop"
    NoticeHint.OPEN_STORES -> "Open stores"
    NoticeHint.KITSU_CREDENTIALS -> "Sign in"
}

/** Each notice the engine posts, as a snackbar, with its hint as the button. */
@Composable
internal fun EngineNotices(onHint: (NoticeHint) -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val current by store.notices.current.collectAsStateWithLifecycle()
    LaunchedEffect(current?.id) {
        val item = current ?: return@LaunchedEffect
        val hint = item.notice.hint
        snackbars.show(item.notice.text, hint?.label()) { hint?.let(onHint) }
        store.notices.dismiss(item)
    }
}
