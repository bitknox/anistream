// What the system says of the app's place, told to the engine and the player: the foreground or
// the background (E7's AppPhase). The network it is on is told from the application.

package tv.anistream.app

import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppPhases
import tv.anistream.store.AppStore

/**
 * Follows the process into the background and back, for the life of the process, as
 * ProcessLifecycleOwner has it: no activity started is the background, and Picture in Picture keeps
 * one started, so it is the foreground.
 *
 * In the background the player turns its video track off and plays the audio on, and the engine is
 * told, with whether an episode plays, so torrents wait but for it; and told again whenever that
 * changes there, paused or played on from the notification, or ended. Back in the foreground the
 * video comes back, and the engine checks the connection again, binds its loopback servers again
 * and reopens what plays.
 */
class SystemPhases(private val store: AppStore, private val player: PlayerModel, private val scope: CoroutineScope) {
    private val phases = AppPhases(store::send)

    /** Whether an episode plays: the player up, its picture shown, and not paused. */
    private fun playing(): Boolean = player.isPresented && player.hasPicture && !player.paused

    fun start() {
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                player.backgrounded(false)
                phases.foreground()
            }

            override fun onStop(owner: LifecycleOwner) {
                player.backgrounded(true)
                phases.background(playing())
            }
        })
        // The player's state is Compose state, which the screens' snapshot manager says the
        // changes of, as long as the process: read here as it changes, on the main thread.
        scope.launch {
            snapshotFlow(::playing).collect(phases::playing)
        }
    }
}
