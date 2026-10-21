package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.store.AppStore

/** Playback's Episodes group: the new-episode notifications row, there only when the app has a switch for it. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class EpisodeNotificationsRowTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /** The row's switch says off, as SwitchRow tells TalkBack. */
    private val off = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Off")

    private fun pane(actions: AppActions) {
        val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Column(Modifier.verticalScroll(rememberScrollState())) { PlaybackPane(actions) }
                }
            }
        }
    }

    @Test
    fun turning_the_row_on_asks_the_app_which_decides() {
        val turns = mutableListOf<Boolean>()
        pane(AppActions(episodeNotifications = AppSwitch(MutableStateFlow(false)) { turns += it }))
        val row = compose.onNodeWithText("New episode notifications")
        row.performScrollTo()
        row.assert(off)
        row.performClick()
        // The app turns it on once it may post; the row only asks.
        assertEquals(listOf(true), turns)
        row.assert(off)
    }

    @Test
    fun without_a_switch_the_row_is_left_out() {
        pane(AppActions())
        assertTrue(compose.onAllNodesWithText("New episode notifications").fetchSemanticsNodes().isEmpty())
    }
}
