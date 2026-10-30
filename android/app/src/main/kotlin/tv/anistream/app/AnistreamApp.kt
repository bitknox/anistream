// The process's one engine and store, which outlive every activity.

package tv.anistream.app

import tv.anistream.i18n.Strings
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
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.plus
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.SettingsCatalog
import tv.anistream.engine.ffi.startLog
import tv.anistream.engine.core.AnilistId
import tv.anistream.player.CaptionStyle
import tv.anistream.player.CaptionStyleWatch
import tv.anistream.player.Device
import tv.anistream.player.MpvBackend
import tv.anistream.player.PlayerConnection
import tv.anistream.player.PlayerHost
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppHost
import tv.anistream.store.BackgroundEngine
import tv.anistream.store.EngineEpisodeCheck
import tv.anistream.store.EpisodeCheck
import tv.anistream.app.tv.TvConnection
import tv.anistream.store.AppStore
import tv.anistream.store.EngineLaunch
import tv.anistream.store.KeystoreKey
import tv.anistream.store.SealedFileSecrets
import tv.anistream.store.traced
import tv.anistream.engine.model.WidgetData
import tv.anistream.ui.AppLink
import tv.anistream.widgets.WidgetArt
import tv.anistream.widgets.WidgetHost
import tv.anistream.widgets.Widgets

class AnistreamApp : Application(), PlayerHost, NewEpisodesHost, WidgetHost {
    /** Where the store's work runs: the main thread, for as long as the process. */
    private val scope = MainScope() + kotlinx.coroutines.CoroutineExceptionHandler { _, e ->
        // A coroutine that fails in the store is logged; it does not take the process with it.
        android.util.Log.e("anistream", "the store's work failed", e)
    }

    val store: AppStore by lazy {
        AppStore(scope).also { store ->
            if (Flavour.UPDATES) {
                val updates = AppUpdates(this, store.updates)
                store.updates.attach(updates, updates.files)
                appUpdates = updates
            }
        }
    }

    /** The app's own update on Android, in the app flavour only; the store makes it. */
    var appUpdates: AppUpdates? = null
        get() {
            store
            return field
        }
        private set

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
            systemCaptionStyle = { CaptionStyle.system(this) },
        ).also { player ->
            store.onPlayback = player::apply
            PlayerConnection(this, player, scope)
            SystemPhases(store, player, scope).start()
            captions = CaptionStyleWatch(this, player)
        }
    }

    /** The player's caption style, following the setting and the system's style. */
    private var captions: CaptionStyleWatch? = null

    override val notificationIcon: Int get() = R.drawable.ic_launcher_monochrome

    /** The art the screens have decoded, for the notification; asked for when it has not come yet. */
    override fun artwork(url: String): Bitmap? {
        store.artwork.image(url)?.let { return it.image }
        store.artwork.request(url, NOTIFICATION_ART)
        return null
    }

    val notifier: Notifier by lazy { Notifier(this) }

    override val episodeNotifier: EpisodeNotifier by lazy { EpisodeNotifier(this) }

    /** What the home-screen widgets show, as the engine last made it. */
    val widgets: WidgetRecord by lazy { WidgetRecord.of(filesDir) }

    /** The widgets' record and art, kept as each check or visit makes them. */
    private val widgetKeeper by lazy { WidgetKeeper(this, widgets, WidgetArt.of(filesDir)) }

    /** The check for new episodes, on the engine, with the widgets' record and its art made after it. */
    override val episodeCheck: EpisodeCheck by lazy {
        EngineEpisodeCheck(background, widgets = { store, data -> widgetKeeper.keep(store, data) })
    }

    override val widgetRecord: StateFlow<WidgetData?> get() = widgets.latest

    override val widgetArt: WidgetArt get() = widgetKeeper.art

    /** A widget's tap: the activity on [link], explicitly, or as the launcher opens it. */
    override fun widgetIntent(link: AppLink?): Intent =
        (link?.let(::linkIntent) ?: Intent(this, MainActivity::class.java).setAction(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    override fun refreshWidgetRecord() = WidgetRecordWork.ask(this)

    /** Make the widgets' record now, on the app's engine while it runs or a quiet one of its own. */
    suspend fun makeWidgetRecord() {
        background.use { widgetKeeper.make(it) }
    }

    override suspend fun recordWidgets() {
        if (Widgets.placed(this)) makeWidgetRecord()
    }

    /** A widget placed or the last of one removed: the background check follows whether any is. */
    override fun widgetsChanged() {
        scope.launch { episodeAlerts.widgetsPlaced(Widgets.placed(this@AnistreamApp)) }
    }

    override fun onCreate() {
        super.onCreate()
        // Every process draws the widgets again when a record lands: the app's, and one started
        // for the background check alone.
        widgetKeeper.follow(scope)
        scope.launch { Widgets.publishPreviews(this@AnistreamApp) }
        // Whether a widget is placed, which keeps the background check, as the app starts.
        widgetsChanged()
        // An update's APK and installer session a stopped process left go at every start.
        if (Flavour.UPDATES) appUpdates?.let { updates -> scope.launch(Dispatchers.IO) { updates.clearLeftovers() } }
    }

    /** The new-episode notifications switch, and the schedule it keeps. */
    val episodeAlerts: EpisodeAlerts by lazy { EpisodeAlerts(this, store, episodeNotifier) }

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
        episodeAlerts.follow(scope)
        ContinueShortcut(this).follow(store, scope)
        recordWidgetsOnLeaving()
        watchNetwork()
        startTv()
        // The fixtures' downloads are seeded where they are; only the app's are kept here.
        if (!Flavour.FIXTURES) keepDownloadsInAppSpace()
    }

    /** What keeps the TV this phone drives through the background, once Play on TV is up. */
    private var tvConnection: TvConnection? = null

    /**
     * Play on TV, once the engine runs: the TVs Android's DNS-SD finds (the fixtures' made-up
     * ones in that flavour), and the service that keeps a TV driven in the background. A TV that
     * plays what the player here played closes the player.
     */
    private fun startTv() {
        scope.launch {
            store.phase.first { it != AppStore.Phase.Starting }
            if (store.phase.value != AppStore.Phase.Running) return@launch
            val browser = Flavour.tvBrowser(this@AnistreamApp, store) ?: return@launch
            val tablet = resources.configuration.smallestScreenWidthDp >= 600
            val version = runCatching { packageManager.getPackageInfo(packageName, 0).versionName }.getOrNull() ?: "0"
            if (!store.startTv(browser, if (tablet) Strings.deviceNameTablet(this@AnistreamApp) else Strings.deviceNamePhone(this@AnistreamApp), "anistream-android/$version")) return@launch
            store.tv.handedOff = { player.viewerClosed() }
            tvConnection = TvConnection(this@AnistreamApp, store.tv, scope)
        }
    }

    /**
     * Make the widgets' record again each time the app leaves the screen, while a widget is
     * placed: what was watched, marked or synced on this visit is on the home screen as the person
     * gets there.
     */
    private fun recordWidgetsOnLeaving() {
        ProcessLifecycleOwner.get().lifecycle.addObserver(
            LifecycleEventObserver { _, event ->
                if (event != Lifecycle.Event.ON_STOP) return@LifecycleEventObserver
                scope.launch {
                    if (!Widgets.placed(this@AnistreamApp)) return@launch
                    runCatching { makeWidgetRecord() }.onFailure { Log.w(TAG, "the widgets' record was not made: ${it.message}") }
                }
            },
        )
    }

    /**
     * Tell the engine the network the device is on, as it changes: cellular, Data Saver as
     * constrained, and no network that reaches the internet as offline. Cellular and constrained
     * hold downloads and seeding as the settings say; offline the engine turns into its one
     * offline state, which every screen shows.
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
