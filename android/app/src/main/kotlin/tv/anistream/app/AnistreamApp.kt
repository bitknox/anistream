// The process's one engine and store, which outlive every activity.

package tv.anistream.app

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.util.Log
import kotlinx.coroutines.MainScope
import tv.anistream.engine.ffi.startLog
import tv.anistream.player.CaptionStyle
import tv.anistream.player.CaptionStyleWatch
import tv.anistream.player.Device
import tv.anistream.player.MpvBackend
import tv.anistream.player.PlayerConnection
import tv.anistream.player.PlayerHost
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppHost
import tv.anistream.store.AppStore
import tv.anistream.store.EngineLaunch
import tv.anistream.store.KeystoreKey
import tv.anistream.store.SealedFileSecrets

class AnistreamApp : Application(), PlayerHost {
    /** Where the store's work runs: the main thread, for as long as the process. */
    private val scope = MainScope()

    val store: AppStore by lazy { AppStore(scope) }

    /**
     * The player, for the life of the process rather than an activity's: mpv and the media session
     * stay through a rotation, a fold, Picture in Picture and the app in the background.
     */
    override val player: PlayerModel by lazy {
        PlayerModel(
            send = { store.send(it) },
            backend = { MpvBackend(this) },
            scope = scope,
            power = { Device.power(this) },
            systemCaptionStyle = { CaptionStyle.system(this) },
        ).also { player ->
            store.onPlayback = player::apply
            PlayerConnection(this, player, scope)
            captions = CaptionStyleWatch(this, player)
        }
    }

    /** The player's caption style, following the setting and the system's style. */
    private var captions: CaptionStyleWatch? = null

    val notifier: Notifier by lazy { Notifier(this) }

    private var started = false

    /** Start the engine [launch] describes, once per process: the first activity's launch wins. */
    fun start(launch: EngineLaunch) {
        if (started) return
        started = true
        // The engine's log, a file a day under the cache, which never holds a token.
        startLog(launch.home.cache.path)
        Log.i(TAG, "starting the ${if (launch.engine == EngineLaunch.Engine.App) "app" else "testing"} engine")
        // Before the engine, so no playback event finds no player.
        player
        store.start(launch, host())
    }

    /** The engine's host: tokens sealed by the keystore in a private file, links, notifications. */
    private fun host() = AppHost(
        secrets = SealedFileSecrets(filesDir.resolve("secrets")) { KeystoreKey.get() },
        open = ::open,
        notify = notifier::post,
    )

    /** Open [url] in the app that handles it, the browser for most. */
    private fun open(url: String): Boolean = try {
        startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (_: ActivityNotFoundException) {
        false
    } catch (_: SecurityException) {
        false
    }

    companion object {
        const val TAG = "anistream"
    }
}
