package tv.anistream.store

import java.io.File
import java.nio.file.Files
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
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test

/**
 * The store on the real engine, this Mac's testing build of it, through JNA: the fake AniList and
 * the fixture titles, nothing leaving the machine. Only in the fixtures flavour, whose engine has
 * `startTesting`.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class EngineTest {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("anistream-engine").toFile()

    @Before
    fun setUp() {
        assumeTrue("the app flavour's engine has no testing start", HAS_TESTING_ENGINE)
        // The Gradle task sets HOME to a directory of the test's own; never the user's.
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
    fun the_testing_engine_answers_seasonal_with_the_fixture_titles() = runBlocking {
        val store = withContext(main) { AppStore(scope) }
        val launch = EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.FRESH), EngineHome.override(root))
        withContext(main) {
            // Asked before the engine is there: it waits, and goes once it is.
            store.loadSeasonal()
            store.loadContinue()
            store.start(launch, AppHost(MemorySecrets()))
        }
        withTimeout(30_000) {
            val seasonal = store.seasonal.state.first { it is ListState.Loaded || it is ListState.Failed }
            assertEquals(ListState.Loaded(listOf(900002u, 900006u)), seasonal)
            assertEquals(ListState.Empty, store.continuing.list.state.first { it != ListState.Loading && it != ListState.Idle })
            assertTrue(withContext(main) { store.titles[900002u] } != null)
            while (!withContext(main) { store.isSettled }) delay(50)
        }
        withContext(main) { store.shutdown() }
        assertEquals(AppStore.Phase.Stopped, withTimeout(10_000) { store.phase.first { it == AppStore.Phase.Stopped } })
    }
}
