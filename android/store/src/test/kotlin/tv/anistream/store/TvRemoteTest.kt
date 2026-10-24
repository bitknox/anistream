package tv.anistream.store

import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
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
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.ffi.FfiLinkPhase
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvRowKind

/**
 * Play on TV on the real engine against the fixtures' two fake TVs on the loopback interface: the
 * picker finds them, a code that is wrong says so, the QR link pairs at once, and the card follows
 * what the TV plays.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class TvRemoteTest {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("anistream-tv").toFile()
    private lateinit var store: AppStore
    private lateinit var browser: FfiTvBrowser

    @Before
    fun setUp(): Unit = runBlocking {
        assumeTrue("the app flavour's engine has no fake TVs", HAS_TESTING_ENGINE)
        val home = System.getenv("HOME").orEmpty()
        check(home.isNotEmpty() && home != System.getProperty("user.home")) { "HOME is the user's own: $home" }
        Dispatchers.setMain(main)
        store = withContext(main) { AppStore(scope) }
        withContext(main) {
            store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.FRESH), EngineHome.override(root)), AppHost(MemorySecrets()))
        }
        withTimeout(30_000) { store.phase.first { it == AppStore.Phase.Running } }
        withContext(main) {
            browser = requireNotNull(FixtureTvs.start(requireNotNull(store.engine)))
            assertTrue(store.startTv(browser, TvRemoteModel.DEVICE_NAME, "anistream-android/test"))
        }
    }

    @After
    fun tearDown() {
        if (::store.isInitialized) runBlocking { withContext(main) { store.tv.disconnect(); store.shutdown() } }
        scope.cancel()
        Dispatchers.resetMain()
        main.close()
        root.deleteRecursively()
    }

    private val tv get() = store.tv

    private suspend fun <T> StateFlow<T>.until(check: (T) -> Boolean): T = withTimeout(20_000) { first(check) }

    @Test
    fun the_picker_lists_both_tvs_and_a_wrong_code_says_so(): Unit = runBlocking {
        withContext(main) {
            tv.openPicker()
            tv.startBrowsing()
        }
        val rows = tv.rows.until { it.size == 2 }
        assertEquals(setOf("Living room", "Bedroom"), rows.map { it.name }.toSet())
        assertTrue(rows.all { it.kind == FfiTvRowKind.NEW })
        assertEquals(TvRemoteModel.Sheet.Picker, tv.sheet.value)

        withContext(main) { tv.choose(rows.first { it.name == "Living room" }) }
        val asked = tv.pairing.until { it != null && it.needsCode && !it.working }
        assertEquals("Living room", asked?.tvName)
        withContext(main) { tv.submit("000000") }
        val refused = tv.pairing.until { it != null && !it.working && it.problem != null }
        assertFalse(refused?.problem.isNullOrBlank())
        assertNull(tv.connected.value)

        withContext(main) {
            tv.cancelPairing()
            tv.stopBrowsing()
        }
        assertNull(tv.pairing.value)
    }

    @Test
    fun the_tvs_link_pairs_at_once_and_the_card_follows_the_tv(): Unit = runBlocking {
        val link = requireNotNull(FixtureTvs.openLink(browser, 0))
        withContext(main) { assertTrue(tv.open(link)) }
        val fingerprint = tv.connected.until { it != null }!!
        tv.sheet.until { it == TvRemoteModel.Sheet.Remote }
        assertEquals(listOf("Living room"), tv.paired.until { it.isNotEmpty() }.map { it.name })
        tv.card.until { it?.card?.phase == FfiLinkPhase.CONNECTED }

        withContext(main) { tv.offer(TvPlayRequest(900001u, "Fixture title", "3")) }
        val playing = tv.card.until { it?.card?.now?.let { now -> !now.loading && now.episode == "3" } == true }!!
        assertEquals(900001u, playing.card.now!!.id)
        assertFalse(playing.card.over)
        // Once the TV names the episode, the card shows the TV's own state, not the request's.
        assertNull(tv.starting.until { it == null })
        assertNotNull(withContext(main) { tv.playhead() })

        val paused = playing.card.now!!.paused
        withContext(main) { tv.send(FfiTvCommand.TogglePause) }
        tv.card.until { it?.card?.now?.paused == !paused }
        withContext(main) { tv.send(FfiTvCommand.Volume(TvRemoteModel.VOLUME_STEP)) }
        // A connected link stays as it is when told to try again.
        withContext(main) { tv.retryNow() }
        delay(200)
        assertEquals(FfiLinkPhase.CONNECTED, tv.card.value?.card?.phase)
        assertNull(tv.problem.value)

        withContext(main) {
            tv.forget(fingerprint)
            assertNull(tv.connected.value)
            assertNull(tv.sheet.value)
        }
        tv.paired.until { it.isEmpty() }
    }

    @Test
    fun a_tv_gone_mid_episode_stops_the_playhead_where_it_went(): Unit = runBlocking {
        val link = requireNotNull(FixtureTvs.openLink(browser, 0))
        withContext(main) { assertTrue(tv.open(link)) }
        tv.card.until { it?.card?.phase == FfiLinkPhase.CONNECTED }
        withContext(main) { tv.offer(TvPlayRequest(900001u, "Fixture title", "3")) }
        tv.card.until { it?.card?.now?.let { now -> !now.loading } == true }
        assertTrue(tv.card.value!!.live)

        (browser as FixtureTvBrowser).tvs[0].stop()
        val lost = tv.card.until { it != null && it.card.phase != FfiLinkPhase.CONNECTED }!!
        assertFalse(lost.live)
        assertNotNull(lost.lostAt)
        assertNotNull(lost.card.now)
        val stopped = withContext(main) { tv.playhead() }
        delay(1_200)
        assertEquals(stopped, withContext(main) { tv.playhead() })
    }
}
