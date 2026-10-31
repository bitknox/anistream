// mpv answers back-to-back replace loads with one START_FILE, for the newest playlist entry only
// (libmpv 2.5, as mpv-host's book found): the model matches each start to its load by that entry,
// so the starting screen fades for the newest load and the loads before it are let go.

package tv.anistream.player

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerUi

class CoalescedLoadsTest {
    private fun starting(rig: Rig, playback: ULong, episode: String) =
        rig.ui(playback, PlayerUi.Starting(900001u, "Stardust Wayfarer", episode, null, null, null))

    @Test
    fun twoQuickNextEpisodesFadeTheStartingScreenForTheNewest() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertTrue(rig.model.hasPicture)

        // Next, and next again before mpv has started the first of them.
        starting(rig, 2u, "8")
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        starting(rig, 3u, "9")
        rig.ops(3u, PlayerOp.Load("/media/9.mkv", emptyList()))

        // mpv coalesces the two replace loads: one start-file, for /media/9.mkv.
        rig.emit(MpvEvent.startFile(rig.mpv.newestEntry))
        rig.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(1.0)
        advanceTimeBy(30_000)
        rig.settle()

        val seen = "reported for #${rig.reports.last().first}, hasPicture ${rig.model.hasPicture}, starting ${rig.model.starting?.episode}"
        assertEquals("reported for #3, hasPicture true, starting null", seen)
    }

    @Test
    fun twoQuickSourceSwitchesShowThePicture() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        // A source switch loads the other stream for the same playback, and a second one at once.
        rig.ops(1u, PlayerOp.Load("/media/7-b.mkv", emptyList()))
        rig.ops(1u, PlayerOp.Load("/media/7-c.mkv", emptyList()))
        rig.emit(MpvEvent.startFile(rig.mpv.newestEntry))
        rig.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(1.0)
        advanceTimeBy(30_000)
        rig.settle()
        assertEquals("hasPicture true", "hasPicture ${rig.model.hasPicture}")
    }

    /** The event thread can read a START_FILE before the load's answer names its entry: it waits for it. */
    @Test
    fun aStartReadBeforeItsLoadAnsweredIsMatchedWhenItDoes() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        starting(rig, 2u, "8")
        val answer = CompletableDeferred<Unit>()
        rig.mpv.answering = answer
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        // mpv started entry 2 and drew its first frame; the worker has not heard back yet.
        rig.emit(MpvEvent.startFile(2))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertEquals(false, rig.model.hasPicture)
        // The answer comes: the start is matched, and the frame was the new file's.
        answer.complete(Unit)
        rig.settle()
        assertEquals("hasPicture true, starting null", "hasPicture ${rig.model.hasPicture}, starting ${rig.model.starting?.episode}")
    }

    /** A start for each load, spaced out: every start moves the file on, and only the last one's frame is the picture. */
    @Test
    fun aStartForEachLoadMovesTheFileAlong() = runTest {
        val rig = Rig(this)
        rig.opened(1u)
        starting(rig, 2u, "8")
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertEquals("a frame of 1 while 2 loads is not 2's", false, rig.model.hasPicture)
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        assertTrue(rig.model.hasPicture)
        assertNull(rig.model.starting)
    }
}
