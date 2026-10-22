package tv.anistream.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
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
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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
import tv.anistream.store.ContinueItem
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.FixtureArt
import tv.anistream.testkit.Scene

/**
 * Where each app link lands, drawn as BoardShots draws the boards: the app's frame on a recorded
 * scene, opened on Continue, then handed a link as a launcher shortcut or a notification hands it
 * to the activity. Each checks the screen it lands on and captures it to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class LinkShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val links = Channel<AppLink>(Channel.BUFFERED)
    private val played = mutableListOf<Pair<AnilistId, String>>()
    private lateinit var store: AppStore

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
    fun search_opens_search_with_its_field_open() {
        follow("link-search", AppLink.Search)
        // The full-screen field is up: its placeholder shows twice, the bar's and the expanded one's.
        assertTrue(compose.onAllNodesWithText("Search titles").fetchSemanticsNodes().size >= 2)
    }

    @Test
    fun downloads_opens_downloads_over_library() {
        follow("link-downloads", AppLink.Downloads)
        compose.onAllNodesWithText("Downloads").fetchSemanticsNodes().let { assertTrue(it.isNotEmpty()) }
        compose.activity.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        // Back from Downloads is Library, as from Library's own row.
        assertTrue(compose.onAllNodesWithText("Library").fetchSemanticsNodes().isNotEmpty())
    }

    @Test
    fun continue_plays_what_continue_would_over_continue() {
        follow("link-continue", AppLink.Continue(CONTINUED))
        val entry = requireNotNull(store.titles[CONTINUED])
        // Straight to the player, over Continue: no title page in between.
        assertTrue(compose.onAllNodesWithText("In progress").fetchSemanticsNodes().isNotEmpty())
        assertEquals(listOf(CONTINUED to ContinueItem.of(entry).episode.toString()), played)
    }

    @Test
    fun play_opens_the_title_and_plays_its_episode() {
        follow("link-play", AppLink.Play(CONTINUED, "3"))
        assertEquals(listOf(CONTINUED to "3"), played)
    }

    /** The watching scene drawn on Continue, [link] followed, and the screen it lands on captured. */
    private fun follow(name: String, link: AppLink) {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val events = recorded.events.filter(::replayed)
        store.apply(events)

        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            AnistreamTheme {
                AppRoot(
                    store,
                    actions = AppActions(play = { id, episode -> played += id to episode }),
                    links = links.receiveAsFlow(),
                )
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        store.apply(events)
        links.trySend(link)
        compose.waitForIdle()
        store.apply(events)
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-$name.png")
    }

    /** The status bar and the gesture bar, 24 dp each, as BoardShots sets them. */
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
        /** A title under way in the watching scene. */
        const val CONTINUED: AnilistId = 900001u

        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
