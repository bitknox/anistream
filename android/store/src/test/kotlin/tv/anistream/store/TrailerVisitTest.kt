package tv.anistream.store

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.TrailerControl

@OptIn(ExperimentalCoroutinesApi::class)
class TrailerVisitTest {
    private fun TestScope.visit(sent: MutableList<TrailerControl>) = TrailerVisit(this, { sent += it })

    @Test
    fun `nothing happens until begin`() = runTest {
        val v = visit(mutableListOf())
        v.leave()
        assertFalse(v.live.value)
    }

    @Test
    fun `playing brings the video forward`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        assertTrue(v.live.value)
        assertFalse(v.shown.value)
        v.signal(TrailerSignal.Playing)
        assertTrue(v.shown.value)
    }

    @Test
    fun `nothing playing in eight seconds releases silently`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        advanceTimeBy(7_999)
        assertTrue(v.live.value)
        advanceTimeBy(2)
        assertFalse(v.live.value)
        assertFalse(v.shown.value)
    }

    @Test
    fun `an error releases`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        v.signal(TrailerSignal.Failed(150))
        assertFalse(v.live.value)
    }

    @Test
    fun `scrolling away fades first and pauses after the fade`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        assertFalse(v.shown.value)
        assertTrue(sent.isEmpty())
        advanceTimeBy(301)
        assertEquals(listOf(TrailerControl.PAUSE), sent)
    }

    @Test
    fun `scrolling back plays and waits for the picture`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(400)
        v.scrolledAway(false)
        assertEquals(listOf(TrailerControl.PAUSE, TrailerControl.PLAY), sent)
        assertFalse(v.shown.value)
        v.signal(TrailerSignal.Playing)
        assertTrue(v.shown.value)
    }

    @Test
    fun `scrolling back before the pause just brings it forward`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(100)
        v.scrolledAway(false)
        advanceTimeBy(1_000)
        assertTrue(sent.isEmpty())
        assertTrue(v.shown.value)
    }

    @Test
    fun `thirty seconds away releases for the visit`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(300 + 29_000)
        assertTrue(v.live.value)
        advanceTimeBy(2_000)
        assertFalse(v.live.value)
        v.begin()
        assertFalse(v.live.value)
    }

    @Test
    fun `the end returns the banner then releases and does not loop`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.signal(TrailerSignal.Ending)
        assertFalse(v.shown.value)
        assertTrue(v.live.value)
        advanceTimeBy(301)
        assertFalse(v.live.value)
    }

    @Test
    fun `leaving the page releases`() = runTest {
        val v = visit(mutableListOf())
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.release()
        assertFalse(v.live.value)
        assertFalse(v.shown.value)
    }
}
