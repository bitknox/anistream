// The one activity: edge to edge, portrait on phones, the navigation bar and its destinations.

package tv.anistream.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.res.Configuration
import android.graphics.Color
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.launch
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.DownloadsModel
import tv.anistream.store.InviteLinks
import tv.anistream.ui.AppRoot
import tv.anistream.ui.Destination

class MainActivity : ComponentActivity() {
    private val app get() = application as AnistreamApp

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {
        app.notifier.asked()
    }

    /** AniList's and MyAnimeList's sign-in in a Custom Tab, and the redirect that ends it. */
    private val signIn by lazy { BrowserSignIn(this, app.store) }

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

        app.start(Flavour.launch(this, intent))
        Flavour.started(app.store, intent)
        val steerable = debuggable || Flavour.FIXTURES
        val start = intent.getStringExtra(SECTION)?.takeIf { steerable }
            ?.let(Destination::named) ?: Destination.Continue
        val title = intent.getStringExtra(TITLE)?.takeIf { steerable }?.toUIntOrNull()
        val query = intent.getStringExtra(QUERY)?.takeIf { steerable }
        val page = intent.getStringExtra(PAGE)?.takeIf { steerable }

        setContent {
            AnistreamTheme {
                AppRoot(
                    app.store,
                    if (query != null) Destination.Search else start,
                    startTitle = title,
                    startQuery = query,
                    startPage = page,
                )
            }
        }
        if (Flavour.FIXTURES || debuggable) announceReady()
        askForNotificationsWhenNeeded()
        signIn.start()
        if (!signIn.handle(intent)) invite(intent)
        keepDownloadsRunning()
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

    /** An invite link the app was opened with opens the join sheet with it, checked as it opens. */
    private fun invite(intent: Intent?): Boolean {
        val link = intent?.data?.toString()?.let(InviteLinks::invite) ?: return false
        app.store.stores.openJoin(link)
        // Taken off the intent, so a recreated activity does not open the sheet again.
        intent.data = null
        return true
    }

    override fun onResume() {
        super.onResume()
        signIn.resumed()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        holdPortraitOnPhones(newConfig)
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
            if (app.store.settle(timeoutMillis = 180_000)) {
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
        if (signIn.handle(intent) || invite(intent)) return
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
    }
}
