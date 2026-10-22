// What the engine is told of the app's place: E7's AppPhase.

package tv.anistream.store

import tv.anistream.engine.model.AppState
import tv.anistream.engine.model.Command

/**
 * Tells the engine where the app is: the foreground, or the background with whether an episode
 * plays there. In the background it says so again each time that changes, so the engine stops
 * keeping a torrent going for an episode that has paused or ended, and keeps one going again for
 * an episode played on from the notification.
 */
class AppPhases(private val send: (Command) -> Unit) {
    private var background = false
    private var playing = false

    /** The app came to the foreground. */
    fun foreground() {
        background = false
        send(Command.AppPhase(AppState.Foreground))
    }

    /** The app went to the background, with an episode [playing] there or not. */
    fun background(playing: Boolean) {
        background = true
        this.playing = playing
        send(Command.AppPhase(AppState.Background(playing)))
    }

    /** Whether an episode plays, each time that changes: said again while in the background. */
    fun playing(now: Boolean) {
        if (!background || now == playing) return
        playing = now
        send(Command.AppPhase(AppState.Background(now)))
    }
}
