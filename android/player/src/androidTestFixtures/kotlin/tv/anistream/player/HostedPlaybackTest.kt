// The hosted player protocol end to end, in the emulator: the testing engine on the Playing scene,
// the player model on a real mpv (no surface: vo=null, as a phone in the background has it), and
// the generated episode from android/scripts/player-media.sh.

package tv.anistream.player

import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Ending
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.PlayerEnd
import tv.anistream.engine.model.PlayerReport
import tv.anistream.engine.model.SkipKind
import tv.anistream.store.AppHost
import tv.anistream.store.AppStore
import tv.anistream.store.EngineHome
import tv.anistream.store.EngineLaunch
import tv.anistream.store.FixtureScene
import tv.anistream.store.MemorySecrets
import tv.anistream.store.fixtureMedia

@RunWith(AndroidJUnit4::class)
class HostedPlaybackTest {
    private val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var root: File
    private lateinit var scope: CoroutineScope
    private lateinit var store: AppStore
    private lateinit var model: PlayerModel
    private val events = mutableListOf<Event>()
    private val sent = mutableListOf<Command>()

    /** Hoshikuzu no Tabibito, the test kit's finished title; the scene has watched 1 to 6. */
    private val finished = 900_001u

    @Before
    fun setUp() {
        root = File(context.cacheDir, "hosted-test-${System.nanoTime()}").apply { mkdirs() }
    }

    @After
    fun tearDown() = runBlocking {
        withContext(Dispatchers.Main) {
            if (::model.isInitialized && model.isPresented) model.viewerClosed()
        }
        // Off the main thread: the engine winds down while its last events are delivered there.
        if (::store.isInitialized) withContext(Dispatchers.IO) { store.shutdown() }
        if (::scope.isInitialized) scope.cancel()
        root.deleteRecursively()
        Unit
    }

    /** The episode, copied out of the test APK's assets. */
    private fun episode(): File {
        val file = File(context.cacheDir, "episode.mkv")
        if (!file.exists()) {
            val names = InstrumentationRegistry.getInstrumentation().context.assets.list("").orEmpty()
            check("episode.mkv" in names) { "no episode in the test APK: run android/scripts/player-media.sh" }
            InstrumentationRegistry.getInstrumentation().context.assets.open("episode.mkv").use { input ->
                file.outputStream().use(input::copyTo)
            }
        }
        return file
    }

    /** The testing engine on the Playing scene streaming [media], with [config] written first when given. */
    private fun start(media: String, config: String? = null, hwdec: String = "no") = runBlocking {
        val home = EngineHome.override(root)
        config?.let {
            home.config.mkdirs()
            File(home.config, "config.toml").writeText(it)
        }
        fixtureMedia = media
        withContext(Dispatchers.Main) {
            scope = MainScope()
            store = AppStore(scope)
            model = PlayerModel(
                send = { command ->
                    sent += command
                    store.send(command)
                },
                backend = { MpvBackend(context, extra = listOf("ao" to "null")) },
                scope = scope,
                hwdec = hwdec,
            )
            store.onPlayback = model::apply
            store.onApplied = { events += it }
            store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.PLAYING), home), AppHost(MemorySecrets()))
        }
        until("the engine", 60_000) { store.phase.value == AppStore.Phase.Running }
    }

    private fun until(what: String, timeoutMs: Long = 20_000, condition: () -> Boolean) = runBlocking {
        android.util.Log.i("HostedPlaybackTest", "waiting for $what")
        val deadline = System.currentTimeMillis() + timeoutMs
        while (!withContext(Dispatchers.Main) { condition() }) {
            check(System.currentTimeMillis() < deadline) { "timed out waiting for $what" }
            delay(20)
        }
    }

    private fun <T> main(block: () -> T): T = runBlocking { withContext(Dispatchers.Main) { block() } }

    private fun play(episode: String) = main { store.send(Command.Play(finished, episode)) }

    /** The reports sent, and the playbacks' endings: read on the main thread, inside [main] or [until]. */
    private val reports: List<PlayerReport> get() = sent.filterIsInstance<Command.PlayerReport>().map { it.report }

    private val endings: List<Event.PlaybackEnded> get() = events.filterIsInstance<Event.PlaybackEnded>()

    @Test
    fun anEpisodeResumesOffersItsSkipsRollsOnThroughUpNextAndCloses() {
        start(episode().path)

        // Start: the starting screen, then the first frame over it.
        play("7")
        until("the player") { model.isPresented }
        assertEquals("7", main { model.staged?.episode })
        until("the first frame", 30_000) { model.hasPicture && model.starting == null }
        until("the episode on screen") { model.episode?.number == "7" }
        until("the duration") { model.duration != null }
        assertEquals(60.0, main { model.duration!! }, 0.5)
        until("the chapters") { model.chapters.size == 5 }
        assertEquals(listOf("Prologue", "Opening", "Part A", "Ending", "Preview"), main { model.chapters.map { it.title } })
        until("the tracks") { model.audioTracks.size == 2 && model.subtitleTracks.size == 2 }

        // The opening's skip, offered and not taken by itself: the scene offers skips.
        main { model.seekTo(6.0) }
        until("the skip offered") { model.skip?.kind == SkipKind.OPENING }
        assertEquals(PlayerModel.Corner.Skip, main { model.corner })
        main { model.takeSkip() }
        until("past the opening") { model.playhead >= 19.5 }

        // Closed part-way: the engine keeps the place.
        main { model.seekTo(34.0) }
        // A moment of playing there, as a viewer would before closing.
        until("playing on from 34 s") { model.playhead >= 36.0 }
        main { model.viewerClosed() }
        assertEquals(listOf(PlayerReport.Ended(PlayerEnd.QUIT), PlayerReport.Closed), main { reports.takeLast(2) })
        until("the player down") { !model.isPresented }
        until("the playback over") { endings.isNotEmpty() }
        assertEquals(Ending.Closed, main { endings.last().ending })

        // Played again, it resumes there.
        play("7")
        until("the player again") { model.isPresented }
        until("the resume note", 30_000) { model.resume != null }
        assertEquals(36.0, main { model.resume!!.position }, 2.0)
        until("the first frame again", 30_000) { model.hasPicture }
        assertTrue(main { model.playhead } >= 34.0)
        val first = main { model.playback!! }

        // Up Next from the ending, counting down; Play now plays the next in the same player.
        main { model.seekTo(43.0) }
        until("Up Next") { model.upNextShown }
        assertEquals("8", main { model.upNext?.next?.number })
        assertEquals(42.0, main { model.upNext?.endingAt!! }, 0.5)
        assertTrue(main { model.countdown!!.first } in 7..10)
        main { model.playNext() }
        until("the next episode", 30_000) {
            (model.playback ?: 0u) > first && model.hasPicture && model.episode?.number == "8"
        }
        assertTrue("the same player", main { model.isPresented })
        assertFalse(main { model.stillWatching })

        // Closed by the viewer: quit, then closed, and the engine says the playback closed.
        val endedBefore = main { endings.size }
        main { model.viewerClosed() }
        until("the playback over") { endings.size > endedBefore }
        assertEquals(Ending.Closed, main { endings.last().ending })
        assertFalse(main { model.isPresented })
    }

    @Test
    fun anAutomaticSkipCanBeUndone() {
        start(episode().path, config = "[playback]\nskip_opening = true\n")
        play("7")
        until("the first frame", 30_000) { model.hasPicture }
        // The opening, 5 s to 20 s: skipped by itself as the playhead reaches it.
        until("the skip taken", 20_000) { model.undo != null }
        assertEquals(SkipKind.OPENING, main { model.undo!!.kind })
        until("past the opening") { model.playhead >= 19.5 }
        val undoTo = main { model.undo!!.undoTo }
        assertTrue(undoTo in 4.0..7.0)
        main { model.undoSkip() }
        until("back where it skipped from") { model.playhead in (undoTo - 1)..(undoTo + 4) }
        main { model.viewerClosed() }
        until("the player down") { !model.isPresented }
    }

    @Test
    fun aStreamThatStopsSendingOffersAnotherAndCanBeWaitedFor() {
        val file = episode()
        // The server sends the first third of the file, then holds.
        val server = StreamServer(file, holdAfter = file.length() / 3)
        try {
            start("http://127.0.0.1:${server.port}/episode.mkv")
            play("7")
            until("the first frame", 30_000) { model.hasPicture }
            until("waiting for data", 60_000) { model.bufferingSince != null }
            // The engine offers another stream once it has waited 8 s.
            until("the stall prompt", 20_000) { model.stall != null }
            assertEquals(PlayerModel.Corner.Stall, main { model.corner })
            main { model.keepWaiting() }
            assertTrue(main { model.stall == null })
            server.release()
            until("playing again", 30_000) { model.bufferingSince == null && model.playhead > 25 }
            assertTrue("no prompt while it waits on", main { model.stall == null })
            main { model.viewerClosed() }
            until("the player down") { !model.isPresented }
        } finally {
            server.close()
        }
    }

    /**
     * The emulator's MediaCodec stalls under FFmpeg (the spike's AS5), which takes mpv's core with
     * it: the watchdog has mpv decode in software, or opens a new one, and the episode plays.
     */
    @Test
    fun aHardwareDecoderThatStallsFallsBackToSoftware() {
        start(episode().path, hwdec = "mediacodec-copy")
        play("7")
        until("a picture, in software if need be", 60_000) { model.hasPicture && model.playhead > 2 }
        android.util.Log.i("HostedPlaybackTest", "decoding with ${main { model.decoding }}, ${main { model.surfaceGeneration }} new surfaces")
        main { model.viewerClosed() }
        until("the player down") { !model.isPresented }
    }
}
