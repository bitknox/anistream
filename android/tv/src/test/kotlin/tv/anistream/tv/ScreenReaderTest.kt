package tv.anistream.tv

import android.view.accessibility.AccessibilityManager
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.activity.ComponentActivity
import android.content.ComponentName
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.robolectric.annotation.GraphicsMode
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import org.robolectric.RuntimeEnvironment
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import tv.anistream.tv.player.seekSemantics
import tv.anistream.tv.player.spokenTime
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.rememberScreenReader
import tv.anistream.tv.ui.spelled

/** What a screen reader gets on the TV: the seek bar's range and actions, rows' states, tabs, codes. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class ScreenReaderTest {
    val rule = createAndroidComposeRule<ComponentActivity>()

    /** The app's manifest has no plain activity for the rule to start; it is declared first, as in TvShots. */
    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(rule)

    private fun reader(on: Boolean) {
        val manager = RuntimeEnvironment.getApplication().getSystemService(AccessibilityManager::class.java)
        shadowOf(manager).setTouchExplorationEnabled(on)
    }

    @Test
    fun the_seek_bar_has_a_range_in_ten_second_steps_and_seeks_on_set_progress() {
        val sought = mutableListOf<Double>()
        val moved = mutableListOf<Double>()
        rule.setContent {
            Box(Modifier.size(100.dp).testTag("bar").seekSemantics(724.0, 1420.0, null, { sought += it }, { moved += it }))
        }
        val node = rule.onNodeWithTag("bar")
        node.assert(SemanticsMatcher("range in 10 s steps") {
            val info = it.config[SemanticsProperties.ProgressBarRangeInfo]
            info.current == 724f && info.range == 0f..1420f && info.steps == 141
        })
        node.assert(SemanticsMatcher("state in words") {
            val state = it.config[SemanticsProperties.StateDescription]
            state.contains("12 minutes") && state.contains("23 minutes")
        })
        node.performSemanticsAction(SemanticsActions.SetProgress) { it(300f) }
        assertEquals(listOf(300.0), sought)
        val actions = rule.onNodeWithTag("bar").fetchSemanticsNode().config[SemanticsActions.CustomActions]
        actions[0].action()
        actions[1].action()
        assertEquals(listOf(-10.0, 10.0), moved)
    }

    @Test
    fun a_time_is_spoken_in_words() {
        assertEquals("12 minutes, 4 seconds", spokenTime(724.0))
        assertEquals("1 hour, 2 minutes", spokenTime(3720.0))
        assertEquals("0 seconds", spokenTime(0.0))
    }

    @Test
    fun a_code_is_spelled_one_character_at_a_time() {
        assertEquals("0, 4, 2, 9, 1, 7", spelled("042 917"))
    }

    @Test
    fun the_screen_reader_state_follows_the_system() {
        reader(true)
        var on = false
        rule.setContent { on = rememberScreenReader().value }
        rule.waitForIdle()
        assertTrue(on)
        reader(false)
        rule.waitForIdle()
        assertFalse(on)
    }

    @Test
    fun a_switch_row_says_it_is_a_switch_and_its_state() {
        rule.setContent { TvListRow("Autoplay", onClick = {}, switch = true, modifier = Modifier.testTag("row")) }
        rule.onNodeWithTag("row").assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Switch))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.ToggleableState, ToggleableState.On))
    }

    @Test
    fun a_selected_row_and_a_tab_say_selected() {
        rule.setContent {
            TvListRow("English", onClick = {}, selected = true, modifier = Modifier.testTag("row"))
            TvTab("Episodes", selected = true, onSelect = {}, modifier = Modifier.testTag("tab"))
        }
        rule.onNodeWithTag("row").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        rule.onNodeWithTag("tab").assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
    }
}
