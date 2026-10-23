package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import java.io.File
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.design.AnistreamTheme
import tv.anistream.store.ApkFacts
import tv.anistream.store.ApkFetch
import tv.anistream.store.AppStore
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.UpdateStatus
import tv.anistream.store.UpdateCheck
import tv.anistream.store.UpdateFiles
import tv.anistream.store.UpdateHost
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState

/**
 * The update on screen: Settings' row from "Update to v2.0.3" with its size through "Downloading
 * 34 %" to "Installing", the notice that a release is out with Update as its button, the notice
 * that follows the download, and the sentence that asks for "Install unknown apps".
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class AppUpdateTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val store = AppStore(scope)
    private val offer = UpdateOffer("2.0.3", "https://example.invalid/anistream-2.0.3-android-arm64.apk", "ab".repeat(32), 48_200_000, "https://example.invalid/releases/v2.0.3")

    @After
    fun tearDown() {
        scope.cancel()
    }

    /** Android, as the update sees it, saying what it was asked. */
    private class Host : UpdateHost {
        var allowed = true
        val calls = mutableListOf<String>()
        override fun canInstall() = allowed
        override fun openInstallSettings(offer: UpdateOffer) {
            calls += "settings"
        }
        override fun returnedFromSettings() {}
        override fun startDownload(offer: UpdateOffer): Boolean {
            calls += "download"
            return true
        }
        override fun stopDownload() {
            calls += "stop"
        }
        override fun inspect(file: File): ApkFacts? = null
        override fun installed() = ApkFacts("tv.anistream.app", 1, emptySet())
        override fun install(file: File, offer: UpdateOffer) {}
        override fun abandonInstall() {}
    }

    private fun host(): Host = Host().also { store.updates.attach(it, UpdateFiles(File(compose.activity.cacheDir, "updates"))) }

    private fun show(content: @Composable () -> Unit) {
        compose.setContent {
            AnistreamTheme {
                val snackbarScope = rememberCoroutineScope()
                val snackbars = remember { Snackbars(SnackbarHostState(), snackbarScope) }
                CompositionLocalProvider(LocalAppStore provides store, LocalSnackbars provides snackbars) {
                    Column {
                        content()
                        SnackbarHost(snackbars.host) { NoticeSnackbar(it) }
                    }
                }
            }
        }
    }

    /**
     * Wait for [done], running what the main thread was handed meanwhile: the update's run comes
     * back to it from the download's thread.
     */
    private fun settle(done: () -> Boolean) {
        compose.waitUntil(5_000) {
            org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle()
            done()
        }
    }

    private fun aboutPane() = show { Column(Modifier.verticalScroll(rememberScrollState())) { AboutPane() } }

    @Test
    fun about_says_update_to_the_release_with_its_size_and_nothing_without_one() {
        host()
        aboutPane()
        compose.onNodeWithText("Check for updates").assertIsDisplayed()
        assertTrue(compose.onAllNodes(androidx.compose.ui.test.hasText("Update to", substring = true)).fetchSemanticsNodes().isEmpty())
        compose.runOnIdle { store.updates.offer(offer) }
        compose.onNodeWithText("Update to v2.0.3").assertIsDisplayed()
        compose.onNodeWithText("48 MB").assertIsDisplayed()
    }

    @Test
    fun where_the_build_does_not_install_it_the_row_opens_the_release_page() {
        aboutPane()
        compose.runOnIdle { store.updates.offer(offer) }
        compose.onNodeWithText("Update to v2.0.3").assertIsDisplayed()
        compose.onNodeWithText("Opens the release page").assertIsDisplayed()
    }

    @Test
    fun pressed_it_downloads_with_its_percent_and_cancel_stops_it() {
        val host = host()
        val gate = CompletableDeferred<Unit>()
        val fetch = ApkFetch { _, _, expected, progress ->
            progress(16_400_000, expected)
            gate.await()
        }
        aboutPane()
        compose.runOnIdle { store.updates.offer(offer) }
        compose.onNodeWithText("Update to v2.0.3").performClick()
        compose.runOnIdle {
            assertEquals(listOf("download"), host.calls)
            scope.launch { store.updates.run(fetch) }
        }
        settle { (store.updates.state.value as? UpdateState.Downloading)?.done == 16_400_000L }
        compose.onNodeWithText("Downloading 34 %").assertIsDisplayed()
        compose.onNodeWithText("16 MB of 48 MB").assertIsDisplayed()
        compose.onNodeWithText("Cancel").performClick()
        compose.runOnIdle {
            assertEquals("stop", host.calls.last())
            assertEquals(UpdateState.Available(offer), store.updates.state.value)
        }
        compose.onNodeWithText("Update to v2.0.3").assertIsDisplayed()
    }

    @Test
    fun the_first_time_it_asks_for_install_unknown_apps_in_one_sentence() {
        val host = host().apply { allowed = false }
        show {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                AboutPane()
                UpdatePrompts()
            }
        }
        compose.runOnIdle { store.updates.offer(offer) }
        compose.onNodeWithText("Update to v2.0.3").performClick()
        compose.onNodeWithText("Install unknown apps").assertIsDisplayed()
        compose.onNodeWithText("To install updates, Android needs you to turn on Allow from this source for anistream.").assertIsDisplayed()
        compose.onNodeWithText("Open settings").performClick()
        compose.runOnIdle {
            assertEquals(listOf("settings"), host.calls)
            assertEquals(UpdateState.Allowing(offer, opened = true), store.updates.state.value)
        }
        // Down while Android's page is open.
        assertTrue(compose.onAllNodes(androidx.compose.ui.test.hasText("Open settings")).fetchSemanticsNodes().isEmpty())
        // Back with the switch on, the download starts.
        host.allowed = true
        compose.runOnIdle { store.updates.resumed() }
        compose.runOnIdle { assertEquals("download", host.calls.last()) }
        compose.onNodeWithText("Downloading", substring = true).assertIsDisplayed()
    }

    @Test
    fun a_failure_says_why_under_the_row_and_as_a_notice() {
        host()
        aboutPane()
        compose.runOnIdle {
            store.updates.offer(offer)
            store.updates.press()
            // The file the fetch hands over is not the release's.
            scope.launch { store.updates.run { _, into, _, _ -> into.parentFile?.mkdirs(); into.writeText("some other file") } }
        }
        settle { store.updates.state.value is UpdateState.Failed }
        compose.onNodeWithText("The download does not match the release's checksum, so it was not installed.").assertIsDisplayed()
        compose.onNodeWithText("Update to v2.0.3").assertIsDisplayed()
        assertEquals("The download does not match the release's checksum, so it was not installed.", store.notices.current.value?.notice?.text)
    }

    @Test
    fun a_release_found_is_said_once_as_a_notice_with_update_as_its_button() {
        val host = host()
        show { UpdatePrompts() }
        compose.runOnIdle { store.updates.offer(offer, tell = true) }
        compose.onNodeWithText("anistream v2.0.3 is out.").assertIsDisplayed()
        compose.onNodeWithText("Update to v2.0.3").performClick()
        compose.runOnIdle { assertEquals(listOf("download"), host.calls) }
    }

    @Test
    fun the_notice_follows_the_download_with_its_line_and_cancel_and_settings_hides_it() {
        host()
        val gate = CompletableDeferred<Unit>()
        val onSettings = androidx.compose.runtime.mutableStateOf(false)
        show {
            UpdateNotice(hidden = onSettings.value)
        }
        compose.runOnIdle {
            store.updates.offer(offer)
            store.updates.press()
            scope.launch { store.updates.run { _, _, expected, progress -> progress(16_400_000, expected); gate.await() } }
        }
        settle { (store.updates.state.value as? UpdateState.Downloading)?.done == 16_400_000L }
        compose.onNodeWithText("Downloading v2.0.3 · 34 %").assertIsDisplayed()
        // Settings' row says the same, so the notice is down there.
        compose.runOnIdle { onSettings.value = true }
        settle { compose.onAllNodes(androidx.compose.ui.test.hasText("Downloading", substring = true)).fetchSemanticsNodes().isEmpty() }
        compose.runOnIdle { onSettings.value = false }
        compose.onNodeWithText("Cancel").performClick()
        compose.runOnIdle { assertEquals(UpdateState.Available(offer), store.updates.state.value) }
        settle { compose.onAllNodes(androidx.compose.ui.test.hasText("Downloading", substring = true)).fetchSemanticsNodes().isEmpty() }
    }

    @Test
    fun the_words_at_each_step() {
        assertEquals("Update to v2.0.3", UpdateWords.title(UpdateState.Available(offer)))
        assertEquals("48 MB", UpdateWords.line(UpdateState.Available(offer), installs = true))
        assertEquals("Downloading 34 %", UpdateWords.title(UpdateState.Downloading(offer, 16_400_000, 48_200_000)))
        assertEquals("Downloading", UpdateWords.title(UpdateState.Downloading(offer, 16_400_000, 0)))
        assertEquals("16 MB", UpdateWords.line(UpdateState.Downloading(offer, 16_400_000, 0), installs = true))
        assertEquals("Installing", UpdateWords.title(UpdateState.Verifying(offer)))
        assertEquals("Checking the download", UpdateWords.line(UpdateState.Verifying(offer), installs = true))
        assertEquals("Installing", UpdateWords.title(UpdateState.Installing(offer)))
        assertEquals("Installing v2.0.3", UpdateWords.notice(UpdateState.Installing(offer)))
        assertEquals("Open release", UpdateWords.action(offer, installs = false))
        assertEquals("Checking", UpdateWords.check(UpdateCheck.Checking))
        assertEquals("anistream is up to date", UpdateWords.check(UpdateCheck.UpToDate))
        assertEquals(null, UpdateWords.check(UpdateCheck.Found))
        assertEquals("Not checked: GitHub could not be reached", UpdateWords.check(UpdateCheck.Failed("GitHub could not be reached.")))
    }

    @Test
    fun check_now_asks_the_engine_and_says_what_it_found() {
        aboutPane()
        compose.onNodeWithText("Check now").performClick()
        compose.runOnIdle { assertEquals(listOf(Command.CheckForUpdates), store.waiting) }
        compose.runOnIdle { store.apply(Event.Update(UpdateStatus.Checking)) }
        compose.onNodeWithText("Checking").assertIsDisplayed()
        compose.runOnIdle { store.apply(Event.Update(UpdateStatus.UpToDate)) }
        compose.onNodeWithText("anistream is up to date").assertIsDisplayed()
    }
}
