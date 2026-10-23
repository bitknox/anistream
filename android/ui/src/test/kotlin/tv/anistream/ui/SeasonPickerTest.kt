package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import java.time.Clock
import java.time.ZoneId
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
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.FixtureArt
import tv.anistream.testkit.Scene

/**
 * What a long franchise's title page asks AniList for: its own page and the seasons either side
 * of the one on screen, and the next as the viewer moves along the picker, never the whole
 * franchise at once. The page is drawn on the JVM over a store with no engine, so what it asks
 * for waits there to be counted.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class SeasonPickerTest {
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

    /** [season]'s page, as the watching scene recorded it, alone in a store with no engine, opened. */
    private fun open(season: AnilistId): AppStore {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        store.apply(recorded.events.filter { it is Event.Detail && it.v1.id == season })
        compose.setContent { AnistreamTheme { AppRoot(store, startTitle = season) } }
        settle()
        return store
    }

    private fun settle() {
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
    }

    /** The franchise's pages asked for, in the order they were. */
    private fun AppStore.asked(): List<AnilistId> =
        waiting.filterIsInstance<Command.LoadDetail>().map { it.v1 }.filter { it in LONG_FRANCHISE }

    @Test
    fun a_long_franchises_page_asks_for_the_seasons_beside_it_alone() {
        assertEquals(listOf(920008u, 920010u), open(920009u).asked())
    }

    @Test
    fun moving_along_the_picker_asks_for_the_next_season_in_turn() {
        val store = open(920001u)
        assertEquals(listOf(920002u), store.asked())
        compose.onNodeWithText("Season 2").performClick()
        settle()
        assertEquals("the second season's page is not asked for again", listOf(920002u, 920003u), store.asked())
    }

    @Test
    fun a_jump_along_the_picker_asks_for_the_season_landed_on_and_those_beside_it() {
        val store = open(920001u)
        compose.onNodeWithText("Season 9").performScrollTo().performClick()
        settle()
        assertEquals(listOf(920002u, 920009u, 920008u, 920010u), store.asked())
    }

    private companion object {
        /** Cloudcutter Airship's twelve seasons. */
        val LONG_FRANCHISE = (920001u..920012u).toSet()
    }
}
