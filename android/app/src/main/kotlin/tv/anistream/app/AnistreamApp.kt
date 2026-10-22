// The process's one engine and store, which outlive every activity.

package tv.anistream.app

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Intent
import android.graphics.Bitmap
import android.hardware.display.DisplayManager
import android.view.Display
import android.net.Uri
import android.util.Log
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Environment
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.SettingsCatalog
import tv.anistream.engine.ffi.startLog
import tv.anistream.engine.core.AnilistId
import tv.anistream.player.Device
import tv.anistream.player.MpvBackend
import tv.anistream.player.PlayerConnection
import tv.anistream.player.PlayerHost
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppHost
import tv.anistream.store.BackgroundEngine
import tv.anistream.store.EpisodeCheck
import tv.anistream.store.AppStore
import tv.anistream.store.EngineLaunch
import tv.anistream.store.KeystoreKey
import tv.anistream.store.SealedFileSecrets
import tv.anistream.store.traced

class AnistreamApp : Application(), PlayerHost, NewEpisodesHost {
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
            display = {
                getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY)
                    ?.let { Device.display(it, resources.displayMetrics.density) }
            },
        ).also { player ->
            store.onPlayback = player::apply
            PlayerConnection(this, player, scope)
            SystemPhases(store, player).start()
        }
    }

    override val notificationIcon: Int get() = R.drawable.ic_launcher_monochrome

    /** The art the screens have decoded, for the notification; asked for when it has not come yet. */
    override fun artwork(url: String): Bitmap? {
        store.artwork.image(url)?.let { return it.image }
        store.artwork.request(url, NOTIFICATION_ART)
        return null
    }

    val notifier: Notifier by lazy { Notifier(this) }

    override val episodeNotifier: EpisodeNotifier by lazy { EpisodeNotifier(this) }

    /** The check for new episodes: none until this build's engine has `CheckNewEpisodes`. */
    override val episodeCheck: EpisodeCheck? = null

    /** The new-episode notifications switch, and the schedule it keeps. */
    val episodeAlerts: EpisodeAlerts by lazy { EpisodeAlerts(this, episodeNotifier, available = episodeCheck != null) }

    /**
     * The engine work in the background runs on: the app's own once it has started, or one of its
     * own, quiet, while the app is not running.
     */
    private val background by lazy {
        BackgroundEngine(
            running = { store.takeIf { started && store.phase.value.let { it == AppStore.Phase.Starting || it == AppStore.Phase.Running } } },
            launch = { Flavour.background(this) },
            host = ::host,
        )
    }

    override suspend fun markWatched(id: AnilistId, episode: UInt) {
        background.use { it.setWatched(id, listOf(episode.toString())) }
    }

    private var started = false

    /** Start the engine [launch] describes, once per process: the first activity's launch wins. */
    fun start(launch: EngineLaunch) {
        if (started) return
        started = true
        Log.i(TAG, "starting the ${if (launch.engine == EngineLaunch.Engine.App) "app" else "testing"} engine")
        // Before the engine, so no playback event finds no player.
        player
        // The engine's log first, a file a day under the cache, which never holds a token: the first
        // call into the engine's library, which loads it and JNA, off the main thread with the start.
        val prepare: () -> Unit = { traced("startLog") { startLog(launch.home.cache.path) } }
        if (background.busy) {
            // Background work has an engine of its own on the same files: it stops first.
            scope.launch {
                background.stop()
                store.start(launch, host(), prepare)
            }
        } else {
            store.start(launch, host(), prepare)
        }
        episodeAlerts.reconcile()
        ContinueShortcut(this).follow(store, scope)
        watchNetwork()
        // The fixtures' downloads are seeded where they are; only the app's are kept here.
        if (!Flavour.FIXTURES) keepDownloadsInAppSpace()
    }

    /**
     * Tell the engine the network the device is on, as it changes: cellular, and Data Saver as
     * constrained. Either holds downloads and seeding as the settings say.
     */
    private fun watchNetwork() {
        val connectivity = getSystemService(ConnectivityManager::class.java) ?: return
        var said: Pair<Boolean, Boolean>? = null
        val say = { capabilities: NetworkCapabilities? ->
            val cellular = capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true
            val constrained = connectivity.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
            val now = cellular to constrained
            if (now != said) {
                said = now
                scope.launch { store.networkPath(cellular, constrained) }
            }
        }
        say(connectivity.getNetworkCapabilities(connectivity.activeNetwork))
        connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = say(capabilities)
            override fun onLost(network: Network) = say(null)
        })
    }

    /**
     * Finished downloads go to the app's own space on the device, which needs no storage
     * permission and goes with the app: set once, where Settings names no folder.
     */
    private fun keepDownloadsInAppSpace() {
        val folder = (getExternalFilesDir(Environment.DIRECTORY_MOVIES) ?: filesDir.resolve("downloads")).apply { mkdirs() }
        scope.launch {
            val view = store.settings.view.first { it != null }
            val row = store.settings.row(SettingKey.DOWNLOAD_DIR, view)
            if (SettingsCatalog.typedText(row).isNullOrBlank()) {
                store.setSetting(SettingKey.DOWNLOAD_DIR, SettingValue.Str(folder.path))
            }
        }
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

        /** The notification's art, on its long side. */
        private const val NOTIFICATION_ART = 512
    }
}
