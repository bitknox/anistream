package tv.anistream.tv

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerUi
import tv.anistream.engine.model.SkipKind
import tv.anistream.player.MpvEvent
import tv.anistream.player.MpvNative
import tv.anistream.player.MpvValue
import tv.anistream.player.PlayerModel
import tv.anistream.player.ScriptedMpv
import tv.anistream.store.AppClock
import tv.anistream.store.AppStore
import tv.anistream.store.HAS_TESTING_ENGINE
import tv.anistream.testkit.FixtureArt
import tv.anistream.testkit.Scene
import tv.anistream.tv.player.CHROME_HIDE
import tv.anistream.tv.player.TvPlayerScreen
import tv.anistream.tv.remote.PairPage
import tv.anistream.tv.remote.PairRequest
import tv.anistream.tv.remote.PairRequestDialog
import tv.anistream.tv.shell.Destination
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.shell.TvRoot
import tv.anistream.tv.ui.TV_ART_BUDGET
import tv.anistream.tv.ui.TvArtDecoder
import tv.anistream.tv.ui.TvTheme

/**
 * The TV's screens, drawn on the JVM at 1920 × 1080 from a scene the testing engine recorded, and
 * the D-pad's focus rules walked through them: the first focus, the rail, Back to the opener.
 *
 * `./gradlew :tv:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvShots {
    val compose = createAndroidComposeRule<ComponentActivity>()

    /**
     * The app's manifest has no plain activity for the rule to start, as a library's test manifest
     * would: it is declared to Robolectric's package manager first.
     */
    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() = assumeTrue(HAS_TESTING_ENGINE)

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    fun continue_home() = shoot("tv-continue") {
        assertTrue("the hero's play button takes the first focus", focusedText().startsWith("Resume") || focusedText().startsWith("Play"))
    }

    @Test
    fun rail_opens_on_left() = shoot("tv-rail") {
        press(Key.DirectionLeft)
        assertEquals("Continue", focusedText())
    }

    @Test
    fun rail_right_returns_to_the_content() = shoot(null) {
        val first = focusedText()
        press(Key.DirectionLeft)
        press(Key.DirectionRight)
        assertEquals(first, focusedText())
    }

    @Test
    fun search() = shoot("tv-search", query = "hoshi")

    @Test
    fun title() = shoot("tv-title", title = 900001u)

    @Test
    fun calendar() = shoot("tv-calendar", start = Destination.Calendar)

    @Test
    fun seasonal() = shoot("tv-seasonal", start = Destination.Seasonal)

    @Test
    fun library() = shoot("tv-library", start = Destination.Library)

    @Test
    fun settings() = shoot("tv-settings", start = Destination.Settings)

    /** Back from a title page puts the focus back on the card that opened it. */
    @Test
    fun back_returns_to_the_opener() = shoot(null) {
        press(Key.DirectionDown)
        val opener = focusedText()
        press(Key.DirectionCenter)
        settle()
        assertTrue("a title page is open", opener != focusedText())
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        settle()
        assertEquals(opener, focusedText())
    }

    /** The player in the opening at 2:31 of 24:02, the chrome up as after a key. */
    @Test
    fun player() = playerShot("tv-player") {}

    /** The chrome gone again, the skip offer holding Center in the bottom right. */
    @Test
    fun player_skip() = playerShot("tv-player-skip") { time = CHROME_HIDE + 1_000 }

    /** Settings › Remotes › Pair a phone: the QR code, the code beside it, and Done. */
    @Test
    fun pair() {
        compose.setContent {
            TvTheme {
                PairPage(name = "Den", code = "042917", link = PAIR_LINK, address = "192.168.1.20:47811", onClose = {})
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-pair.png")
    }

    /** A phone asked to pair: its code over the browse UI, Cancel focused. */
    @Test
    fun pair_request() {
        compose.setContent {
            TvTheme { PairRequestDialog(PairRequest("042917", "Mika's phone"), onCancel = {}) }
        }
        compose.waitForIdle()
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-pair-request.png")
    }

    /** Up Next as the ending starts: its card bottom right, Play now focused. */
    @Test
    fun player_up_next() = playerShot("tv-player-up-next") { model ->
        time = CHROME_HIDE + 1_000
        compose.runOnUiThread {
            model.apply(Event.PlayerUi(1u, PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", null), true, 148.0, 10u, 3u)))
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals("Play now", focusedText())
    }

    /** The Info panel: the episode, then how it plays. */
    @Test
    fun player_info() = playerShot("tv-player-info") {
        // The chrome is up with focus on Play: Right along the buttons to Info.
        repeat(6) { if (focusedText() != "Info") press(Key.DirectionRight) }
        assertEquals("Info", focusedText())
        press(Key.DirectionCenter)
    }

    /** The model's clock, held where a shot puts it. */
    private var time = 0L

    private fun playerShot(name: String, then: (PlayerModel) -> Unit) {
        AppClock.clock = Clock.fixed(Instant.parse("2026-10-20T19:02:00Z"), ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        var mpv: ScriptedMpv? = null
        val model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", EPISODE, TITLE, Translation.SUB)))
        }
        compose.waitForIdle()
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            TvTheme {
                // As a preview draws it: the spinner, which turns for ever, as a still.
                CompositionLocalProvider(LocalInspectionMode provides true) {
                    TvRoot(store, model, TvNavigator(), play = { _, _ -> }, playerScreen = {
                        TvPlayerScreen(it, video = { modifier ->
                            Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
                        })
                    })
                }
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
            m.emit(MpvEvent.property("duration", MpvValue.Real(1442.0)))
            m.emit(MpvEvent.property("pause", MpvValue.Flag(false)))
            m.emit(MpvEvent.property("video-params", MpvValue.parse("""{"w":1920,"h":1080,"dw":1920,"dh":1080}""")!!))
            m.emit(MpvEvent.property("chapter-list", MpvValue.parse(CHAPTERS)!!))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            m.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
            model.apply(Event.PlayerUi(1u, PlayerUi.Skip(SkipKind.OPENING, 220.0)))
            model.poke()
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        then(model)
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        awaitArt(store)
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /**
     * The app's root on [start] (or a title page, or Search with [query]), fed the scene's
     * events, then [then], then the capture when [name] is given.
     */
    private fun shoot(
        name: String?,
        start: Destination = Destination.Continue,
        title: AnilistId? = null,
        query: String? = null,
        then: (() -> Unit)? = null,
    ) {
        val recorded = Scene.named(WATCHING)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val events = recorded.events.filter(::replayed)
        store.apply(events)
        val player = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { 0L }, hwdec = "no")
        val nav = TvNavigator(if (query != null) Destination.Search else start).apply {
            query?.let { searchText = it }
            title?.let(::openTitle)
        }
        compose.setContent {
            TvTheme { TvRoot(store, player, nav, play = { _, _ -> }) }
        }
        compose.waitForIdle()
        store.apply(events)
        settle()
        // Once more for what a screen asks for after a wait, such as Search after its pause.
        store.apply(events)
        awaitArt(store)
        settle()
        then?.let {
            it()
            settle()
        }
        if (name != null) compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    private fun settle() {
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
    }

    private fun press(key: Key) {
        compose.onRoot().performKeyInput { pressKey(key) }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
    }

    /** The focused control's text, its descendants' merged. */
    private fun focusedText(): String {
        val node: SemanticsNodeInteraction = compose.onNode(isFocused())
        val text = node.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Text).orEmpty()
        return text.joinToString(" ") { it.text }
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

    companion object {
        /** A 1080p TV: 960 × 540 dp at 320 dpi. */
        const val TV = "w960dp-h540dp-land-television-xhdpi"

        const val WATCHING = "watching"

        /** A pairing link as the engine writes one, with a made-up key. */
        const val PAIR_LINK = "anistream-remote://pair?addr=192.168.1.20:47811&fp=" +
            "3f9a0c4e7b1d28e5a6c3f0b9d4e7a1c2b5f8e3d6a9c0b7e4f1a2d5c8b3e6f9a0&code=042917"

        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        const val TITLE = "Hoshikuzu no Tabibito"
        const val EPISODE = "The Lantern Road"

        /** The opening 2:10 to 3:40, the ending 21:31 to 23:01. */
        const val CHAPTERS = """[{"title":"Prologue","time":0},{"title":"Opening","time":130},{"title":"Part A","time":220},""" +
            """{"title":"Ending","time":1291},{"title":"Preview","time":1381}]"""

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
