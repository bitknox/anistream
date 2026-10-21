// The player model on a scripted engine and a scripted mpv: ops to calls, events to reports,
// closing, the cards' rules, and the decoder watchdog.

package tv.anistream.player

import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Ending
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.MpvOption
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.PlayerChoice
import tv.anistream.engine.model.PlayerCommand
import tv.anistream.engine.model.PlayerEnd
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerReport
import tv.anistream.engine.model.PlayerUi
import tv.anistream.engine.model.SkipKind
import tv.anistream.engine.model.TrackKind
import tv.anistream.engine.model.MpvValue as EngineValue

class OpsTest {
    @Test
    fun eachOpIsOneCallOnMpvInOrder() = runTest {
        val rig = Rig(this)
        rig.ops(
            1u,
            PlayerOp.Open(false, 1.5, 80.0),
            PlayerOp.Load("https://cdn.example/7.mkv", listOf(MpvOption("start", "600"), MpvOption("alang", "ja,jpn"))),
            PlayerOp.AddSubtitle("/tmp/full.ass", "Full", "en"),
            PlayerOp.Set("aid", EngineValue.Int(2)),
            PlayerOp.Set("sub-delay", EngineValue.Double(0.5)),
            PlayerOp.Set("chapter-list", EngineValue.Node("""[{"title":"Opening","time":3.5}]""")),
            PlayerOp.Cycle("pause"),
            PlayerOp.Add("volume", 5.0),
            PlayerOp.Seek(30.0, true),
            PlayerOp.Seek(-10.0, false),
            PlayerOp.Shaders(listOf("/s/a.glsl", "/s/b.glsl")),
            PlayerOp.AudioFilter(true, "@night:dynaudnorm"),
            PlayerOp.AudioFilter(false, "@night"),
            PlayerOp.Stop,
        )
        assertEquals("no", rig.mpv.begun)
        assertTrue(rig.model.isPresented)
        assertEquals(
            listOf(
                MpvCall.Set("speed", MpvValue.Real(1.5)),
                MpvCall.Set("volume", MpvValue.Real(80.0)),
                MpvCall.Load("https://cdn.example/7.mkv", mapOf("start" to "600", "alang" to "ja,jpn")),
                MpvCall.AddSubtitle("/tmp/full.ass", "Full", "en"),
                MpvCall.Set("aid", MpvValue.Int64(2)),
                MpvCall.Set("sub-delay", MpvValue.Real(0.5)),
                MpvCall.Set(
                    "chapter-list",
                    MpvValue.List(listOf(MpvValue.Map(mapOf("title" to MpvValue.Text("Opening"), "time" to MpvValue.Real(3.5))))),
                ),
                MpvCall.Command(listOf("cycle", "pause")),
                MpvCall.Command(listOf("add", "volume", "5.0")),
                MpvCall.Seek(30.0, SeekMode.Absolute),
                MpvCall.Seek(-10.0, SeekMode.Relative),
                MpvCall.Shaders(listOf("/s/a.glsl", "/s/b.glsl")),
                MpvCall.Command(listOf("af", "add", "@night:dynaudnorm")),
                MpvCall.Command(listOf("af", "remove", "@night")),
                MpvCall.Command(listOf("stop")),
            ),
            rig.mpv.calls,
        )
    }

    @Test
    fun fullScreenIsTheWindowsNotMpvs() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ops(1u, PlayerOp.Cycle("fullscreen"))
        assertTrue(rig.model.fullScreen)
        rig.ops(1u, PlayerOp.Set("fullscreen", EngineValue.Flag(false)))
        assertFalse(rig.model.fullScreen)
        rig.ops(1u, PlayerOp.ToggleFullscreen)
        assertTrue(rig.model.fullScreen)
        assertFalse(rig.mpv.calls.any { it is MpvCall.Command && "fullscreen" in it.args })
    }

    @Test
    fun restatingTheChaptersReportsThemAsTheyStand() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.mpv.answers["chapter-list"] = MpvValue.List(listOf(MpvValue.Map(mapOf("title" to MpvValue.Text("Opening"), "time" to MpvValue.Int64(5)))))
        rig.ops(1u, PlayerOp.RestateChapters)
        assertEquals(1uL to PlayerReport.Property("chapter-list", """[{"title":"Opening","time":5}]"""), rig.reports.last())
    }
}

class ReportTest {
    @Test
    fun mpvsEventsAreReportedForThePlaybackWhoseFileItHas() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.playhead(12.0)
        assertEquals(1uL to PlayerReport.Property("time-pos", "12"), rig.reports.last())

        // The next episode: the file is stopped and the next loaded, which is the next playback's
        // from mpv's start-file on.
        rig.ops(1u, PlayerOp.Stop)
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        rig.emit(MpvEvent.endFile("stop"))
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
        val tail = rig.reports.takeLast(2)
        assertEquals(1uL to PlayerReport.Ended(PlayerEnd.STOP), tail.first())
        assertEquals(2uL to PlayerReport.FileLoaded, tail.last())
        assertEquals("the same mpv", 1, rig.made.size)
    }

    @Test
    fun onlyTheEnginesPropertiesAreReported() = runTest {
        val rig = Rig(this)
        rig.opened()
        val before = rig.reports.size
        rig.emit(MpvEvent.property("estimated-vf-fps", MpvValue.Real(23.976)))
        rig.emit(MpvEvent.property("paused-for-cache", MpvValue.Flag(true)))
        assertEquals(before + 1, rig.reports.size)
        assertEquals(1uL to PlayerReport.Property("paused-for-cache", "true"), rig.reports.last())
        assertNotNull(rig.model.bufferingSince)
    }

    @Test
    fun theDisplayIsToldToMpvAndReportedWithoutComputeShaders() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.model.surfaceDisplay(DisplayInfo(2400, 1080, 2.625, 60.0))
        rig.settle()
        assertEquals(MpvCall.Set("display-fps-override", MpvValue.Real(60.0)), rig.mpv.calls.last())
        assertEquals(1uL to PlayerReport.Display(2400u, 1080u, 2.625, 60.0, null, false), rig.reports.last())
    }

    @Test
    fun theDisplayIsSaidAsThePlayerOpensBeforeAnySurface() = runTest {
        val rig = Rig(this, display = DisplayInfo(2400, 1080, 2.625, 60.0))
        rig.opened()
        val display = rig.reports.indexOfFirst { it.second is PlayerReport.Display }
        val loaded = rig.reports.indexOfFirst { it.second == PlayerReport.FileLoaded }
        assertTrue("the display before the file", display in 0 until loaded)
    }

    @Test
    fun theFirstReportsSayWhatTheDeviceRunsOn() = runTest {
        val rig = Rig(this)
        rig.opened()
        assertTrue(rig.reports.any { it.second is PlayerReport.Power })
    }

    @Test
    fun aHookLibplaceboDisabledIsReported() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent(MpvNative.Event.LOG_MESSAGE, "vo/gpu-next/libplacebo", """{"level":"error","text":"Failed executing hook, disabling"}"""))
        assertEquals(1uL to PlayerReport.HookDisabled("Failed executing hook, disabling"), rig.reports.last())
    }
}

class ClosingTest {
    @Test
    fun theViewerClosingReportsQuitThenClosedAndRefusesWhatFollows() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.model.viewerClosed()
        rig.settle()
        assertEquals(listOf(PlayerReport.Ended(PlayerEnd.QUIT), PlayerReport.Closed), rig.reports.takeLast(2).map { it.second })
        assertFalse(rig.model.isPresented)
        assertTrue(rig.mpv.ended)

        // The engine's answer to a player that is gone, a Close among it: refused, said to nothing.
        val sent = rig.sent.size
        rig.ops(1u, PlayerOp.Cycle("pause"), PlayerOp.Quit, PlayerOp.Close)
        assertEquals(sent, rig.sent.size)
        assertFalse(rig.model.isPresented)
    }

    @Test
    fun theEnginesQuitReportsHowTheFileEnded() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ops(1u, PlayerOp.Quit)
        assertEquals(listOf(PlayerReport.Ended(PlayerEnd.QUIT), PlayerReport.Closed), rig.reports.takeLast(2).map { it.second })
        assertFalse(rig.model.isPresented)
    }

    @Test
    fun theEnginesCloseSaysNothingMore() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.emit(MpvEvent.endFile("eof"))
        assertEquals(PlayerReport.Ended(PlayerEnd.EOF), rig.reports.last().second)
        val sent = rig.sent.size
        rig.ops(1u, PlayerOp.Close)
        assertEquals(sent, rig.sent.size)
        assertFalse(rig.model.isPresented)
        assertTrue(rig.mpv.ended)
    }

    @Test
    fun aCloseBeforeAnyPictureWaitsForTheVerdict() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        rig.opened()
        rig.ops(1u, PlayerOp.Close)
        assertTrue("the starting screen waits for how it ended", rig.model.isPresented)
        rig.model.apply(Event.PlaybackEnded(false, Ending.Failed, false))
        assertEquals(PlaybackFailure(900_001u, "7"), rig.model.failure)
        rig.model.viewerClosed()
        assertFalse(rig.model.isPresented)
    }

    @Test
    fun aNewPlaybackOpensANewPlayerAfterOneClosed() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.ops(1u, PlayerOp.Close)
        rig.opened(2u)
        assertEquals(2, rig.made.size)
        assertTrue(rig.model.isPresented)
        assertEquals(2uL to PlayerReport.FileLoaded, rig.reports.last())
    }

    @Test
    fun mpvGoingAwayByItselfClosesThePlayer() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.SHUTDOWN))
        assertFalse(rig.model.isPresented)
        assertEquals(PlayerReport.Closed, rig.reports.last().second)
    }

    private fun staged(rig: Rig, episode: String) =
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", episode, null, 1.0))

    /** What the engine says to a player it opens for [playback]. */
    private fun opening(rig: Rig, playback: ULong, episode: String) {
        rig.ui(playback, PlayerUi.Starting(900_001u, "Stardust Wayfarer", episode, null, null, null))
        rig.ops(playback, PlayerOp.Open(false, null, null), PlayerOp.Load("/media/$episode.mkv", emptyList()))
    }

    @Test
    fun closingWhileThePlayIsFoundStopsItAndItsPlayerNeverComesUp() = runTest {
        val rig = Rig(this)
        // The playback the player showed before, long over: what it last heard of.
        rig.opened(1u)
        rig.model.viewerClosed()
        staged(rig, "7")
        assertTrue("the starting screen, finding a source", rig.model.isPresented)
        rig.model.viewerClosed()
        assertFalse(rig.model.isPresented)
        assertEquals("for the play being found, not the playback before", Command.Player(PlayerCommand.Stop), rig.sent.last())

        // Found all the same: its player is refused, and the engine told it closed, once.
        opening(rig, 2u, "7")
        rig.ops(2u, PlayerOp.Cycle("pause"))
        assertFalse(rig.model.isPresented)
        assertEquals(1, rig.made.size)
        assertEquals(1, rig.reports.count { it == (2uL to PlayerReport.Closed) })

        // A play after it is the viewer's again.
        staged(rig, "8")
        rig.opened(3u)
        assertTrue(rig.model.isPresented)
        assertEquals(2, rig.made.size)
    }

    /** The viewer closes the player on the next episode's starting screen, while it is found. */
    private fun closedWhileTheNextIsFound(rig: Rig, ask: (PlayerModel) -> Unit) {
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.ui(1u, PlayerUi.UpNext(NextEpisode("8", null, null), true, 42.0, 10u, 3u))
        ask(rig.model)
        staged(rig, "8")
        // The window says what comes, as the playback driving it.
        rig.ui(1u, PlayerUi.Starting(900_001u, "Stardust Wayfarer", "8", null, null, null))
        rig.model.viewerClosed()
        rig.settle()
        assertEquals(listOf(PlayerReport.Ended(PlayerEnd.QUIT), PlayerReport.Closed), rig.reports.takeLast(2).map { it.second })
        assertEquals("the next episode, still being found, stops too", Command.Player(PlayerCommand.Stop), rig.sent.last())
        assertFalse(rig.model.isPresented)

        opening(rig, 2u, "8")
        assertFalse("no player comes back up for it", rig.model.isPresented)
        assertEquals(1, rig.made.size)
        assertEquals(2uL to PlayerReport.Closed, rig.reports.last())
    }

    @Test
    fun closingAfterPlayNowBringsNoPlayerBack() = runTest {
        closedWhileTheNextIsFound(Rig(this)) { it.playNext() }
    }

    @Test
    fun closingAfterAnEpisodeIsPickedBringsNoPlayerBack() = runTest {
        closedWhileTheNextIsFound(Rig(this)) { it.choose(PlayerChoice.Episode("8")) }
    }

    @Test
    fun aPlaybackThatNeverOpenedComesDownWhenItPlaysElsewhere() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        assertTrue(rig.model.isPresented)
        rig.model.apply(Event.PlaybackEnded(false, Ending.Elsewhere, false))
        assertFalse(rig.model.isPresented)
        assertTrue(rig.made.isEmpty())
    }
}

class CardTest {
    private fun upNext(rig: Rig, auto: Boolean = true, endingAt: Double? = 42.0, countdown: UInt = 10u, after: UInt = 3u, playback: ULong = 1u) =
        rig.ui(playback, PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", null), auto, endingAt, countdown, after))

    @Test
    fun upNextCountsDownFromTheEndingAndPlaysNextAtZero() = runTest {
        val rig = Rig(this)
        rig.opened()
        upNext(rig)
        rig.playhead(41.0)
        assertFalse(rig.model.upNextShown)
        rig.playhead(42.0)
        assertTrue(rig.model.upNextShown)
        assertEquals(PlayerModel.Corner.UpNext, rig.model.corner)
        assertEquals(10, rig.model.countdown?.first)
        rig.playhead(47.0)
        assertEquals(5, rig.model.countdown?.first)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        rig.playhead(52.0)
        assertEquals(1, rig.controls.count { it == PlayerCommand.PlayNext })
        rig.playhead(53.0)
        assertEquals("once", 1, rig.controls.count { it == PlayerCommand.PlayNext })
    }

    @Test
    fun upNextCountsDownAsLongAsTheEngineSays() = runTest {
        val rig = Rig(this)
        rig.opened()
        upNext(rig, countdown = 5u)
        rig.playhead(42.0)
        assertEquals(5, rig.model.countdown?.first)
        rig.playhead(46.0)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        rig.playhead(47.0)
        assertEquals(1, rig.controls.count { it == PlayerCommand.PlayNext })
    }

    @Test
    fun withNoCountdownOrAutoOffUpNextWaitsForTheViewer() = runTest {
        for ((auto, countdown) in listOf(true to 0u, false to 10u)) {
            val rig = Rig(this)
            rig.opened()
            upNext(rig, auto = auto, countdown = countdown)
            rig.playhead(60.0)
            assertTrue(rig.model.upNextShown)
            assertNull(rig.model.countdown)
            assertFalse(PlayerCommand.PlayNext in rig.controls)
            rig.model.playNext()
            assertEquals(PlayerCommand.PlayNext, rig.controls.last())
        }
    }

    @Test
    fun withTalkBackUpNextWaitsAndTheControlsStay() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.model.assisted = true
        upNext(rig)
        rig.playhead(60.0)
        assertTrue(rig.model.upNextShown)
        assertNull(rig.model.countdown)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        advanceTimeBy(10_000)
        assertTrue(rig.model.controlsVisible)
    }

    @Test
    fun withoutEndingDataUpNextShowsNearTheEnd() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent.property("duration", MpvValue.Real(1440.0)))
        upNext(rig, endingAt = null)
        rig.playhead(1409.0)
        assertFalse(rig.model.upNextShown)
        rig.playhead(1410.0)
        assertTrue(rig.model.upNextShown)
    }

    @Test
    fun aSeekPastTheEndingRunsTheCountdownAgainRatherThanPlayingNext() = runTest {
        val rig = Rig(this)
        rig.opened()
        upNext(rig)
        rig.playhead(44.0)
        assertEquals(8, rig.model.countdown?.first)
        // Well past where the countdown would have run out: it runs again, all ten seconds.
        rig.model.seekTo(80.0)
        rig.ops(1u, PlayerOp.Seek(80.0, true))
        rig.playhead(80.0)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        assertEquals(10, rig.model.countdown?.first)
        rig.playhead(89.0)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        rig.playhead(90.0)
        assertEquals(1, rig.controls.count { it == PlayerCommand.PlayNext })
    }

    @Test
    fun scrubbingPastTheEndingPlaysNothingUntilTheCountdownRunsAgain() = runTest {
        val rig = Rig(this)
        rig.opened()
        upNext(rig)
        rig.playhead(43.0)
        // The keyframe seeks under the finger move the playhead past the countdown's end.
        rig.model.scrub(70.0)
        rig.playhead(70.0)
        assertFalse("not while scrubbing", PlayerCommand.PlayNext in rig.controls)
        rig.model.endScrub()
        rig.ops(1u, PlayerOp.Seek(70.0, true))
        rig.playhead(71.0)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
        assertEquals(9, rig.model.countdown?.first)
        rig.playhead(80.0)
        assertEquals(1, rig.controls.count { it == PlayerCommand.PlayNext })
    }

    @Test
    fun aCancelledUpNextNeverPlaysNext() = runTest {
        val rig = Rig(this)
        rig.opened()
        upNext(rig)
        rig.playhead(43.0)
        rig.model.cancelNext()
        assertEquals(PlayerCommand.CancelNext, rig.controls.last())
        assertFalse(rig.model.upNextShown)
        rig.playhead(60.0)
        assertFalse(PlayerCommand.PlayNext in rig.controls)
    }

    @Test
    fun theCornerHoldsOneCardStallThenUndoThenUpNextThenSkip() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ui(1u, PlayerUi.Skip(SkipKind.ENDING, 56.0))
        assertEquals(PlayerModel.Corner.Skip, rig.model.corner)
        upNext(rig)
        rig.playhead(43.0)
        assertEquals(PlayerModel.Corner.UpNext, rig.model.corner)
        rig.ui(1u, PlayerUi.AutoSkipped(SkipKind.OPENING, 5.0))
        assertEquals(PlayerModel.Corner.Undo, rig.model.corner)
        rig.ui(1u, PlayerUi.Stall(8u))
        assertEquals(PlayerModel.Corner.Stall, rig.model.corner)
        rig.model.keepWaiting()
        assertEquals(PlayerCommand.StallWait, rig.controls.last())
        rig.ui(1u, PlayerUi.Stall(9u))
        assertNull("not again until the stall ends", rig.model.stall)
        rig.ui(1u, PlayerUi.StallCleared)
        rig.ui(1u, PlayerUi.Stall(8u))
        assertNotNull(rig.model.stall)
        rig.model.switchSource()
        assertEquals(PlayerCommand.StallSwitch, rig.controls.last())
        assertNull(rig.model.stall)
    }

    @Test
    fun undoLastsSixSecondsOfPlayback() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ui(1u, PlayerUi.AutoSkipped(SkipKind.OPENING, 5.0))
        rig.playhead(20.0)
        rig.playhead(25.0)
        assertNotNull(rig.model.undo)
        rig.playhead(26.0)
        assertNull(rig.model.undo)
    }

    @Test
    fun undoGoesAfterFifteenSecondsPaused() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ui(1u, PlayerUi.AutoSkipped(SkipKind.OPENING, 5.0))
        advanceTimeBy(14_000)
        assertNotNull(rig.model.undo)
        advanceTimeBy(1_500)
        assertNull(rig.model.undo)
    }

    @Test
    fun takingTheSkipAndUndoingAreTheEnginesToDo() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ui(1u, PlayerUi.Skip(SkipKind.OPENING, 20.0))
        assertFalse(rig.model.skipDimmed)
        advanceTimeBy(8_300)
        assertTrue("the skip dims after 8 s", rig.model.skipDimmed)
        rig.model.takeSkip()
        assertNull(rig.model.skip)
        rig.ui(1u, PlayerUi.AutoSkipped(SkipKind.OPENING, 5.0))
        rig.model.undoSkip()
        assertEquals(listOf(PlayerCommand.TakeSkip, PlayerCommand.UndoSkip(5.0)), rig.controls)
    }

    @Test
    fun aNewPlaybackStartsWithNoneOfTheLastOnesCards() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.ui(1u, PlayerUi.Skip(SkipKind.OPENING, 20.0))
        rig.ui(1u, PlayerUi.Resumed(600.0))
        rig.ui(2u, PlayerUi.Starting(900_001u, "Stardust Wayfarer", "8", null, null, null))
        assertNull(rig.model.skip)
        assertNull(rig.model.resume)
        assertEquals("Starting episode 8", rig.model.startingLine())
    }

    @Test
    fun theStartingLineAdmitsTheWait() = runTest {
        val rig = Rig(this)
        rig.ui(1u, PlayerUi.Starting(900_001u, "Stardust Wayfarer", "7", "Rain over Kiriya Pass", "http://art.example/c.png", "http://art.example/b.png"))
        assertTrue("nothing remembered from PlaybackStaged is needed", rig.model.isPresented)
        assertEquals("http://art.example/b.png", rig.model.starting?.bannerUrl)
        advanceTimeBy(4_100)
        assertEquals("Finding a source · 4 s", rig.model.startingLine())
        assertEquals("Resolving the stream · 4 s", rig.model.startingLine("resolving the stream"))
    }

    @Test
    fun theStartingScreenStaysUntilTheNextFilesFirstFrame() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertTrue(rig.model.hasPicture)
        rig.ui(2u, PlayerUi.Starting(900_001u, "Stardust Wayfarer", "8", null, null, null))
        // The old file's frames do not lift it, nor count as the next one's picture.
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.ops(1u, PlayerOp.Stop)
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertNotNull(rig.model.starting)
        assertFalse(rig.model.hasPicture)
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertNull(rig.model.starting)
    }

    /** Playback [playback] rolls on from the one before, through Up Next's countdown. */
    private fun rollOn(rig: Rig, playback: ULong, after: UInt = 3u, touch: Boolean = false) {
        upNext(rig, after = after, playback = playback - 1u)
        rig.playhead(52.0)
        if (touch) rig.model.seekBy(10.0)
        rig.ops(playback, PlayerOp.Load("/media/$playback.mkv", emptyList()))
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(0.5)
    }

    @Test
    fun threeEpisodesInARowWithNobodyThereAskStillWatching() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        for (playback in 2uL..4uL) {
            rollOn(rig, playback)
            assertEquals("after ${playback - 1u} in a row", playback == 4uL, rig.model.stillWatching)
        }
        assertEquals("it pauses", PlayerCommand.TogglePause, rig.controls.last())
        rig.emit(MpvEvent.property("pause", MpvValue.Flag(true)))
        rig.model.continueWatching()
        assertFalse(rig.model.stillWatching)
        assertEquals(PlayerCommand.TogglePause, rig.controls.last())
    }

    @Test
    fun stillWatchingAsksAfterAsManyAsTheEngineSays() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        for (playback in 2uL..3uL) {
            rollOn(rig, playback, after = 2u)
            assertEquals(playback == 3uL, rig.model.stillWatching)
        }
        assertEquals(2, rig.model.stillWatchingAfter)
    }

    @Test
    fun anEpisodePlayedNextAfterASeekIsNotOneInARowWithNobodyThere() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        for (playback in 2uL..4uL) {
            upNext(rig, after = 2u, playback = playback - 1u)
            if (playback == 3uL) {
                // The viewer seeks into the ending, and the countdown runs out after it.
                rig.model.seekTo(45.0)
                rig.ops(playback - 1u, PlayerOp.Seek(45.0, true))
                rig.playhead(55.0)
            } else {
                rig.playhead(52.0)
            }
            rig.ops(playback, PlayerOp.Load("/media/$playback.mkv", emptyList()))
            rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            rig.playhead(0.5)
        }
        // Two rolled on with nobody there would ask; the one after the seek is not among them.
        assertEquals(3, rig.controls.count { it == PlayerCommand.PlayNext })
        assertFalse(rig.model.stillWatching)
    }

    @Test
    fun stillWatchingGoesOnceAnsweredOrTheViewerCloses() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        for (playback in 2uL..3uL) rollOn(rig, playback, after = 2u)
        assertTrue(rig.model.stillWatching)
        rig.model.continueWatching()
        assertFalse("answered", rig.model.stillWatching)
        // Asked again, and the player closed under it, by the back gesture.
        rig.model.askStillWatching()
        rig.model.viewerClosed()
        assertFalse("closed", rig.model.stillWatching)
        assertFalse(rig.model.isPresented)
    }

    @Test
    fun anyInputStartsTheCountAgain() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        for (playback in 2uL..4uL) rollOn(rig, playback, touch = playback == 3uL)
        assertFalse(rig.model.stillWatching)
    }
}

class ViewerTest {
    @Test
    fun commandsAreAimedAtThePlaybackOnScreen() = runTest {
        val rig = Rig(this)
        rig.model.togglePause()
        rig.opened(3u)
        rig.model.togglePause()
        rig.model.takeSkip()
        assertEquals("whatever plays until the engine names one", listOf(null, 3uL, 3uL), rig.aims)
    }

    @Test
    fun picksAndTracksGoToTheEngine() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent.property("speed", MpvValue.Real(1.0)))
        rig.emit(
            MpvEvent(
                MpvNative.Event.PROPERTY_CHANGE, "track-list",
                """[{"type":"audio","id":1,"lang":"jpn","selected":true},{"type":"audio","id":2,"lang":"eng"},{"type":"sub","id":1,"lang":"eng","title":"Full"}]""",
            ),
        )
        assertEquals(3, rig.model.tracks.size)
        rig.model.selectAudio(2)
        rig.model.selectSubtitle(null)
        rig.model.playAt(1.5)
        rig.model.choose(PlayerChoice.Episode("3"))
        rig.model.setUpscaling("sharp")
        rig.model.setNightMode(true)
        rig.model.step(1)
        // The engine selects the tracks, and keeps them for the series as the viewer's.
        assertFalse(rig.mpv.calls.any { it is MpvCall.Set && it.property in setOf("aid", "sid") })
        assertEquals(
            listOf(
                PlayerCommand.ChooseTrack(TrackKind.AUDIO, 2), PlayerCommand.ChooseTrack(TrackKind.SUBTITLE, null),
                PlayerCommand.Speed(1.5), PlayerCommand.Choose(PlayerChoice.Episode("3")), PlayerCommand.Upscaling("sharp"),
                PlayerCommand.NightMode(true), PlayerCommand.Step(1),
            ),
            rig.controls,
        )
        assertEquals(listOf(1L, 2L), rig.model.audioTracks.map { it.id })
        assertEquals("Full", rig.model.subtitleTracks.first().title)
    }

    @Test
    fun scrubbingSeeksByKeyframesThenExactlyOnRelease() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.model.scrub(10.0)
        rig.model.scrub(10.2)
        rig.model.scrub(11.0)
        assertTrue(rig.model.controlsVisible)
        rig.model.endScrub()
        rig.settle()
        assertEquals(listOf(MpvCall.Seek(10.0, SeekMode.Keyframe), MpvCall.Seek(11.0, SeekMode.Keyframe)), rig.mpv.calls.takeLast(2))
        assertEquals(PlayerCommand.SeekTo(11.0), rig.controls.last())
    }

    @Test
    fun theControlsHideAfterThreeSecondsUnlessHeld() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.emit(MpvEvent.property("pause", MpvValue.Flag(false)))
        rig.model.poke()
        assertTrue(rig.model.controlsVisible)
        advanceTimeBy(3_300)
        assertFalse(rig.model.controlsVisible)
        rig.model.hold("sheet", true)
        advanceTimeBy(5_000)
        assertTrue(rig.model.controlsVisible)
        rig.model.hold("sheet", false)
        rig.emit(MpvEvent.property("pause", MpvValue.Flag(true)))
        advanceTimeBy(5_000)
        assertTrue("paused, they stay", rig.model.controlsVisible)
    }

    @Test
    fun theBackgroundTurnsTheVideoTrackOff() = runTest {
        val rig = Rig(this)
        rig.opened()
        rig.model.backgrounded(true)
        rig.model.backgrounded(false)
        rig.settle()
        assertEquals(listOf(MpvCall.Set("vid", MpvValue.Text("no")), MpvCall.Set("vid", MpvValue.Text("auto"))), rig.mpv.calls.takeLast(2))
    }

    @Test
    fun aPlayerOpenedInTheBackgroundPlaysTheAudioAlone() = runTest {
        val rig = Rig(this)
        rig.model.backgrounded(true)
        rig.opened()
        assertTrue(MpvCall.Set("vid", MpvValue.Text("no")) in rig.mpv.calls)
    }

    @Test
    fun theTitlesArtStaysForTheSessionAfterTheStart() = runTest {
        val rig = Rig(this)
        rig.ui(1u, PlayerUi.Starting(900_001u, "Stardust Wayfarer", "7", null, "http://art.example/c.png", "http://art.example/b.png"))
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertNull(rig.model.starting)
        assertEquals("http://art.example/c.png", rig.model.coverUrl)
        rig.model.viewerClosed()
        assertNull(rig.model.coverUrl)
    }

    @Test
    fun theSurfaceGoesToMpvWhenEitherComesFirst() = runTest {
        val rig = Rig(this)
        rig.model.surfaceChanged(2400, 1080)
        rig.model.surfaceCreated(android.view.Surface(android.graphics.SurfaceTexture(0)))
        rig.opened()
        assertEquals(1, rig.mpv.attached)
        assertEquals(listOf(2400 to 1080), rig.mpv.sizes)
        rig.model.surfaceChanged(1080, 2400)
        rig.model.surfaceDestroyed()
        assertEquals(listOf(2400 to 1080, 1080 to 2400), rig.mpv.sizes)
        assertEquals(1, rig.mpv.detached)
    }

    @Test
    fun theSegmentsComeFromTheChapters() {
        val segments = seekSegments(
            listOf(Chapter("Prologue", 0.0), Chapter("Opening", 5.0), Chapter("Part A", 20.0), Chapter("Ending", 42.0), Chapter("Preview", 56.0)),
            60.0,
        )
        assertEquals(
            listOf(
                SeekSegment(SeekSegment.Kind.Plain, 0.0, 5.0), SeekSegment(SeekSegment.Kind.Opening, 5.0, 20.0),
                SeekSegment(SeekSegment.Kind.Plain, 20.0, 42.0), SeekSegment(SeekSegment.Kind.Ending, 42.0, 56.0),
                SeekSegment(SeekSegment.Kind.Plain, 56.0, 60.0),
            ),
            segments,
        )
        assertEquals("12:04", PlayerClock.label(724.0))
        assertEquals("−2:29", PlayerClock.remaining(149.0))
        assertEquals("1:02:09", PlayerClock.label(3729.0))
    }
}

class WatchdogTest {
    @Test
    fun aLoadWaitsForTheSurfaceWhenDecodingInHardware() = runTest {
        val rig = Rig(this, hwdec = "mediacodec,mediacodec-copy")
        rig.ops(1u, PlayerOp.Open(false, null, null), PlayerOp.Load("/media/7.mkv", emptyList()))
        assertTrue("held for the surface", rig.mpv.calls.none { it is MpvCall.Load })
        rig.model.surfaceCreated(android.view.Surface(android.graphics.SurfaceTexture(0)))
        rig.settle()
        assertEquals(MpvCall.Load("/media/7.mkv", emptyMap()), rig.mpv.calls.last())
    }

    @Test
    fun aLoadWithNoSurfaceForLongDecodesInSoftware() = runTest {
        val rig = Rig(this, hwdec = "mediacodec,mediacodec-copy")
        rig.ops(1u, PlayerOp.Open(false, null, null), PlayerOp.Load("/media/7.mkv", emptyList()))
        advanceTimeBy(PlayerTiming.SURFACE_WAIT + 100)
        assertEquals(MpvCall.Load("/media/7.mkv", mapOf("hwdec" to "no")), rig.mpv.calls.last())
    }

    @Test
    fun aDecoderWithNoPictureMovesToSoftware() = runTest {
        val rig = Rig(this, hwdec = "mediacodec")
        rig.model.surfaceCreated(android.view.Surface(android.graphics.SurfaceTexture(0)))
        rig.opened()
        advanceTimeBy(PlayerTiming.FIRST_FRAME_WATCHDOG + 1_000)
        assertTrue(rig.mpv.software)
        assertEquals("no", rig.model.decoding)
        assertEquals(1, rig.made.size)
    }

    @Test
    fun waitingForDataIsNotAStalledDecoder() = runTest {
        val rig = Rig(this, hwdec = "mediacodec")
        rig.model.surfaceCreated(android.view.Surface(android.graphics.SurfaceTexture(0)))
        rig.opened()
        rig.emit(MpvEvent.property("paused-for-cache", MpvValue.Flag(true)))
        advanceTimeBy(PlayerTiming.FIRST_FRAME_WATCHDOG + 5_000)
        assertFalse(rig.mpv.software)
    }

    @Test
    fun anMpvThatStopsAnsweringIsReplacedWhereTheFileWas() = runTest {
        val rig = Rig(this, hwdec = "mediacodec")
        rig.model.surfaceCreated(android.view.Surface(android.graphics.SurfaceTexture(0)))
        rig.opened(options = listOf(MpvOption("http-header-fields", "Referer: https://lantern.example/")))
        rig.ops(1u, PlayerOp.Shaders(listOf("/s/a.glsl")))
        val first = rig.mpv
        first.stuck = true
        val generation = rig.model.surfaceGeneration
        advanceTimeBy(PlayerTiming.FIRST_FRAME_WATCHDOG + 3_000)
        assertEquals(2, rig.made.size)
        assertTrue(first.ended || first.listener == null)
        assertEquals(generation + 1, rig.model.surfaceGeneration)
        assertEquals("no", rig.mpv.begun)
        // The new surface comes from a new SurfaceView; the file waits for nothing in software.
        assertEquals(
            listOf(
                MpvCall.Load("/media/episode.mkv", mapOf("http-header-fields" to "Referer: https://lantern.example/", "hwdec" to "no")),
                MpvCall.Shaders(listOf("/s/a.glsl")),
            ),
            rig.mpv.calls,
        )
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
        assertEquals(1uL to PlayerReport.FileLoaded, rig.reports.last())
    }
}
