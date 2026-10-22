// The player's screen on the JVM, as the frame shows it: up while the model is, and gone after.

package tv.anistream.player

import android.content.pm.ActivityInfo
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.model.Ending
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.PlayerOp
import tv.anistream.player.ui.PlayerScreen
import tv.anistream.store.AppStore
import tv.anistream.testkit.FixtureArt
import tv.anistream.ui.LocalAppStore

/** A phone held upright, as the browse screens are. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PlayerScreenTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @After
    fun tearDown() = scope.cancel()

    private var mpv: ScriptedMpv? = null
    private val model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { 0L }, hwdec = "no")

    /** The screen while the model is up, as the frame's player route has it. */
    private fun shown() {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    if (model.isPresented) PlayerScreen(model, video = { _, modifier, _ -> Box(modifier) })
                }
            }
        }
        compose.waitForIdle()
    }

    private val orientation: Int get() = compose.activity.requestedOrientation

    @Test
    fun aPlayThatNeverPlayedSaysSoWithCloseWhichClosesIt() {
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlaybackEnded(false, Ending.Failed, false))
        }
        shown()
        compose.onNodeWithText("Couldn't play episode 7").assertIsDisplayed()
        compose.onNodeWithText("Choose source").assertIsDisplayed()
        compose.onNodeWithText("Close").performClick()
        compose.waitForIdle()
        assertFalse(model.isPresented)
    }

    @Test
    fun aPlayerTheEngineClosesLeavesThePhoneUprightAgain() {
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
        }
        shown()
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE, orientation)
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        }
        compose.waitForIdle()
        // The episode played to its end, and the engine closed the player: nobody pressed close.
        compose.runOnUiThread { model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Close))) }
        compose.waitForIdle()
        assertFalse(model.isPresented)
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_PORTRAIT, orientation)
    }
}
