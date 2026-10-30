package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.semantics.SemanticsProperties
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE

/** Keystrokes that arrive faster than the store's echo of them are all kept. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class SearchTypingTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    @Test
    fun a_burst_of_letters_all_stay_in_the_field() {
        assumeTrue(HAS_TESTING_ENGINE)
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(LocalAppStore provides store) { SearchScreen(open = true) }
            }
        }
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        val field = compose.onAllNodes(hasSetTextAction()).onFirst()
        for (letter in "garden") {
            field.performTextInput(letter.toString())
            // One frame per key: the store's echo of the last key lands after the next one is typed.
            compose.mainClock.advanceTimeByFrame()
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        val shown = field.fetchSemanticsNode().config[SemanticsProperties.EditableText].text
        assertEquals("garden", shown)
        assertEquals("garden", store.search.status.value.query.text)
    }
}
