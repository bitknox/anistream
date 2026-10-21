// What the system says of the app, told to the engine and the player: the foreground or the
// background (E7's AppPhase), and the network it is on (NetworkPath).

package tv.anistream.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import tv.anistream.engine.model.AppState
import tv.anistream.engine.model.Command
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppStore

/**
 * Follows the process into the background and back, and the network it uses, for the life of the
 * process.
 *
 * - The background, as ProcessLifecycleOwner has it (no activity started; Picture in Picture keeps
 *   one started, so it is the foreground): the player turns its video track off and plays the audio
 *   on, and the engine is told, with whether an episode plays, so torrents wait but for it. Back in
 *   the foreground the video comes back, and the engine checks the connection again, binds its
 *   loopback servers again and reopens what plays.
 * - The network: cellular, and constrained for Data Saver (Android's Low Data Mode), which hold
 *   downloads and seeding. Said once, then on every change.
 */
class SystemPhases(private val context: Context, private val store: AppStore, private val player: PlayerModel) {
    private var last: Command.NetworkPath? = null
    private val main = Handler(Looper.getMainLooper())

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

        val connectivity = context.getSystemService(ConnectivityManager::class.java) ?: return
        connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
                path(capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR), connectivity)
            }
        })
        // Data Saver turned on or off changes no network, so it is listened for of its own.
        ContextCompat.registerReceiver(
            context,
            object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                    val capabilities = connectivity.getNetworkCapabilities(connectivity.activeNetwork)
                    path(capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true, connectivity)
                }
            },
            IntentFilter(ConnectivityManager.ACTION_RESTRICT_BACKGROUND_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
    }

    /** Tell the engine of the network, when it has changed. Called on the system's threads. */
    @Synchronized
    private fun path(cellular: Boolean, connectivity: ConnectivityManager) {
        val constrained = connectivity.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
        val path = Command.NetworkPath(cellular, constrained)
        if (path == last) return
        last = path
        main.post { store.send(path) }
    }
}
