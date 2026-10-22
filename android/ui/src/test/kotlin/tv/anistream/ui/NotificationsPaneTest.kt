package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.NoticeReason
import tv.anistream.engine.model.UpcomingNotice
import tv.anistream.engine.model.UpcomingNotices
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.Scene

/**
 * Settings' Notifications pane on the watching scene's settings: the New episodes switch, which
 * asks the app to turn it on rather than turning it itself, the line that says why it is off when
 * Android did not allow it, and the system's settings for the channel.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class NotificationsPaneTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    /** The switch says off, as SwitchRow tells TalkBack. */
    private val off = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Off")

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        store.apply(Scene.named("watching").events.filterIsInstance<Event.Settings>())
    }

    private fun pane(actions: AppActions) {
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Column(Modifier.verticalScroll(rememberScrollState())) { NotificationsPane(actions) }
                }
            }
        }
    }

    @Test
    fun turning_the_switch_on_asks_the_app_which_decides() {
        val turns = mutableListOf<Boolean>()
        var opened = 0
        pane(AppActions(notifications = NotificationActions({ turns += it }, MutableStateFlow(false)) { opened++ }))
        // The switch's row, by its line: "New episodes" also heads the group.
        val row = compose.onAllNodesWithText("For shows in your Watching list", substring = true)[0]
        row.assert(off)
        row.performClick()
        // The app turns it on once it may post; the row only asks, and sends nothing itself.
        assertEquals(listOf(true), turns)
        assertTrue(store.waiting.none { it is Command.SetSetting })
        compose.onNodeWithText("Notification settings").performClick()
        assertEquals(1, opened)
    }

    @Test
    fun where_android_said_no_the_line_says_so() {
        pane(AppActions(notifications = NotificationActions({}, MutableStateFlow(true)) {}))
        assertTrue(compose.onAllNodesWithText(NOTIFICATIONS_REFUSED, substring = true).fetchSemanticsNodes().isNotEmpty())
    }

    @Test
    fun coming_up_lists_the_engines_week_and_says_which_a_show_switched_on_for_itself() {
        pane(AppActions())
        // The pane asks for the week as it opens.
        assertTrue(Command.UpcomingNotices in store.waiting)
        compose.runOnUiThread {
            store.apply(
                Event.UpcomingNotices(
                    UpcomingNotices(
                        enabled = false,
                        generatedAt = 1_792_000_000,
                        airings = listOf(
                            UpcomingNotice(900002u, "The Library Above the Clouds", 6u, 1_792_163_730, NoticeReason.WATCHING, 4u),
                            UpcomingNotice(900006u, "Summer Wind Relay", 9u, 1_792_515_630, NoticeReason.SWITCHED_ON, 9u, notifies = true),
                            UpcomingNotice(900003u, "Amber Forest", 2u, 1_792_600_000, NoticeReason.SWITCHED_OFF, 2u),
                        ),
                        overrides = 2u,
                    ),
                ),
            )
        }
        compose.waitForIdle()
        assertTrue(compose.onAllNodesWithText("Coming up · next 7 days").fetchSemanticsNodes().isNotEmpty())
        assertTrue(compose.onAllNodesWithText("E6 · ", substring = true).fetchSemanticsNodes().isNotEmpty())
        assertEquals(1, compose.onAllNodesWithText("On in this show’s settings", substring = true).fetchSemanticsNodes().size)
        // A show its own switch leaves out is named under the week, not listed.
        assertTrue(compose.onAllNodesWithText("E2 · ", substring = true).fetchSemanticsNodes().isEmpty())
        assertTrue(compose.onAllNodesWithText("Amber Forest is off in its own settings. A show opens its settings.").fetchSemanticsNodes().isNotEmpty())
    }

    @Test
    fun without_the_app_the_switch_turns_the_setting() {
        pane(AppActions())
        compose.onAllNodesWithText("For shows in your Watching list", substring = true)[0].performClick()
        assertTrue(Command.SetSetting(SettingKey.EPISODE_NOTIFICATIONS, SettingValue.Bool(true)) in store.waiting)
        assertTrue(compose.onAllNodesWithText("Notification settings").fetchSemanticsNodes().isEmpty())
    }
}
