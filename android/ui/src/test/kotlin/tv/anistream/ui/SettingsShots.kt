package tv.anistream.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
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
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.InvitePreview
import tv.anistream.engine.model.InvitedStore
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.StorePluginRow
import tv.anistream.engine.model.StorePluginStatus
import tv.anistream.engine.model.StoreRow
import tv.anistream.engine.model.StoreState
import tv.anistream.engine.model.ProviderRow
import tv.anistream.engine.model.ProviderUpdate
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
    fun phone_settings_notifications() = shoot("phone-settings-notifications", settingsRoute(SettingsPane.Notifications)) { store ->
        // The week coming up, as the engine lists it for the watching scene's shows.
        val now = AppClock.now().epochSecond
        store.apply(
            listOf(
                Event.ComingEpisodes(
                    listOf(
                        tv.anistream.engine.model.ComingEpisode(900002u, "The Library Above the Clouds", 6u, now + 2 * 86_400 + 11_700, coverUrl = coverOf(store, 900002u)),
                        tv.anistream.engine.model.ComingEpisode(900006u, "Summer Wind Relay", 9u, now + 6 * 86_400 + 18_000, coverUrl = coverOf(store, 900006u), switchedOn = true),
                    ),
                ),
            ),
        )
    }

    private fun coverOf(store: AppStore, id: UInt): String? = store.titles[id]?.coverUrl

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

    @Test
    @Config(qualifiers = PHONE)
    fun phone_providers() = shoot("phone-providers", PROVIDERS_ROUTE) { it.apply(listOf(Event.Providers(PROVIDERS))) }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_providers_reorder() = shoot("phone-providers-reorder", PROVIDERS_ROUTE, tap = "Reorder", byDescription = true, screen = true) {
        it.apply(listOf(Event.Providers(PROVIDERS)))
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_provider() = shoot("phone-provider", providerRoute("mukashi")) { it.apply(listOf(Event.Providers(PROVIDERS))) }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_store() = shoot("phone-store", storeRoute("hotaru")) { it.apply(listOf(Event.Stores(listOf(HOTARU)))) }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_stores() = shoot("phone-stores", STORES_ROUTE) { it.apply(listOf(Event.Stores(listOf(HOTARU)))) }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_join_store() = shoot("phone-join-store", settingsRoute(SettingsPane.Sources), screen = true) { store ->
        store.stores.openJoin(INVITE)
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(InvitedStore(INVITE, "Hotaru Plugins", "https://plugins.hotaru.example", "plugins.hotaru.example", "9f3c 1a2b 77de 0c41", alreadyJoined = false)))))
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_store_update() = shoot("phone-store-update", storeRoute("hotaru"), tap = "Update") { it.apply(listOf(Event.Stores(listOf(HOTARU)))) }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_downloads() = shoot("phone-downloads", DOWNLOADS_ROUTE) { it.apply(listOf(Event.Downloads(DOWNLOADS))) }

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
        byDescription: Boolean = false,
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
        compose.waitForIdle()
        val nodes = if (byDescription) compose.onAllNodesWithContentDescription(tap) else compose.onAllNodesWithText(tap)
        nodes[tapIndex].performClick()
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

        private fun provider(
            id: String,
            name: String,
            kind: String = "plugin",
            origin: ProviderOrigin = ProviderOrigin.Store("hotaru", "Hotaru Plugins"),
            usable: Boolean = true,
            disabled: Boolean = false,
            update: ProviderUpdate? = null,
            hosts: List<String> = emptyList(),
        ) = ProviderRow(
            id = id, kind = kind, state = if (usable) "ready" else "down", latencyMs = if (usable) 240uL else null,
            lastError = if (usable) null else "timed out", usable = usable && !disabled, heldBack = false, name = name,
            version = if (kind == "plugin") "1.4.0" else null, hosts = hosts, origin = origin, disabled = disabled, update = update,
        )

        const val INVITE = "https://plugins.hotaru.example/join/4kq2v9#9f3c1a2b77de0c41"

        /** The boards' store, AndroidStore's: three plugins installed, one with an update, one to install. */
        val HOTARU = StoreRow(
            id = "hotaru", name = "Hotaru Plugins", url = "https://plugins.hotaru.example", host = "plugins.hotaru.example",
            fingerprint = "9f3c 1a2b 77de 0c41", member = "Mika", owner = false, state = StoreState.Ready, checkedAt = null,
            plugins = listOf(
                StorePluginRow("archive", "Hotaru Open Archive", "2.3.0", listOf("open-archive.hotaru.example", "media.open-archive.hotaru.example"), StorePluginStatus.Installed),
                StorePluginRow("mukashi", "Mukashi Archive", "1.5.0", listOf("mukashi-archive.example", "media2.mukashi-archive.example"), StorePluginStatus.Update("1.4.0", listOf("media2.mukashi-archive.example"))),
                StorePluginRow("kobunko", "Kōbunko Film Library", "0.9.1", listOf("films.kobunko.example"), StorePluginStatus.Installed),
                StorePluginRow("shorts", "Atelier Hotaru Shorts", "1.0.2", listOf("shorts.atelier-hotaru.example"), StorePluginStatus.Available),
            ),
        )

        private fun download(id: Long, title: UInt, name: String, episode: String, state: DownloadState, downloaded: ULong, total: ULong, error: String? = null) =
            DownloadRow(
                id = id, anilistId = title, title = name, episode = episode, state = state,
                fraction = if (total > 0uL) downloaded.toDouble() / total.toDouble() else 0.0,
                downloaded = downloaded, total = total, error = error, path = if (state == DownloadState.DONE) "/files/$id.mkv" else null,
            )

        /** AndroidDownloads' queue, on the fixtures' titles. */
        val DOWNLOADS = listOf(
            download(1, 900001u, "Hoshikuzu", "8", DownloadState.DONE, 1_400_000_000uL, 1_400_000_000uL),
            download(2, 900001u, "Hoshikuzu", "9", DownloadState.ACTIVE, 300_000_000uL, 1_400_000_000uL),
            download(3, 900001u, "Hoshikuzu", "10", DownloadState.QUEUED, 0uL, 0uL),
            download(4, 900002u, "Kumo", "3", DownloadState.DONE, 460_000_000uL, 460_000_000uL),
            download(5, 900002u, "Kumo", "4", DownloadState.FAILED, 130_000_000uL, 480_000_000uL, error = "the source stopped responding at 130 MB"),
            download(6, 900003u, "Tsukiakari", "5", DownloadState.PAUSED, 210_000_000uL, 480_000_000uL),
        )

        /** The boards' sources: four plugins and the built-in torrent source, last. */
        val PROVIDERS = listOf(
            provider("archive", "Hotaru Open Archive", hosts = listOf("open-archive.hotaru.example", "media.open-archive.hotaru.example")),
            provider("mukashi", "Mukashi Archive", hosts = listOf("mukashi-archive.example"), update = ProviderUpdate("1.5.0", listOf("media2.mukashi-archive.example"))),
            provider("machi", "Machi Library Films", origin = ProviderOrigin.ByHand, usable = false, hosts = listOf("films.machi.example")),
            provider("kobunko", "Kōbunko Film Library", disabled = true, hosts = listOf("films.kobunko.example")),
            provider("torrent", "torrent", kind = "torrent", origin = ProviderOrigin.BuiltIn),
        )

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
