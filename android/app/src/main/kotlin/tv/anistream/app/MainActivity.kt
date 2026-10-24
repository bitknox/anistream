// The one activity: edge to edge, portrait on phones, the navigation bar and its destinations.

package tv.anistream.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.app.PictureInPictureUiState
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import tv.anistream.app.tv.TvRemoteService
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.SettingKey
import tv.anistream.player.ui.PlayerFrame
import tv.anistream.store.DownloadsModel
import tv.anistream.store.InviteLinks
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.TvRemoteModel
import tv.anistream.store.traced
import tv.anistream.ui.AppActions
import tv.anistream.ui.AppLink
import tv.anistream.ui.AppRoot
import tv.anistream.ui.NotificationActions
import tv.anistream.ui.Destination

class MainActivity : ComponentActivity() {
    private val app get() = application as AnistreamApp

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {
        app.notifier.asked()
    }

    /**
     * The permission asked for as new-episode notifications are turned on: they go on with it,
     * and without it stay off, the switch's line saying why.
     */
    private val askForEpisodes = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.notifier.asked()
        if (granted) app.episodeAlerts.set(true) else app.episodeAlerts.refuse()
    }

    /**
     * Android 17's local network permission, asked the first time Play on TV is used: the TVs are
     * found and reached on the network this phone is on. The browse starts again once it is given.
     */
    private val askLocalNetwork = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) {
            app.store.tv.browseAgain()
            app.store.tv.retryNow()
        }
    }

    private fun askLocalNetworkOnce() {
        if (Build.VERSION.SDK_INT < LOCAL_NETWORK_SDK) return
        if (checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED) return
        askLocalNetwork.launch(Manifest.permission.ACCESS_LOCAL_NETWORK)
    }

    /** What waits on the permission a show's own switch asked for: switching it on. */
    private var permitted: (() -> Unit)? = null

    /** The permission asked for as a show is switched on for itself: on with it, left as it was without. */
    private val askForShow = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.notifier.asked()
        if (granted) permitted?.invoke()
        permitted = null
    }

    /**
     * Do [then] once new-episode notifications can show, as a show's own switch turns on: at once
     * where they can, after the permission where it was never given, and where Android says no,
     * not at all, with the system's settings open instead.
     */
    private fun permitNotifications(then: () -> Unit) {
        val granted = ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        when (EpisodeAlerts.Turn.of(true, app.episodeNotifier.canPost, granted)) {
            EpisodeAlerts.Turn.On -> then()
            EpisodeAlerts.Turn.Ask -> {
                permitted = then
                askForShow.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
            EpisodeAlerts.Turn.SystemSettings -> openNotificationSettings()
            EpisodeAlerts.Turn.Off -> {}
        }
    }

    /** New-episode notifications, on Settings' switch: on only once they can show. */
    private fun setEpisodeNotifications(on: Boolean) {
        val permitted = ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        when (EpisodeAlerts.Turn.of(on, app.episodeNotifier.canPost, permitted)) {
            EpisodeAlerts.Turn.Off -> app.episodeAlerts.set(false)
            EpisodeAlerts.Turn.On -> app.episodeAlerts.set(true)
            EpisodeAlerts.Turn.Ask -> askForEpisodes.launch(Manifest.permission.POST_NOTIFICATIONS)
            EpisodeAlerts.Turn.SystemSettings -> {
                app.episodeAlerts.refuse()
                openNotificationSettings()
            }
        }
    }

    /**
     * The system's settings for the New episodes channel: its sound, vibration and the lock screen,
     * and the app's notifications as a whole where those are off.
     */
    private fun openNotificationSettings() {
        // The notifier makes the channel, which the page needs to be there.
        app.episodeNotifier
        startActivity(
            Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS)
                .putExtra(Settings.EXTRA_APP_PACKAGE, packageName)
                .putExtra(Settings.EXTRA_CHANNEL_ID, EpisodeNotifier.CHANNEL),
        )
    }

    /** AniList's and MyAnimeList's sign-in in a Custom Tab, and the redirect that ends it. */
    private val signIn by lazy { BrowserSignIn(this, app.store) }

    /**
     * The app links the activity is opened on, launcher shortcuts and notifications, kept until
     * the frame is there to follow them.
     */
    private val links = Channel<AppLink>(Channel.BUFFERED)
    private val linkFlow = links.receiveAsFlow()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Light icons on the dark ground, over bars the app draws itself.
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        // The navigation bar's own colour runs under the gesture bar, with no scrim of the system's.
        window.isNavigationBarContrastEnforced = false
        holdPortraitOnPhones(resources.configuration)

        traced("launch") { app.start(Flavour.launch(this, intent)) }
        app.store.tv.firstUse = ::askLocalNetworkOnce
        // Where Android stopped the app as "Install unknown apps" turned on, the press goes on.
        app.appUpdates?.kept()?.let(app.store.updates::returning)
        Flavour.started(this, app.store, intent, app.episodeNotifier)
        val steerable = debuggable || Flavour.FIXTURES
        val start = intent.getStringExtra(SECTION)?.takeIf { steerable }
            ?.let(Destination::named) ?: Destination.Continue
        val title = intent.getStringExtra(TITLE)?.takeIf { steerable }?.toUIntOrNull()
        val query = intent.getStringExtra(QUERY)?.takeIf { steerable }
        val page = intent.getStringExtra(PAGE)?.takeIf { steerable }

        // Play from the title page, the Continue hero or an episode row: the engine finds the
        // stream and opens the player, which the frame then shows.
        val actions = AppActions(
            play = { id, episode -> app.store.send(Command.Play(id, episode)) },
            notifications = NotificationActions(
                turn = ::setEpisodeNotifications,
                refused = app.episodeAlerts.refused,
                permit = ::permitNotifications,
                openSystemSettings = ::openNotificationSettings,
            ),
        )
        // Play on TV from the player: the episode and where it is, which the TV starts from.
        val player = PlayerFrame(app.player) {
            val tv = app.store.tv
            val model = app.player
            val id = model.titleId
            val episode = model.startingEpisode
            if (!tv.available || id == null || episode == null) {
                null
            } else {
                { tv.offer(TvPlayRequest(id, model.showTitle, episode, model.position, fromPlayer = true)) }
            }
        }
        traced("setContent") { setContent {
            AnistreamTheme {
                AppRoot(
                    app.store,
                    if (query != null) Destination.Search else start,
                    actions = actions,
                    startTitle = title,
                    startQuery = query,
                    startPage = page,
                    player = player,
                    links = linkFlow,
                    onSection = { section -> app.appUpdates?.section = section },
                )
            }
        } }
        if (Flavour.FIXTURES || debuggable) announceReady()
        askForNotificationsWhenNeeded()
        signIn.start()
        // A restored activity, or one opened again from Recents, has its navigation already.
        val fresh = savedInstanceState == null && intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY == 0
        if (!signIn.handle(intent) && !(fresh && follow(intent)) && !invite(intent)) remote(intent)
        keepDownloadsRunning()
        startUpdateActivities()
    }

    /**
     * What the update asks to be started from an activity the viewer is looking at: Android's
     * "Install unknown apps" page for the app, and the installer's confirmation.
     */
    private fun startUpdateActivities() {
        val updates = app.appUpdates ?: return
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                updates.launch.collect { intent ->
                    intent ?: return@collect
                    val started = runCatching { startActivity(intent) }
                        .onFailure { Log.w(AnistreamApp.TAG, "update: ${intent.action} did not open: ${it.message}") }
                        .isSuccess
                    updates.launched(intent, started)
                }
            }
        }
    }

    /**
     * While the app is in front, ask for the download job whenever an episode waits or runs: a
     * user-initiated job is asked for from a screen the person is using.
     */
    private fun keepDownloadsRunning() {
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                app.store.downloads.rows.collect { rows ->
                    val wifiOnly = !app.store.settings.isOn(SettingKey.DOWNLOADS_ON_CELLULAR)
                    if (DownloadsModel.busy(rows)) app.notifier.wantPermission()
                    DownloadJob.ensure(this@MainActivity, rows, wifiOnly)
                }
            }
        }
    }

    /**
     * An app link the activity was opened on goes to the frame to follow: a shortcut's, which the
     * launcher is told was used, or a notification's.
     */
    private fun follow(intent: Intent?): Boolean {
        val link = intent?.appLink() ?: return false
        // Taken off the intent, so a recreated activity does not follow it again.
        intent.action = null
        links.trySend(link)
        // A notification's action leaves its notification up; following it takes it away.
        intent?.getStringExtra(EpisodeNotifier.EXTRA_NOTIFICATION)?.let(app.episodeNotifier::dismiss)
        ContinueShortcut.shortcutOf(link)?.let { ShortcutManagerCompat.reportShortcutUsed(this, it) }
        return true
    }

    /** An invite link the app was opened with opens the join sheet with it, checked as it opens. */
    private fun invite(intent: Intent?): Boolean {
        val link = intent?.data?.toString()?.let(InviteLinks::invite) ?: return false
        app.store.stores.openJoin(link)
        // Taken off the intent, so a recreated activity does not open the sheet again.
        intent.data = null
        return true
    }

    /**
     * A TV's pairing link, from its QR code read by the system's camera, pairs with it; the TV
     * notification's tap opens its card.
     */
    private fun remote(intent: Intent?): Boolean {
        intent ?: return false
        if (intent.getBooleanExtra(TvRemoteService.EXTRA_REMOTE, false)) {
            intent.removeExtra(TvRemoteService.EXTRA_REMOTE)
            app.store.tv.openRemote()
            return true
        }
        val link = intent.data?.toString()?.takeIf { it.startsWith("${TvRemoteModel.SCHEME}://") } ?: return false
        intent.data = null
        return app.store.tv.open(link)
    }

    /**
     * The volume keys move the TV's volume while its card is open, or while it is driven and
     * plays; the TV's media session takes them once the app is behind another.
     */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val step = when (event.keyCode) {
            KeyEvent.KEYCODE_VOLUME_UP -> TvRemoteModel.VOLUME_STEP
            KeyEvent.KEYCODE_VOLUME_DOWN -> -TvRemoteModel.VOLUME_STEP
            else -> return super.dispatchKeyEvent(event)
        }
        val tv = app.store.tv
        val driving = tv.connected.value != null &&
            (tv.sheet.value == TvRemoteModel.Sheet.Remote || tv.card.value?.card?.now != null)
        if (!driving || app.player.isPresented) return super.dispatchKeyEvent(event)
        if (event.action == KeyEvent.ACTION_DOWN) tv.send(FfiTvCommand.Volume(step))
        return true
    }

    override fun onResume() {
        super.onResume()
        signIn.resumed()
        // Back from Android's "Install unknown apps" page: the update goes on, if it may.
        app.store.updates.resumed()
    }

    /**
     * Into Picture in Picture, or out of it: the window shows the video alone while in. Closed
     * from its window, the activity is already stopped when this says so, and the player closes,
     * keeping the place; expanded, the controls come up and hide after 3 s.
     */
    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        val player = app.player
        player.inPip = isInPictureInPictureMode
        if (isInPictureInPictureMode) return
        if (lifecycle.currentState == Lifecycle.State.CREATED) player.viewerClosed() else player.poke()
    }

    /** On the way into the window the overlays go at once, before the system moves the video. */
    override fun onPictureInPictureUiStateChanged(pipState: PictureInPictureUiState) {
        super.onPictureInPictureUiStateChanged(pipState)
        if (pipState.isTransitioningToPip) app.player.inPip = true
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        // The player asks for landscape on a phone while it is up, and for portrait as it closes.
        if (!app.player.isPresented) holdPortraitOnPhones(newConfig)
    }

    /**
     * On a phone, under 600 dp at its smallest, the browse screens are portrait only; tablets and
     * foldables turn freely, and a folded foldable's outer screen follows the phone rule.
     */
    private fun holdPortraitOnPhones(configuration: Configuration) {
        requestedOrientation = if (configuration.smallestScreenWidthDp < 600) {
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        } else {
            ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    private val debuggable: Boolean get() = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    /** Say `ANISTREAM_READY` in the log once the screens have what they asked for: what shots.sh waits on. */
    private fun announceReady() {
        lifecycleScope.launch {
            // Up to three minutes: an emulator on a machine busy with builds starts the engine slowly.
            if (app.store.settle(timeoutMillis = 180_000) && Flavour.reachedTv(intent, app.store)) {
                Log.i(AnistreamApp.TAG, READY)
            } else {
                Log.w(AnistreamApp.TAG, "the screens did not settle")
            }
        }
    }

    /** Ask for the notification permission the first time a notification needs it, and only then. */
    private fun askForNotificationsWhenNeeded() {
        val preferences = getSharedPreferences("anistream", Context.MODE_PRIVATE)
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                app.notifier.wantsPermission.collect { wants ->
                    if (wants && !preferences.getBoolean(NOTIFICATIONS_ASKED, false)) {
                        preferences.edit().putBoolean(NOTIFICATIONS_ASKED, true).apply()
                        askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // The redirect a sign-in comes back with is the engine's, and is not kept as the
        // activity's intent.
        if (signIn.handle(intent) || follow(intent) || invite(intent) || remote(intent)) return
        setIntent(intent)
    }

    companion object {
        /** The marker shots.sh waits for in the log. */
        const val READY = "ANISTREAM_READY"

        /** The destination to open on: `continue`, `calendar`, `seasonal`, `library` or `search`. */
        const val SECTION = "section"

        /** A title page to open there, by its AniList id. */
        const val TITLE = "title"

        /** A search to run, on Search. */
        const val QUERY = "query"

        /** A page to open over the destination, by its route: `settings`, `settings/playback`. */
        const val PAGE = "page"

        private const val NOTIFICATIONS_ASKED = "notificationsAsked"

        /** Android 17, where reaching the local network asks first. */
        private const val LOCAL_NETWORK_SDK = 37
    }
}
