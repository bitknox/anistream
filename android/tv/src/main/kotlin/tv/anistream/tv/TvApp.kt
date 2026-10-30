// The TV process's one engine, store and player, which outlive the activity.

package tv.anistream.tv

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Intent
import android.graphics.Bitmap
import android.hardware.display.DisplayManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.util.Log
import android.view.Display
import androidx.core.net.toUri
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.plus
import kotlinx.coroutines.launch
import tv.anistream.engine.ffi.startLog
import tv.anistream.player.CaptionStyle
import tv.anistream.player.CaptionStyleWatch
import tv.anistream.player.Device
import tv.anistream.player.MpvBackend
import tv.anistream.player.PlayerConnection
import tv.anistream.player.PlayerHost
import tv.anistream.player.PlayerModel
import tv.anistream.store.DeviceWatch
import tv.anistream.store.AppHost
import tv.anistream.store.AppPhases
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
    private val scope = MainScope() + kotlinx.coroutines.CoroutineExceptionHandler { _, e ->
        // A coroutine that fails in the store is logged; it does not take the process with it.
        android.util.Log.e("anistream", "the store's work failed", e)
    }

    // The TV updates from the release's TV APK, never the phone's.
    val store: AppStore by lazy {
        AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, updateAsset = { it.androidTv })
    }

    /** The TV's own update, where the flavour has one: the release build's, or a staged fixture. */
    var updater: UpdateLauncher? = null
        private set

    private var updaterAsked = false

    /** Attach the flavour's updater, once per process, from the first activity's launch. */
    fun attachUpdater(intent: Intent) {
        if (updaterAsked) return
        updaterAsked = true
        updater = Flavour.updater(this, scope, intent)
    }

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
        watchNetwork()
        // Trailers play only when the device allows it; the engine decides from what it is told.
        DeviceWatch.watch(this) { state -> scope.launch { store.deviceState(state) } }
        ProcessLifecycleOwner.get().lifecycle.addObserver(remote)
        ProcessLifecycleOwner.get().lifecycle.addObserver(phases())
        watchNext.start()
    }

    /**
     * The app's place, told to the engine as the phone tells it: the foreground, or the
     * background, where torrents wait. Leaving the app pauses the episode (TvActivity.onStop),
     * so nothing plays there.
     */
    private fun phases() = object : DefaultLifecycleObserver {
        private val phases = AppPhases(store::send)

        override fun onStart(owner: LifecycleOwner) = phases.foreground()

        override fun onStop(owner: LifecycleOwner) = phases.background(playing = false)
    }

    /**
     * The network as the engine plans around it, as the phone says it: a metered link, Data Saver,
     * or none at all, which is the one offline state the screens show.
     */
    private fun watchNetwork() {
        val connectivity = getSystemService(ConnectivityManager::class.java) ?: return
        var said: Triple<Boolean, Boolean, Boolean>? = null
        val say = { capabilities: NetworkCapabilities? ->
            val cellular = capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true
            val constrained = connectivity.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
            val offline = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) != true
            val now = Triple(cellular, constrained, offline)
            if (now != said) {
                said = now
                scope.launch { store.networkPath(cellular, constrained, offline) }
            }
        }
        say(connectivity.getNetworkCapabilities(connectivity.activeNetwork))
        connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = say(capabilities)

            // Another network may already be the default; only none at all is offline.
            override fun onLost(network: Network) = say(connectivity.activeNetwork?.let(connectivity::getNetworkCapabilities))
        })
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
