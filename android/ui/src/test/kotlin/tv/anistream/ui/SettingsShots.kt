package tv.anistream.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import java.time.ZoneId
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.engine.model.Event
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.FixtureArt
import tv.anistream.testkit.Scene

/**
 * Settings and what it opens, drawn on the JVM the way [BoardShots] draws the browse screens:
 * a recorded scene replayed into the app's own store, at each board's size, the page opened by its
 * route, and a menu or a sheet opened by tapping what opens it.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class SettingsShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings() = shoot("phone-settings", settingsRoute(null))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_playback() = shoot("phone-settings-playback", settingsRoute(SettingsPane.Playback))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_titles() = shoot("phone-settings-titles", settingsRoute(SettingsPane.Playback), tap = "Title language")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_sources() = shoot("phone-settings-sources", settingsRoute(SettingsPane.Sources))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_accounts() = shoot("phone-settings-accounts", settingsRoute(SettingsPane.Accounts))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_downloads() = shoot("phone-settings-downloads", settingsRoute(SettingsPane.Downloads))

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_settings() = shoot("tablet-settings", settingsRoute(SettingsPane.Playback))

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_settings_titles() = shoot("tablet-settings-titles", settingsRoute(SettingsPane.Playback), tap = "Title language", dismiss = true)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_accounts_signed_out() = shoot("phone-accounts-signed-out", settingsRoute(SettingsPane.Accounts), scene = "fresh")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_kitsu_sheet() = shoot("phone-kitsu-sheet", settingsRoute(SettingsPane.Accounts), scene = "fresh", tap = "Sign in", tapIndex = 2)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_device_code() = shoot("phone-device-code", settingsRoute(SettingsPane.Accounts), scene = "fresh", screen = true) { store ->
        store.apply(listOf(Event.DeviceCode(DeviceCodePrompt(tracker = "trakt", code = "4F2K9QX1", url = "https://trakt.example/activate"))))
    }

    /**
     * [scene] replayed into a store, the app drawn with [page] open over Continue, and captured
     * once the art has landed. [tap] taps the node with that text first ([tapIndex] of them), and
     * the whole screen is captured, so a menu's or a sheet's window shows; [dismiss] closes the
     * menu again, for the tablet's column, which follows the row tapped. [then] feeds the store
     * more once the page is up.
     */
    private fun shoot(
        name: String,
        page: String,
        tap: String? = null,
        dismiss: Boolean = false,
        scene: String = "watching",
        tapIndex: Int = 0,
        screen: Boolean = false,
        then: (AppStore) -> Unit = {},
    ) {
        val recorded = Scene.named(scene)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val events = recorded.events.filter(::replayed)
        store.apply(events)

        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            AnistreamTheme { AppRoot(store, Destination.Continue, startPage = page) }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        store.apply(events)
        awaitArt(store)
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        compose.runOnUiThread { then(store) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        if (tap == null) {
            if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
            return
        }
        compose.onAllNodesWithText(tap)[tapIndex].performClick()
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        if (dismiss) {
            compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
            compose.mainClock.advanceTimeBy(1_000)
            compose.waitForIdle()
            awaitArt(store)
            compose.onRoot().captureRoboImage(file)
        } else {
            captureScreenRoboImage(file)
        }
    }

    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 20_000
        var landed = -1
        var still = System.currentTimeMillis()
        while (System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            val now = store.artwork.arrivals.value.size
            if (now != landed) {
                landed = now
                still = System.currentTimeMillis()
            } else if (System.currentTimeMillis() - still > 500) {
                return
            }
            Thread.sleep(20)
        }
    }

    private fun systemBars(activity: ComponentActivity) {
        val bar = (24 * activity.resources.displayMetrics.density).roundToInt()
        val status = WindowInsetsCompat.Type.statusBars()
        val navigation = WindowInsetsCompat.Type.navigationBars()
        val insets = WindowInsetsCompat.Builder()
            .setInsets(status, Insets.of(0, bar, 0, 0))
            .setInsets(navigation, Insets.of(0, 0, 0, bar))
            .setVisible(status, true)
            .setVisible(navigation, true)
            .build()
        val content = activity.findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
        ViewCompat.dispatchApplyWindowInsets(content, insets)
    }

    private companion object {
        const val PHONE = "w412dp-h915dp-port-420dpi"
        const val TABLET = "w1280dp-h800dp-land-xhdpi"
        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
