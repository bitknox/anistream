// What the system says of the app's place, told to the engine and the player: the foreground or
// the background (E7's AppPhase). The network it is on is told from the application.

package tv.anistream.app

import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import tv.anistream.engine.model.AppState
import tv.anistream.engine.model.Command
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppStore

/**
 * Follows the process into the background and back, for the life of the process, as
 * ProcessLifecycleOwner has it: no activity started is the background, and Picture in Picture keeps
 * one started, so it is the foreground.
 *
 * In the background the player turns its video track off and plays the audio on, and the engine is
 * told, with whether an episode plays, so torrents wait but for it. Back in the foreground the video
 * comes back, and the engine checks the connection again, binds its loopback servers again and
 * reopens what plays.
 */
class SystemPhases(private val store: AppStore, private val player: PlayerModel) {
    fun start() {
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                player.backgrounded(false)
                store.send(Command.AppPhase(AppState.Foreground))
            }

            override fun onStop(owner: LifecycleOwner) {
                player.backgrounded(true)
                val playing = player.isPresented && player.hasPicture && !player.paused
                store.send(Command.AppPhase(AppState.Background(playing)))
            }
        })
    }
}
