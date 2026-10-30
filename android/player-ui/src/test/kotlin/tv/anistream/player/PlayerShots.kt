package tv.anistream.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
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
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import tv.anistream.engine.model.CommentBlock
import tv.anistream.engine.model.CommentSpan
import tv.anistream.engine.model.CommentView
import tv.anistream.engine.model.DiscussionState
import tv.anistream.engine.model.DiscussionView
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.PostState
import tv.anistream.ui.Destination
import tv.anistream.ui.DiscussionSheet
import tv.anistream.ui.LocalNavigator
import tv.anistream.ui.Navigator
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerUi
import tv.anistream.engine.model.SkipKind
import tv.anistream.engine.model.TimeRange
import tv.anistream.engine.model.TimelineMark
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

    /** The model's clock, held where a shot puts it. */
    private var time = 0L

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

    /** Waiting for data mid-episode, the controls hidden: the buffering line along the video's top edge. */
    @Test
    fun phone_player_buffering() = shoot("phone-player-buffering") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(612.0)))
        mpv.emit(MpvEvent.property("paused-for-cache", MpvValue.Flag(true)))
        time = 1_500
        model.hideControls()
        check(model.showsBuffering) { "the shot is of a buffering player" }
    }

    /** The seek line with the engine's opening and ending marks and what is buffered ahead of the played part. */
    @Test
    fun phone_player_buffered() = shoot("phone-player-buffered") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(Event.PlayerTimeline(1u, listOf(TimelineMark(SkipKind.OPENING, 130.0, 220.0), TimelineMark(SkipKind.ENDING, 1291.0, 1381.0))))
        model.apply(Event.PlayerBuffered(1u, listOf(TimeRange(0.0, 480.0), TimeRange(700.0, 760.0))))
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

    /** The store of the shot being taken, for the steps that act on the screen. */
    private lateinit var store: AppStore
    private lateinit var model: PlayerModel
    private var playing: ScriptedMpv? = null
    private val sent = mutableListOf<tv.anistream.engine.model.Command>()

    /**
     * AndroidPlayerMore: past 85 % of the episode, the More button in the top bar, opened onto the
     * discussion with its count and that opening it pauses.
     */
    @Test
    fun phone_player_more_menu() = shoot("phone-player-more-menu", discussionSheet = true, after = {
        compose.onNodeWithContentDescription("More").performClick()
    }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }

    @Test
    fun phone_player_more_button_waits_for_the_watched_point() {
        shoot("phone-player-before-85", discussionSheet = true) { _, mpv -> mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0))) }
        compose.onNodeWithContentDescription("More").assertDoesNotExist()
    }

    /** The discussion sheet opened from the player: the video pauses where it is. */
    @Test
    fun phone_player_discussion_sheet() = shoot("phone-player-discussion", discussionSheet = true, after = {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Discussion").performClick()
        compose.waitForIdle()
        // The engine is not there to answer, so the pause shows as the command it was sent.
        check(sent.any { it.toString().contains("TogglePause") }) { "opening the discussion pauses the video: $sent" }
        playing!!.emit(MpvEvent.property("pause", MpvValue.Flag(true)))
    }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }

    private fun shoot(
        name: String,
        picture: Boolean = true,
        discussionSheet: Boolean = false,
        after: () -> Unit = {},
        at: (PlayerModel, ScriptedMpv) -> Unit,
    ) {
        store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        if (discussionSheet) {
            store.apply(Event.DiscussionFound(900_001u, 7u, 214u))
            store.apply(Event.Discussion(THREAD))
        }
        var mpv: ScriptedMpv? = null
        // The clock held, so the controls stay up for the shot as after a tap.
        model = PlayerModel(send = { sent += it }, backend = { ScriptedMpv().also { mpv = it; playing = it } }, scope = scope, now = { time }, hwdec = "no")
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
                // As a preview draws it: what moves for ever, such as the buffering line, as a still.
                val nav = androidx.navigation.compose.rememberNavController()
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides androidx.compose.runtime.remember(nav) { Navigator(nav, Destination.Continue) },
                    LocalInspectionMode provides true,
                ) {
                    PlayerScreen(model, video = { _, modifier, _ ->
                        Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
                    })
                    val target by store.discussion.target.collectAsState()
                    target?.let { DiscussionSheet(ENTRY, it.episode) { store.discussion.dismiss() } }
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
        after()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
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

        private val ENTRY = Entry(
            id = 900_001u, title = "Hoshikuzu no Tabibito", secondary = null, format = "TV", episodes = 13u, year = 2026u, score = null,
            studio = null, synopsis = "", coverUrl = "${FixtureArt.HOST}cover/900001-xl.jpg", bannerUrl = null,
            genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
            airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
            related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
            titleRomaji = "Hoshikuzu no Tabibito", titleEnglish = null, titleNative = null, season = null, startDate = null,
            endDate = null, source = null, adult = false, detail = null,
            status = null, synopsisSpans = emptyList(),
        )

        private fun comment(id: UInt, who: String, text: String, hours: Long, likes: UInt, replies: List<CommentView> = emptyList()) = CommentView(
            id = id, author = who, avatar = null, createdAt = 0L, age = Msg.DiscussionAgeHours(hours),
            body = listOf(CommentBlock(spans = listOf(CommentSpan(text = text)))), likeCount = likes,
            pending = PostState.POSTED, replies = replies,
        )

        private val THREAD = DiscussionView(
            media = 900_001u, episode = 7u, title = "Episode 7 Discussion", commentCount = 214u,
            url = "https://anilist.example/forum/thread/1",
            comments = listOf(
                comment(11u, "lampwick", "Nobody spoke for almost a minute, and it was the best part of the episode.", 2, 41u, listOf(comment(12u, "tamsin", "And the music came back on the exact frame her lantern caught.", 1, 13u))),
                comment(14u, "ferrylog", "The lantern light on the faces is drawn frame by frame, not a filter.", 1, 8u),
            ),
            state = DiscussionState.Ready(more = false), canPost = false, signedIn = false, loadImages = false,
        )

        const val TITLE = "Hoshikuzu no Tabibito"
        const val EPISODE = "The Lantern Road"

        /** The board's episode: the opening 2:10 to 3:40, the ending 21:31 to 23:01. */
        const val CHAPTERS = """[{"title":"Prologue","time":0},{"title":"Opening","time":130},{"title":"Part A","time":220},""" +
            """{"title":"Ending","time":1291},{"title":"Preview","time":1381}]"""
    }
}
