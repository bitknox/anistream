package tv.anistream.store

import java.io.File
import java.nio.file.Files
import java.time.Instant
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
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.testkit.Scene

/**
 * Records the testing engine's scenes for the screenshot tests: every event the engine sends while
 * the store asks for what each screen shows, written as JSON into the test kit, which the tests
 * replay into a store of their own with no engine.
 *
 * Runs only when asked, by android/scripts/scenes.sh, which sets `anistream.scenes.out`.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class SceneRecorder {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("anistream-scenes").toFile()
    private val out = System.getProperty("anistream.scenes.out")?.let(::File)

    @Before
    fun setUp() {
        assumeTrue("android/scripts/scenes.sh records the scenes", out != null)
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

    @Test
    fun record_the_scenes() {
        for (scene in listOf(FixtureScene.FRESH, FixtureScene.WATCHING)) {
            Scene.write(record(scene), out!!)
        }
    }

    private fun record(scene: FixtureScene): Scene = runBlocking {
        val events = mutableListOf<Event>()
        val store = withContext(main) { AppStore(scope) }
        val home = File(root, scene.name.lowercase())
        val recordedAt = Instant.now()
        withContext(main) {
            store.onApplied = { batch -> events += batch.filter(::kept) }
            store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(home)), AppHost(MemorySecrets()))
            // What every screen asks for: the lists, the library's segments, the search the
            // shots run, and each fixture title's page, episodes and settings sheet.
            store.loadContinue()
            store.loadSeasonal()
            store.loadCalendar()
            store.loadDownloads()
            store.search("hoshi")
            for (segment in LibraryModel.order) store.loadLibrary(segment)
            for (id in FIXTURE_IDS) {
                store.loadDetail(id)
                store.loadEpisodes(id)
                store.loadShowSettings(id)
            }
        }
        withTimeout(60_000) {
            store.phase.first { it == AppStore.Phase.Running }
            check(store.settle(quietMillis = 1_500, timeoutMillis = 55_000)) { "the ${scene.name} scene did not settle" }
        }
        withContext(main) { store.shutdown() }
        withTimeout(10_000) { store.phase.first { it == AppStore.Phase.Stopped } }
        Scene(scene.name.lowercase(), recordedAt, events.toList())
    }

    companion object {
        /** The test kit's titles, 900001 to 900006. */
        val FIXTURE_IDS = (900001u..900006u).toList()

        /**
         * What a replay keeps: everything but art, which the test kit's files answer, and the
         * player's, which no screenshot of these screens shows.
         */
        private fun kept(event: Event): Boolean = when (event) {
            is Event.ArtworkReady, is Event.ImageFailed -> false
            is Event.PlayerOps, is Event.PlayerUi, is Event.PlaybackStaged, is Event.PlaybackEnded, Event.PlaybackKept -> false
            else -> true
        }
    }
}
