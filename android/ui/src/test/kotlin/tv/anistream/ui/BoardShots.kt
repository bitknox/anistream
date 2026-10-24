package tv.anistream.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToKeyAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToKey
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import java.time.ZoneId
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Event
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.FixtureArt
import tv.anistream.testkit.Scene

/**
 * Each board's screen, drawn on the JVM from a scene the testing engine recorded, at the board's
 * size: no emulator. The store is the app's own, fed the scene's events with no engine behind it,
 * and the clock held at the moment the scene was recorded, so every run draws the same pixels.
 *
 * `./gradlew :ui:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/, and
 * android/scripts/pairs.sh sets them beside the boards.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class BoardShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() {
        // The fixtures flavour's: the scenes are the testing engine's, and so are its titles.
        assumeTrue(HAS_TESTING_ENGINE)
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home() = shoot("phone-home", WATCHING)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_welcome() = shoot("phone-welcome", FRESH)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title() = shoot("phone-title", WATCHING, title = 900001u)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_calendar() = shoot("phone-calendar", WATCHING, start = Destination.Calendar)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search() = shoot("phone-search", WATCHING, start = Destination.Search, query = "hoshi")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_library() = shoot("phone-library", WATCHING, start = Destination.Library)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_more_about() = shoot("phone-more-about", WATCHING, title = 900001u) { open("More about this show") }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_show_settings() = shoot("phone-show-settings", WATCHING, title = 900001u) {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        open("Settings for this show")
    }

    // TalkBack and the keyboard.

    /** An episode row says what a tap and a hold do: it plays, and it opens its options. */
    @Test
    @Config(qualifiers = PHONE)
    fun episode_rows_name_their_actions() = shoot("phone-title-episodes", WATCHING, title = 900001u) {
        toEpisodes()
        val row = compose.onNode(hasClickAction() and hasAnyDescendant(hasText("E7 \u00b7 14", substring = true)), useUnmergedTree = true)
        row.assert(SemanticsMatcher("plays on a tap") { it.config.getOrNull(SemanticsActions.OnClick)?.label == "Play" })
        row.assert(SemanticsMatcher("opens its options on a hold") { it.config.getOrNull(SemanticsActions.OnLongClick)?.label == "Episode options" })
    }

    // Font scale at 200 %, the largest the system offers: nothing cut, nothing overlapping.

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_home_large_text() = shoot("phone-home-large-text", WATCHING)

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_large_text() = shoot("phone-title-large-text", WATCHING, title = 900001u)

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_episodes_large_text() = shoot("phone-title-episodes-large-text", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_calendar_large_text() = shoot("phone-calendar-large-text", WATCHING, start = Destination.Calendar)

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_library_large_text() = shoot("phone-library-large-text", WATCHING, start = Destination.Library)

    @Test
    @Config(qualifiers = TABLET, fontScale = 2.0f)
    fun tablet_home_large_text() = shoot("tablet-home-large-text", WATCHING)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_home() = shoot("tablet-home", WATCHING)

    /** AT2: the divider dragged past the middle settles on half the content. */
    @Test
    @Config(qualifiers = TABLET)
    fun tablet_home_divider() = shoot("tablet-home-divider", WATCHING) {
        // The handle stands where the list pane ends: the rail's 96 and the list's 380 in.
        compose.onRoot().performTouchInput {
            val handle = Offset(483.dp.toPx(), height / 2f)
            down(handle)
            moveBy(Offset(240.dp.toPx(), 0f), delayMillis = 200)
            up()
        }
        compose.waitForIdle()
    }

    /** AT2: another title picked, its fill on the row and its preview beside the list. */
    @Test
    @Config(qualifiers = TABLET)
    fun tablet_home_selected() = shoot("tablet-home-selected", WATCHING) {
        compose.onNodeWithText("Resume E2", substring = true).performClick()
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_more_about() = shoot("tablet-more-about", WATCHING, title = 900001u) { open("More about this show") }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title() = shoot("tablet-title", WATCHING, title = 900001u)

    // A title no source has: its episodes AniList's alone, marked watched as any are, and the way
    // to a source in Play's place.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_no_sources() = shoot("phone-title-no-sources", WATCHING, title = 900001u, sourceless = true)

    /** Its rows as any are, but a tap opens their options rather than playing. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes_no_sources() =
        shoot("phone-title-episodes-no-sources", WATCHING, title = 900001u, sourceless = true) {
            toEpisodes()
            val rows = compose.onAllNodes(hasClickAction() and hasAnyDescendant(hasText("E8 \u00b7", substring = true)), useUnmergedTree = true)
            rows.onFirst().assert(SemanticsMatcher("opens its options on a tap") { it.config.getOrNull(SemanticsActions.OnClick)?.label == "Episode options" })
        }

    /** A row's options offer to add a source, which opens Providers. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_no_sources_to_providers() =
        shoot("phone-title-no-sources-to-providers", WATCHING, title = 900001u, sourceless = true) {
            toEpisodes()
            compose.onAllNodesWithText("E8 \u00b7", substring = true).onFirst().performClick()
            compose.waitForIdle()
            // The page's own button, scrolled away, and the menu's item, which is the last.
            compose.onAllNodesWithText(ADD_A_SOURCE).onLast().performClick()
            compose.waitForIdle()
            compose.onAllNodesWithText(ADD_A_SOURCE).assertCountEquals(0)
        }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_no_sources() = shoot("tablet-title-no-sources", WATCHING, title = 900001u, sourceless = true)

    // The episodes from the last one watched, the watched ones before it folded, and unfolded.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes() = shoot("phone-title-episodes", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes_unfolded() = shoot("phone-title-episodes-unfolded", WATCHING, title = 900001u) { unfoldInPlace() }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_episodes() = shoot("tablet-title-episodes", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_episodes_unfolded() = shoot("tablet-title-episodes-unfolded", WATCHING, title = 900001u) { unfoldInPlace() }

    // A long franchise's picker: twelve seasons of Cloudcutter Airship, more than fit across, so
    // it scrolls, the season on screen in the middle.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long() = shoot("phone-title-long", WATCHING, title = 920009u)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_first() = shoot("phone-title-long-first", WATCHING, title = 920001u)

    /** A jump from the ninth season to the twelfth, whose page was not laid out: settled on its own page. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_jump() = shoot("phone-title-long-jump", WATCHING, title = 920009u) { pickSeason("Season 12") }

    /**
     * The same jump halfway through its crossfade: the stand-in, with the twelfth season's name and
     * cover and T8's placeholders, fading in over the ninth.
     */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_jump_halfway() = shoot("phone-title-long-jump-halfway", WATCHING, title = 920009u, settle = false) {
        compose.onNodeWithText("Season 12").performScrollTo()
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Season 12").performClick()
        compose.mainClock.advanceTimeBy(120)
    }

    /** A jump to a season whose title has not arrived: its card's name and cover, and T8's placeholders. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_waiting() = shoot("phone-title-long-waiting", WATCHING, title = 920009u) { pickSeason("Season 5") }

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_long_large_text() = shoot("phone-title-long-large-text", WATCHING, title = 920009u)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_long() = shoot("tablet-title-long", WATCHING, title = 920009u)

    // A title long enough to wrap, on the lightest banner a title can have: its first line over
    // the art must still read.
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_pale() = shoot("phone-title-pale", WATCHING, title = 900001u, pale = true)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_pale() = shoot("tablet-title-pale", WATCHING, title = 900001u, pale = true)

    /** Tap the season [label] names in the picker, scrolled to first. */
    private fun pickSeason(label: String) {
        compose.onNodeWithText(label).performScrollTo().performClick()
    }

    /** The title page scrolled to the row its watched episodes fold under. */
    private fun toEpisodes() {
        compose.onNode(hasScrollToKeyAction() and hasAnyDescendant(hasText("More about this show")))
            .performScrollToKey(watchedFold(900001u))
        compose.waitForIdle()
    }

    /**
     * Unfold the watched episodes, checking that the last one watched stays where it was on screen
     * while they open above it, then scroll to the row that folds them again.
     */
    private fun unfoldInPlace() {
        toEpisodes()
        val lastWatched = compose.onNodeWithText("E6 \u00b7", substring = true)
        val before = lastWatched.getUnclippedBoundsInRoot().top
        compose.onNodeWithText("watched", substring = true).performClick()
        compose.waitForIdle()
        assertEquals("the last episode watched stays put", before, lastWatched.getUnclippedBoundsInRoot().top)
        compose.onNodeWithText("Hide watched episodes").assertExists()
        toEpisodes()
    }

    /**
     * [scene] replayed into a store, the app drawn on it from [start], and captured once the art
     * has landed and every transition has run.
     */
    private fun shoot(
        name: String,
        scene: String,
        start: Destination = Destination.Continue,
        title: AnilistId? = null,
        query: String? = null,
        /** Whether every transition runs to its end before the capture; else it is taken where [then] left the clock. */
        settle: Boolean = true,
        sourceless: Boolean = false,
        /** Whether 900001 has the long title and the near-white banner of [PaleBanner]. */
        pale: Boolean = false,
        then: (() -> Unit)? = null,
    ) {
        val recorded = Scene.named(scene)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        // The art the screens ask for, as the fake AniList serves it.
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = { url -> PaleBanner.bytes(url) ?: FixtureArt.bytes(url) })
        val events = recorded.events.filter(::replayed).map { if (sourceless) withoutSources(it) else it }
        // After the scene each time, so its own detail never wins over the pale one.
        fun replay() {
            store.apply(events)
            if (pale) store.apply(listOf(Event.Detail(PaleBanner.retitle(checkNotNull(store.titles[PaleBanner.ID])))))
        }
        replay()

        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            AnistreamTheme {
                AppRoot(store, if (query != null) Destination.Search else start, startTitle = title, startQuery = query)
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        // What the screens asked for as they opened is answered as the engine answered it.
        replay()
        awaitArt(store)
        then?.let {
            it()
            if (settle) {
                compose.waitForIdle()
                awaitArt(store)
            }
        }
        if (settle) {
            compose.mainClock.advanceTimeBy(3_000)
            compose.waitForIdle()
        }
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /** Tap what [label] names, scrolled to first where it sits in a list. */
    private fun open(label: String) {
        val node = compose.onNodeWithText(label)
        runCatching { node.performScrollTo() }
        node.performClick()
    }

    /** Wait until the art has stopped landing: decoded off the main thread, then kept on it. */
    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 20_000
        var landed = -1
        var still = System.currentTimeMillis()
        while (System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            val now = store.artwork.landed.value
            if (now != landed) {
                landed = now
                still = System.currentTimeMillis()
            } else if (System.currentTimeMillis() - still > 500) {
                return
            }
            Thread.sleep(20)
        }
    }

    /**
     * The status bar and the gesture bar, as a phone and a tablet have them, so edge-to-edge
     * screens sit where they do on the device: 24 dp each.
     */
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
        // The compose view the rule set the content in, which reads the insets it is handed.
        val content = activity.findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
        ViewCompat.dispatchApplyWindowInsets(content, insets)
    }

    private companion object {
        /** The phone boards: 412 × 915 dp at 420 dpi, as the emulator's phone. */
        const val PHONE = "w412dp-h915dp-port-420dpi"

        /** The tablet boards: 1280 × 800 dp, landscape, at 320 dpi. */
        const val TABLET = "w1280dp-h800dp-land-xhdpi"

        const val WATCHING = "watching"
        const val FRESH = "fresh"

        /** The zone the shots read their days and times in, wherever they run. */
        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        /**
         * [event] as the engine says it with no source set up: no providers, and every table from
         * AniList's data alone. The fixtures' source lists the same numbers, titles and stills as
         * AniList's listings, so its tables stand in for AniList's, marked so.
         */
        fun withoutSources(event: Event): Event = when (event) {
            is Event.Providers -> Event.Providers(emptyList())
            is Event.Episodes -> Event.Episodes(event.id, event.rows.map { it.copy(metadataOnly = true) })
            else -> event
        }

        /** What a screenshot replays: not the jobs, whose progress line would show half-run. */
        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
