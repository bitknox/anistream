package tv.anistream.player.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The system throws for a picture-in-picture ratio outside 1 : 2.39 to 2.39 : 1, so none may leave. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PictureInPictureAspectTest {
    private val min = 0.418410
    private val max = 2.39

    private fun inRange(aspect: Double?) {
        val value = pipAspectRatio(aspect).toDouble()
        assertTrue("$aspect gave $value", value >= min && value <= max)
    }

    @Test
    fun a_tall_video_gets_the_narrowest_ratio() {
        assertEquals(100.0 / 239.0, pipAspectRatio(320.0 / 800.0).toDouble(), 1e-9)
        inRange(320.0 / 800.0)
        inRange(0.0001)
    }

    @Test
    fun a_wide_video_gets_the_widest_ratio() {
        assertEquals(2.39, pipAspectRatio(3.0).toDouble(), 1e-9)
        inRange(21.0 / 9.0 * 1.2)
    }

    @Test
    fun ratios_just_inside_the_edges_stay_inside() {
        inRange(1 / 2.39)
        inRange(1 / 2.39 + 1e-7)
        inRange(2.39 - 1e-7)
        inRange(2.39)
    }

    @Test
    fun unknown_and_broken_aspects_fall_back_to_sixteen_by_nine() {
        assertEquals(16.0 / 9.0, pipAspectRatio(null).toDouble(), 1e-3)
        assertEquals(16.0 / 9.0, pipAspectRatio(Double.NaN).toDouble(), 1e-3)
        assertEquals(16.0 / 9.0, pipAspectRatio(-1.0).toDouble(), 1e-3)
    }

    @Test
    fun a_common_ratio_is_kept() {
        assertEquals(16.0 / 9.0, pipAspectRatio(16.0 / 9.0).toDouble(), 1e-3)
    }
}
