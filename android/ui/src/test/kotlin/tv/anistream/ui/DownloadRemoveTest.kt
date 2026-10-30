package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.longClick
import androidx.navigation.compose.rememberNavController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.engine.model.Event
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE

/**
 * A download that was paused, resumed and is queued again still holds its bytes. The control that
 * took the place of Pause must not throw them away on one tap.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class DownloadRemoveTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun row(id: Long, state: DownloadState, downloaded: ULong) = DownloadRow(
        id = id, anilistId = 900001u, title = "Stardust Wayfarer", episode = "7", state = state,
        fraction = downloaded.toDouble() / 1_000_000.0, downloaded = downloaded, total = 1_000_000uL, error = null, path = null,
    )

    private fun screen(row: DownloadRow) {
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Continue),
                ) { DownloadsScreen() }
            }
        }
        compose.runOnUiThread { store.apply(Event.Downloads(listOf(row))) }
        compose.waitForIdle()
    }

    @Test
    fun removing_a_queued_download_that_has_bytes_asks_first() {
        assumeTrue(HAS_TESTING_ENGINE)
        screen(row(1, DownloadState.QUEUED, 147_000uL))
        compose.onNodeWithContentDescription("Remove from queue").performClick()
        compose.waitForIdle()
        assertTrue("nothing was sent before the answer", store.waiting.none { it is Command.DownloadCancel || it is Command.DownloadDelete })
        compose.onNodeWithText("Delete").assertExists()
    }

    @Test
    fun removing_a_queued_download_with_nothing_yet_is_one_tap() {
        assumeTrue(HAS_TESTING_ENGINE)
        screen(row(2, DownloadState.QUEUED, 0uL))
        compose.onNodeWithContentDescription("Remove from queue").performClick()
        compose.waitForIdle()
        assertTrue(store.waiting.any { it is Command.DownloadCancel })
    }

    /** The long press says the same as the X: a paused row's bytes go only after the question. */
    @Test
    fun the_menu_of_a_paused_download_with_bytes_deletes_after_asking() {
        assumeTrue(HAS_TESTING_ENGINE)
        screen(row(3, DownloadState.PAUSED, 147_000uL))
        compose.onNodeWithText("Episode 7", substring = true).performTouchInput { longClick() }
        compose.waitForIdle()
        compose.onNodeWithText("Remove from list").assertDoesNotExist()
        compose.onNodeWithText("Delete download").performClick()
        compose.waitForIdle()
        assertTrue("nothing was sent before the answer", store.waiting.none { it is Command.DownloadCancel || it is Command.DownloadDelete })
        compose.onNodeWithText("Delete").performClick()
        compose.waitForIdle()
        assertTrue(store.waiting.any { it is Command.DownloadDelete })
    }
}
