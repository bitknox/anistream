package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.core.Translation
import tv.anistream.engine.ffi.episodeLine

/**
 * The engine's episode line, called from a plain JUnit test, outside Robolectric. It also guards
 * the shots: this loads a second copy of the engine's bindings after the shots' sandbox loaded
 * its own. In one JVM, that copy would take every engine callback away from TvShots' hosts; each
 * test class runs in a JVM of its own instead (see the unit tests' `forkEvery` in build-logic).
 */
class EpisodeLineTest {
    @Test
    fun a_dub_is_marked_and_a_sub_is_not() {
        assertEquals("Episode 10 · Dub", episodeLine("Episode 10", Translation.DUB))
        assertEquals("Episode 10", episodeLine("Episode 10", Translation.SUB))
        assertEquals("E10", episodeLine("E10", null))
    }
}
