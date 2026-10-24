// The TV process's one engine, store and player, which outlive the activity.

package tv.anistream.tv

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Intent
import android.graphics.Bitmap
import android.hardware.display.DisplayManager
import android.util.Log
import android.view.Display
import androidx.core.net.toUri
import androidx.lifecycle.ProcessLifecycleOwner
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
import tv.anistream.store.traced
import tv.anistream.tv.home.WatchNext
import tv.anistream.tv.remote.TvRemoteHost
import tv.anistream.tv.ui.TV_ART_BUDGET
import tv.anistream.tv.ui.TvArtDecoder

class TvApp : Application(), PlayerHost {
    /** Where the store's work runs: the main thread, for as long as the process. */
    private val scope = MainScope()

    val store: AppStore by lazy { AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET) }

    /** The player, for the life of the process: mpv and the media session outlive the activity. */
    override val player: PlayerModel by lazy {
        PlayerModel(
            send = { store.send(it) },
            backend = { MpvBackend(this) },
            scope = scope,
            power = { Device.power(this) },
            display = {
                getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY)
                    ?.let { Device.display(it, resources.displayMetrics.density) }
            },
            systemCaptionStyle = { CaptionStyle.system(this) },
        ).also { player ->
            store.onPlayback = player::apply
            PlayerConnection(this, player, scope)
            captions = CaptionStyleWatch(this, player)
        }
    }

    /** The remote phones drive this TV with, listening while the app is on screen. */
    val remote: TvRemoteHost by lazy { TvRemoteHost(this, store, scope) }

    /** The launcher's Watch Next row, written from the Continue queue. */
    val watchNext: WatchNext by lazy {
        WatchNext(this, store, scope, ComponentName(this, TvActivity::class.java))
    }

    /** The player's caption style, following the setting and the system's style. */
    private var captions: CaptionStyleWatch? = null

    /** The art the screens have decoded, for the media session. */
    override fun artwork(url: String): Bitmap? = store.artwork.image(url)?.image

    private var started = false

    /** Start the engine [launch] describes, once per process: the first activity's launch wins. */
    fun start(launch: EngineLaunch) {
        if (started) return
        started = true
        Log.i(TAG, "starting the ${if (launch.engine == EngineLaunch.Engine.App) "app" else "testing"} engine")
        // Before the engine, so no playback event finds no player.
        player
        val prepare: () -> Unit = {
            traced("startLog") { startLog(launch.home.cache.path) }
            traced("flavour") { Flavour.prepare(this) }
        }
        store.start(launch, host(), prepare)
        ProcessLifecycleOwner.get().lifecycle.addObserver(remote)
        watchNext.start()
    }

    private fun host() = AppHost(
        secrets = SealedFileSecrets(filesDir.resolve("secrets")) { KeystoreKey.get() },
        open = ::open,
    )

    /** Open [url] in the app that handles it, where the TV has one. */
    private fun open(url: String): Boolean = try {
        startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (_: ActivityNotFoundException) {
        false
    } catch (_: SecurityException) {
        false
    }

    companion object {
        const val TAG = "anistream-tv"
    }
}
