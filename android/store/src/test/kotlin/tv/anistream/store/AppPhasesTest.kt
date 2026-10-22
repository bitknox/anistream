package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.model.AppState
import tv.anistream.engine.model.Command

class AppPhasesTest {
    private val said = mutableListOf<Command>()
    private val phases = AppPhases { said += it }

    private fun background(playing: Boolean) = Command.AppPhase(AppState.Background(playing))

    @Test
    fun playback_that_stops_in_the_background_is_said_so_the_engine_lets_its_torrent_go() {
        phases.background(playing = true)
        // Paused from the notification, or the episode ended.
        phases.playing(false)
        assertEquals(listOf(background(true), background(false)), said)
    }

    @Test
    fun playback_that_starts_again_in_the_background_is_said_too() {
        phases.background(playing = true)
        phases.playing(false)
        phases.playing(true)
        assertEquals(listOf(background(true), background(false), background(true)), said)
    }

    @Test
    fun nothing_changed_and_the_foreground_say_nothing_more() {
        phases.background(playing = false)
        phases.playing(false)
        phases.foreground()
        // In the foreground the engine is not told of playback; going back is said as itself.
        phases.playing(true)
        phases.playing(false)
        phases.background(playing = false)
        assertEquals(listOf(background(false), Command.AppPhase(AppState.Foreground), background(false)), said)
    }
}
