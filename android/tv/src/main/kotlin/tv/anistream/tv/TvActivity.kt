// The TV app's one activity: the root composition, the engine started on the first launch, and
// playback paused when the app leaves the screen.

package tv.anistream.tv

import android.Manifest
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.SystemClock
import android.text.format.DateFormat
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.snapshotFlow
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.FrontIntent
import tv.anistream.engine.model.RemoteKey
import tv.anistream.player.takeBackBeforeShown
import tv.anistream.store.AirTime
import tv.anistream.store.AppStore
import tv.anistream.tv.home.parsePlayLink
import tv.anistream.tv.shell.Destination
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.shell.TvRoot
import tv.anistream.tv.ui.TvTheme

class TvActivity : ComponentActivity() {
    // Below Android 13 the chosen language is the app's own to apply.
    override fun attachBaseContext(newBase: android.content.Context) = super.attachBaseContext(tv.anistream.store.AppLanguage.wrap(newBase))

    private val app: TvApp get() = application as TvApp

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        app.player.hostStarted()
        // Back a moment after Play, before the player's screen is there to take it.
        takeBackBeforeShown(app.player)
        AirTime.twentyFourHour = DateFormat.is24HourFormat(this)
        app.start(Flavour.launch(this, intent))
        app.attachUpdater(intent)
        val start = Flavour.start(intent)
        Flavour.started(app.store, intent)
        val nav = TvNavigator(start.destination).apply {
            start.query?.let { typeSearch(it) }
            start.title?.let(::openTitle)
            if (start.update) openUpdate()
            start.signIn?.let(::openSignIn)
        }
        setContent {
            TvTheme {
                TvRoot(
                    store = app.store,
                    player = app.player,
                    nav = nav,
                    play = { id, episode -> app.store.send(Command.Play(id, episode.toString())) },
                    playFromStart = { id, episode -> app.store.send(Command.Play(id, episode.toString(), fromStart = true)) },
                    remote = app.remote,
                    watchNext = app.watchNext,
                    firstLaunch = start.firstLaunch,
                )
            }
        }
        this.nav = nav
        if (savedInstanceState == null) openLink(intent)
        app.remote.askLocalNetwork = ::askLocalNetwork
        if (app.remote.findable) askLocalNetwork()
        // What a paired phone asks of the browse UI.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                app.remote.intents.collect { act(nav, it) }
            }
        }
        // Android's pages the update opens: the switch for installing, and the installer's own.
        app.updater?.let { updater ->
            lifecycleScope.launch {
                repeatOnLifecycle(Lifecycle.State.RESUMED) {
                    updater.launch.collect { intent ->
                        if (intent == null) return@collect
                        val started = try {
                            startActivity(intent)
                            true
                        } catch (_: ActivityNotFoundException) {
                            false
                        } catch (_: SecurityException) {
                            false
                        }
                        updater.launched(intent, started)
                    }
                }
            }
        }
        // The screen stays on while an episode plays, and the screensaver may come once it stops.
        lifecycleScope.launch {
            snapshotFlow { app.player.isPresented && app.player.hasPicture && !app.player.paused }.collect { playing ->
                if (playing) {
                    window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                } else {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }
            }
        }
    }

    private var nav: TvNavigator? = null

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        openLink(intent)
    }

    /**
     * A Watch Next entry: the player on its episode, with the title page under it so Back from
     * the player lands there.
     */
    private fun openLink(intent: Intent) {
        val (id, episode) = parsePlayLink(intent.dataString) ?: return
        val nav = nav ?: return
        nav.openTitle(id)
        lifecycleScope.launch {
            app.store.phase.first { it == AppStore.Phase.Running }
            app.store.send(Command.Play(id, episode.toString()))
        }
    }

    /** Do what a phone asked: open a title, type into Search, or press a key on the remote. */
    private fun act(nav: TvNavigator, intent: FrontIntent) {
        val playing = app.player.isPresented
        when (intent) {
            is FrontIntent.Open -> if (!playing) nav.openTitle(intent.id)
            is FrontIntent.Text -> if (app.store.match.tvSearching) {
                // The match search is up: the phone types into it, not into Search.
                app.store.match.phoneTyped(intent.text, intent.submit)
            } else if (!playing) {
                if (nav.destination != Destination.Search || nav.pages.isNotEmpty()) nav.go(Destination.Search)
                nav.phoneSearch(intent.text)
                if (intent.submit) {
                    nav.phoneDone()
                    nav.submitSearch()
                }
            }
            is FrontIntent.Navigate -> when (intent.key) {
                RemoteKey.BACK -> onBackPressedDispatcher.onBackPressed()
                RemoteKey.HOME -> if (!playing) nav.go(Destination.Home)
                else -> press(keyCode(intent.key))
            }
            else -> Unit
        }
    }

    private fun keyCode(key: RemoteKey): Int = when (key) {
        RemoteKey.UP -> KeyEvent.KEYCODE_DPAD_UP
        RemoteKey.DOWN -> KeyEvent.KEYCODE_DPAD_DOWN
        RemoteKey.LEFT -> KeyEvent.KEYCODE_DPAD_LEFT
        RemoteKey.RIGHT -> KeyEvent.KEYCODE_DPAD_RIGHT
        else -> KeyEvent.KEYCODE_DPAD_CENTER
    }

    /** A key as the TV's own remote sends it: down, then up, through the window to this activity. */
    private fun press(code: Int) {
        val now = SystemClock.uptimeMillis()
        val decor = window.decorView
        decor.dispatchKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, code, 0))
        decor.dispatchKeyEvent(KeyEvent(now, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP, code, 0))
    }

    /** Android 17 asks before an app reaches the local network, which the remote needs. */
    private fun askLocalNetwork() {
        if (Build.VERSION.SDK_INT < LOCAL_NETWORK_SDK) return
        if (checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED) return
        requestPermissions(arrayOf(Manifest.permission.ACCESS_LOCAL_NETWORK), LOCAL_NETWORK_REQUEST)
    }

    override fun onDestroy() {
        // Back a moment after Play: the engine has not opened the player yet, and must not.
        if (isFinishing) app.player.hostFinished()
        super.onDestroy()
    }

    override fun onStart() {
        super.onStart()
        // Back to the app after a Back put it behind: a play from here has somewhere to show.
        app.player.hostStarted()
    }

    override fun onResume() {
        super.onResume()
        // The system's 12 or 24 hour clock, which can change while the app is away.
        AirTime.twentyFourHour = DateFormat.is24HourFormat(this)
        // Back from Android's page for the switch, the update goes on where it was.
        app.updater?.resumed()
    }

    override fun onStop() {
        super.onStop()
        // Leaving the app (Home) pauses; the engine keeps the position. No Picture in Picture on TV.
        if (app.player.isPresented) app.player.setPlaying(false)
    }
}

/** The first Android that asks before an app reaches the local network: 17. */
private const val LOCAL_NETWORK_SDK = 37
private const val LOCAL_NETWORK_REQUEST = 17
