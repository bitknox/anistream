// The TV app's one activity: the root composition, the engine started on the first launch, and
// playback paused when the app leaves the screen.

package tv.anistream.tv

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch
import tv.anistream.engine.model.Command
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.shell.TvRoot
import tv.anistream.tv.ui.TvTheme

class TvActivity : ComponentActivity() {
    private val app: TvApp get() = application as TvApp

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        app.start(Flavour.launch(this, intent))
        val start = Flavour.start(intent)
        setContent {
            val nav = remember {
                TvNavigator(start.destination).apply {
                    start.query?.let { searchText = it }
                    start.title?.let(::openTitle)
                }
            }
            TvTheme {
                TvRoot(
                    store = app.store,
                    player = app.player,
                    nav = nav,
                    play = { id, episode -> app.store.send(Command.Play(id, episode.toString())) },
                )
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

    override fun onStop() {
        super.onStop()
        // Leaving the app (Home) pauses; the engine keeps the position. No Picture in Picture on TV.
        if (app.player.isPresented) app.player.setPlaying(false)
    }
}
