// The line under a sign-in code: counted from the code's expiry, to nought and no further.

package tv.anistream.store

import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

class CodeCountdownTest {
    @Test
    fun aCodeCountsDownToNoughtAndNoFurther() {
        assertEquals(900L, CodeCountdown.secondsLeft(1_000, 100_000))
        assertEquals("Expires in 15:00", CodeCountdown.line(900))
        assertEquals("Expires in 14:59", CodeCountdown.line(CodeCountdown.secondsLeft(1_000, 101_000)))
        assertEquals(0L, CodeCountdown.secondsLeft(1_000, 1_000_000))
        assertEquals(0L, CodeCountdown.secondsLeft(1_000, 1_500_000))
        assertEquals("Code expired", CodeCountdown.line(0))
    }

    @Test
    fun theTicksEndAtNought() = runTest {
        var now = 997_000L
        val ticks = CodeCountdown.ticks(1_000) { now.also { now += 1_000 } }.toList()
        assertEquals(listOf(3L, 2L, 1L, 0L), ticks)
    }

    @Test
    fun aCodeThatIsOutAlreadySaysNoughtOnce() = runTest {
        assertEquals(listOf(0L), CodeCountdown.ticks(1_000) { 2_000_000L }.toList())
    }
}
