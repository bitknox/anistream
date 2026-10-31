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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test

/**
 * Reset settings on the start-failed screen, on the real testing engine: an unreadable settings
 * file fails the start and says so, the reset keeps it aside and the engine starts on the
 * defaults, and a failed start that is not about the settings offers no reset.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class ResetSettingsTest {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("anistream-reset").toFile()

    @Before
    fun setUp() {
        assumeTrue("the app flavour's engine has no testing start", HAS_TESTING_ENGINE)
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
    fun a_damaged_settings_file_is_kept_aside_and_the_engine_starts_on_the_defaults() = runBlocking {
        val config = root.resolve("config").apply { mkdirs() }.resolve("config.toml")
        config.writeText("this is [not toml")
        val store = withContext(main) { AppStore(scope) }
        val launch = EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.FRESH), EngineHome.override(root))
        withContext(main) { store.start(launch, AppHost(MemorySecrets())) }
        val failed = withTimeout(30_000) { store.phase.first { it is AppStore.Phase.Failed } } as AppStore.Phase.Failed
        assertTrue("the failure names the settings", failed.settingsUnusable)

        withContext(main) { store.resetSettings() }
        withTimeout(30_000) { store.phase.first { it == AppStore.Phase.Running } }

        val kept = root.resolve("config").listFiles().orEmpty().filter { it.name.startsWith("config.damaged-") }
        assertEquals(1, kept.size)
        assertEquals("this is [not toml", kept.single().readText())
        assertFalse("a bad file does not stay as the settings", config.exists() && config.readText() == "this is [not toml")
        val notice = withTimeout(10_000) { store.notices.current.first { it != null } }
        assertNotNull(notice)
        withContext(main) { store.shutdown() }
    }

    @Test
    fun a_reset_does_nothing_unless_the_settings_were_what_failed() = runBlocking {
        val store = withContext(main) { AppStore(scope) }
        withContext(main) { store.resetSettings() }
        assertEquals(AppStore.Phase.Starting, store.phase.value)
    }
}
