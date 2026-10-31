package tv.anistream.store

import java.io.File
import java.nio.file.Files
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
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
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SearchQuery
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
        // Home's world changes what the notices and the widgets' record hold, so its scenes are
        // recorded apart, under their own names, in a run with the variable set.
        val world = System.getenv(HOME_WORLD_VAR) == "1"
        val scenes = if (world) HOME_SCENES else listOf(FixtureScene.FRESH, FixtureScene.WATCHING)
        for (scene in scenes) {
            Scene.write(record(scene, if (world) "home-" else ""), out!!)
        }
    }

    private fun record(scene: FixtureScene, prefix: String): Scene = runBlocking {
        val events = mutableListOf<Event>()
        val store = withContext(main) { AppStore(scope) }
        val home = File(root, scene.name.lowercase())
        val recordedAt = Instant.now()
        withContext(main) {
            store.onApplied = { batch -> events += batch.filter(::kept) }
            store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(home)), AppHost(MemorySecrets()))
            // What every screen asks for: the lists, the library's segments, Search's start and
            // the searches the shots run, and each fixture title's page, episodes and settings
            // sheet.
            store.loadHome()
            store.loadContinue()
            store.loadSeasonal()
            store.loadCalendar()
            store.loadDownloads()
            store.loadSearchStart()
            store.search(SearchQuery(text = "hoshi"))
            for (segment in LibraryModel.order) store.loadLibrary(segment)
            for (id in FIXTURE_IDS) {
                store.loadDetail(id)
                store.loadEpisodes(id)
                store.loadShowSettings(id)
            }
            // The long franchise's pages the shots open, and some of their neighbours': the rest
            // are seasons whose title has not arrived.
            for (id in LONG_FRANCHISE_IDS) {
                store.loadDetail(id)
                store.loadEpisodes(id)
            }
        }
        withTimeout(60_000) {
            store.phase.first { it == AppStore.Phase.Running }
            check(store.settle(quietMillis = 1_500, timeoutMillis = 55_000)) { "the ${scene.name} scene did not settle" }
        }
        // A browse by genre, once the search before it is answered: a newer search takes an
        // older one's place.
        val browse = SearchQuery(genres = listOf("Fantasy"))
        withContext(main) { store.search(browse) }
        withTimeout(60_000) {
            // The list on screen stays as it was until the answer, so settling alone would not wait.
            while (withContext(main) { events.none { it is Event.Content && it.view == ContentFor.Search(browse) } }) delay(50)
            check(store.settle(quietMillis = 1_500, timeoutMillis = 55_000)) { "the ${scene.name} browse did not settle" }
        }
        withContext(main) { store.shutdown() }
        withTimeout(10_000) { store.phase.first { it == AppStore.Phase.Stopped } }
        Scene(prefix + scene.name.lowercase().replace("_", ""), recordedAt, events.toList())
    }

    companion object {
        /** The environment variable that puts Home's world in a scene. */
        const val HOME_WORLD_VAR = "ANISTREAM_FIXTURES_HOME_WORLD"

        /** The scenes Home's shots replay, recorded with the world on. */
        val HOME_SCENES = listOf(
            FixtureScene.WATCHING, FixtureScene.SUGGESTED, FixtureScene.ONE_LEFT, FixtureScene.CONTINUE_FULL,
            FixtureScene.NOT_FOUND, FixtureScene.SLOW, FixtureScene.FAILED, FixtureScene.OFFLINE,
        )

        /** The test kit's titles, 900001 to 900006. */
        val FIXTURE_IDS = (900001u..900006u).toList()

        /**
         * Seasons 1, 2, 8, 9, 10 and 12 of the test kit's long franchise, Cloudcutter Airship, 920001
         * to 920012, which every scene's fake AniList has beside the six.
         */
        val LONG_FRANCHISE_IDS = listOf(920001u, 920002u, 920008u, 920009u, 920010u, 920012u)

        /**
         * What a replay keeps: everything but art, which the test kit's files answer, and the
         * player's, which no screenshot of these screens shows.
         */
        private fun kept(event: Event): Boolean = when (event) {
            is Event.ArtworkReady, is Event.ImageFailed -> false
            is Event.PlayerOps, is Event.PlayerUi, is Event.PlaybackStaged, is Event.PlaybackEnded, Event.PlaybackKept,
            is Event.PlayerStats, is Event.Clipboard -> false
            else -> true
        }
    }
}
