// Back that reaches the activity while a play is on its way: before the player's own screen is
// there to take it.

package tv.anistream.player

import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.compose.runtime.snapshotFlow
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch

/**
 * Take Back for [player] while a Play is on its way or its player has just come up: see
 * [PlayerModel.backBeforeShown]. Added before the content, so every screen's own Back, the
 * player's among them, comes first. When the player was not up, the Back goes on to the system,
 * which puts the app behind.
 */
fun ComponentActivity.takeBackBeforeShown(player: PlayerModel) {
    val callback = object : OnBackPressedCallback(false) {
        override fun handleOnBackPressed() {
            if (player.backBeforeShown()) return
            isEnabled = false
            onBackPressedDispatcher.onBackPressed()
        }
    }
    onBackPressedDispatcher.addCallback(this, callback)
    lifecycleScope.launch {
        snapshotFlow { player.playAsked || player.isPresented }.collect { callback.isEnabled = it }
    }
}
