package tv.anistream.store

import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.newSingleThreadContext
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.WidgetData

/**
 * The new-episode check on the real testing engine, through JNA, as the periodic work runs it: on
 * the app's engine when it runs, and on a quiet one of its own when it does not, with the widgets'
 * record made after it. Only in the fixtures flavour, whose engine has the scenes and their clock.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class EpisodeCheckTest {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("anistream-check").toFile()

    @Before
    fun setUp() {
        assumeTrue("the app flavour's engine has no scenes", HAS_TESTING_ENGINE)
        val home = System.getenv("HOME").orEmpty()
        check(home.isNotEmpty() && home != System.getProperty("user.home")) { "HOME is the user's own: $home" }
        Dispatchers.setMain(main)
    }

    @After
    fun tearDown() {
        scope.cancel()
        Dispatchers.resetMain()
        main.close()
        root.deleteRecursively()
    }

    private fun watching(dir: String) = EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.WATCHING), EngineHome.override(root.resolve(dir)))

    /**
     * Let [seconds] pass for the engine and its fake AniList: the testing build's own call, which
     * the app flavour's bindings do not have, so it is found by name.
     */
    private fun FfiEngine.advance(seconds: Long) {
        javaClass.getMethod("testingAdvanceClock", Long::class.javaPrimitiveType).invoke(this, seconds)
    }

    @Test
    fun an_episode_that_airs_after_the_first_check_is_said_once_with_the_widgets_record_after_it() = runBlocking {
        val engine = withContext(Dispatchers.IO) { watching("app").start(AppHost(MemorySecrets())) }
        val store = withContext(main) { AppStore(scope).also { it.attach(engine) } }
        val records = mutableListOf<WidgetData>()
        val check = EngineEpisodeCheck(
            BackgroundEngine(running = { store }, launch = { error("the app's engine runs") }, host = { error("the app's engine runs") }),
            widgets = { _, data -> records += data },
        )
        withTimeout(60_000) {
            withContext(main) { store.setSwitch(SettingKey.EPISODE_NOTIFICATIONS, true) }
            store.settings.view.first { it != null && store.settings.isOn(SettingKey.EPISODE_NOTIFICATIONS, it) }

            // The first check notes where things stand and says nothing, so no backlog fires.
            assertEquals(emptyList<NewEpisode>(), check.newEpisodes())
            assertEquals(1, records.size)

            // Past The Library Above the Clouds' episode 6, which airs 2 d 3 h 15 m after the scene's moment.
            engine.advance(2 * DAY + 4 * HOUR)
            val said = check.newEpisodes()
            assertEquals(listOf(Triple(900002u, "The Library Above the Clouds", 6u)), said.map { Triple(it.id, it.title, it.episode) })
            assertTrue("the fixture's cover, for the notification", said.single().coverUrl?.startsWith("http://127.0.0.1:") == true)

            // The widgets' record after each check, of the shape the widgets read.
            assertEquals(2, records.size)
            assertEquals(2u, records.last().version)
            assertTrue(records.last().continueWatching.isNotEmpty())

            // Said once.
            engine.advance(10 * MINUTE)
            assertEquals(emptyList<NewEpisode>(), check.newEpisodes())
        }
        withContext(main) { store.shutdown() }
    }

    @Test
    fun without_the_app_the_check_runs_on_a_quiet_engine_of_its_own_and_stops_it() = runBlocking {
        val records = mutableListOf<WidgetData>()
        val background = BackgroundEngine(running = { null }, launch = { watching("background") }, host = { AppHost(MemorySecrets()) })
        val check = EngineEpisodeCheck(background, widgets = { _, data -> records += data })
        withTimeout(60_000) {
            // Off by default: nothing to say, and the widgets' record made all the same.
            assertEquals(emptyList<NewEpisode>(), check.newEpisodes())
        }
        assertEquals(1, records.size)
        assertTrue("its engine stopped with the check", !withContext(main) { background.busy })
    }

    private companion object {
        const val MINUTE = 60L
        const val HOUR = 60 * MINUTE
        const val DAY = 24 * HOUR
    }
}
