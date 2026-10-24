// Back at depth on a browse screen: scroll to the top and focus the first focus. At the top
// already, Back goes on to the rail.

package tv.anistream.tv.screens

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.FocusMemory

/**
 * Answer the navigator's Back at depth for this screen: when it is [scrolled] or focus is off its
 * first focus, [scrollToTop] and focus the first focus, and say so.
 */
@Composable
fun ToTop(nav: TvNavigator, memory: FocusMemory, scrolled: () -> Boolean, scrollToTop: suspend () -> Unit) {
    val scope = rememberCoroutineScope()
    DisposableEffect(nav, memory) {
        val mine = {
            val deep = scrolled() || !memory.atFirst
            if (deep) {
                scope.launch {
                    scrollToTop()
                    memory.reset()
                    memory.focusFirstSoon()
                }
            }
            deep
        }
        nav.toTop = mine
        // A screen fading out after the next one came in leaves the newcomer's answer be.
        onDispose { if (nav.toTop === mine) nav.toTop = null }
    }
}
