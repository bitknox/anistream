package tv.anistream.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import tv.anistream.design.AnistreamTheme
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerUi
import tv.anistream.engine.model.SkipKind
import tv.anistream.player.ui.PlayerScreen
import tv.anistream.store.AppStore
import tv.anistream.testkit.FixtureArt
import tv.anistream.ui.LocalAppStore

/**
 * The player's boards, drawn on the JVM at the board's size, 915 × 412 dp in landscape: the model
 * on a scripted mpv, put where the board has it, and the title's wide art standing in for mpv's
 * picture, which only a device draws. mpv draws the subtitles too, so the shots have none.
 *
 * `./gradlew :player:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PlayerShots.PHONE_LANDSCAPE)
class PlayerShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @After
    fun tearDown() = scope.cancel()

    /** AndroidPlayer: the controls up in the opening, at 2:31 of 24:02. */
    @Test
    fun phone_player() = shoot("phone-player") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(Event.PlayerUi(1u, PlayerUi.Skip(SkipKind.OPENING, 220.0)))
    }

    /** AndroidPlayerUpNext: the ending under way at 21:33, Up Next two seconds into its countdown, the controls hidden. */
    @Test
    fun phone_player_up_next() = shoot("phone-player-up-next") { model, mpv ->
        model.apply(
            Event.PlayerUi(
                1u,
                PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", "${FixtureArt.HOST}ep/900001/8.jpg"), true, 1291.0, 10u, 3u),
            ),
        )
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1293.0)))
        model.hideControls()
    }

    /** The starting screen (T2): the banner art under the dim, the indicator and its line, before the first frame. */
    @Test
    fun phone_player_starting() = shoot("phone-player-starting", picture = false) { model, _ ->
        model.apply(
            Event.PlayerUi(
                1u,
                PlayerUi.Starting(900_001u, TITLE, "7", EPISODE, "${FixtureArt.HOST}cover/900001-xl.jpg", "${FixtureArt.HOST}banner/900001.jpg"),
            ),
        )
    }

    private fun shoot(name: String, picture: Boolean = true, at: (PlayerModel, ScriptedMpv) -> Unit) {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        var mpv: ScriptedMpv? = null
        // The clock held, so the controls stay up for the shot as after a tap.
        val model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { 0L }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", EPISODE, TITLE, Translation.SUB)))
        }
        compose.waitForIdle()
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            AnistreamTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    PlayerScreen(model, video = { _, modifier, _ ->
                        Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
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
            if (picture) m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            model.poke()
            at(model, m)
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        awaitArt(store)
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /** Wait until the art the screen asked for has landed. */
    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 10_000
        while (store.artwork.hasPending && System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
    }

    companion object {
        /** The player boards: 915 × 412 dp, landscape, at 420 dpi, as the emulator's phone turned. */
        const val PHONE_LANDSCAPE = "w915dp-h412dp-land-420dpi"

        const val TITLE = "Hoshikuzu no Tabibito"
        const val EPISODE = "The Lantern Road"

        /** The board's episode: the opening 2:10 to 3:40, the ending 21:31 to 23:01. */
        const val CHAPTERS = """[{"title":"Prologue","time":0},{"title":"Opening","time":130},{"title":"Part A","time":220},""" +
            """{"title":"Ending","time":1291},{"title":"Preview","time":1381}]"""
    }
}
