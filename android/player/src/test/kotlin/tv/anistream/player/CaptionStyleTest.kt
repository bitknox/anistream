// Plain-text subtitles in the system's caption style: the style as mpv's options, the style read
// from Android's caption settings, and the player taking it as the setting and the style change.
// Made-up styles and Robolectric's settings throughout; nothing reads or changes a device's own.

package tv.anistream.player

import android.provider.Settings
import android.view.accessibility.CaptioningManager
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

class CaptionStyleTest {
    /** Yellow on a blue box, half again as large, in serif. */
    private val yellowOnBlue = CaptionStyle(
        font = "serif", scale = 1.5f,
        text = 0xFFFFFF00.toInt(), background = 0xBF0000FF.toInt(), window = 0,
        edge = CaptionStyle.Edge.OUTLINE, edgeColor = 0xFF000000.toInt(),
    )

    @Test
    fun aBackgroundBehindTheLettersIsATightBoxInItsColour() {
        val options = yellowOnBlue.mpvOptions()
        assertEquals("serif", options["sub-font"])
        assertEquals("57.0", options["sub-font-size"])
        assertEquals("#FFFFFF00", options["sub-color"])
        // libass draws the tight box with the outline, so the box wins over the edge.
        assertEquals("opaque-box", options["sub-border-style"])
        assertEquals("#BF0000FF", options["sub-outline-color"])
        assertEquals("0.00", options["sub-shadow-offset"])
    }

    @Test
    fun aWindowIsOneBoxBehindAllTheLines() {
        val options = yellowOnBlue.copy(background = 0, window = 0x80000000.toInt(), edge = CaptionStyle.Edge.NONE).mpvOptions()
        assertEquals("background-box", options["sub-border-style"])
        assertEquals("#80000000", options["sub-back-color"])
        assertEquals("6.00", options["sub-shadow-offset"])
        assertEquals("0.00", options["sub-outline-size"])
    }

    @Test
    fun edgesWithoutABoxAreTheOutlineOrTheShadowInTheEdgesColour() {
        val plain = yellowOnBlue.copy(background = 0, edgeColor = 0xFF102030.toInt())
        var options = plain.mpvOptions()
        assertEquals("outline-and-shadow", options["sub-border-style"])
        assertEquals("1.65", options["sub-outline-size"])
        assertEquals("#FF102030", options["sub-outline-color"])
        options = plain.copy(edge = CaptionStyle.Edge.DROP_SHADOW).mpvOptions()
        assertEquals("0.00", options["sub-outline-size"])
        assertEquals("2.00", options["sub-shadow-offset"])
        assertEquals("#FF102030", options["sub-back-color"])
        options = plain.copy(edge = CaptionStyle.Edge.DEPRESSED).mpvOptions()
        assertEquals("#99FFFFFF", options["sub-back-color"])
    }

    @Test
    fun theDefaultTypefaceKeepsThePlayersFontAndSizesStayInRange() {
        val options = yellowOnBlue.copy(font = null, scale = 9f).mpvOptions()
        assertEquals(CaptionStyle.DEFAULT_FONT, options["sub-font"])
        assertEquals("76.0", options["sub-font-size"])
        assertEquals("9.5", yellowOnBlue.copy(scale = 0.1f).mpvOptions()["sub-font-size"])
    }

    /** Turning the setting off sets back every option the style set. */
    @Test
    fun mpvsOwnLookCoversEveryOptionTheStyleSets() {
        assertEquals(yellowOnBlue.mpvOptions().keys, CaptionStyle.MPV_DEFAULTS.keys)
    }
}

/** The style as Android's caption settings hold it, in Robolectric's settings. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SystemCaptionStyleTest {
    private val context = RuntimeEnvironment.getApplication()

    private fun put(name: String, value: Int) = Settings.Secure.putInt(context.contentResolver, "accessibility_captioning_$name", value)

    @Test
    fun withCaptionsOffThePlayerKeepsItsOwnLook() {
        put("enabled", 0)
        assertNull(CaptionStyle.system(context))
    }

    @Test
    fun withCaptionsOnTheViewersStyleIsRead() {
        put("enabled", 1)
        // A custom style rather than a preset.
        put("preset", -1)
        put("foreground_color", 0xFFFFFF00.toInt())
        put("background_color", 0)
        put("window_color", 0x80000000.toInt())
        put("edge_type", CaptioningManager.CaptionStyle.EDGE_TYPE_DROP_SHADOW)
        put("edge_color", 0xFF000000.toInt())
        Settings.Secure.putString(context.contentResolver, "accessibility_captioning_typeface", "serif")
        Settings.Secure.putFloat(context.contentResolver, "accessibility_captioning_font_scale", 1.5f)
        val style = CaptionStyle.system(context)
        assertEquals(
            CaptionStyle(
                font = "serif", scale = 1.5f, text = 0xFFFFFF00.toInt(), background = 0, window = 0x80000000.toInt(),
                edge = CaptionStyle.Edge.DROP_SHADOW, edgeColor = 0xFF000000.toInt(),
            ),
            style,
        )
    }
}

class CaptionStylePlayerTest {
    private val yellow = CaptionStyle(
        font = null, scale = 1f, text = 0xFFFFFF00.toInt(), background = 0, window = 0,
        edge = CaptionStyle.Edge.OUTLINE, edgeColor = 0xFF000000.toInt(),
    )

    /** The values the player set on mpv for [property], in order. */
    private fun sets(rig: Rig, property: String) =
        rig.mpv.calls.filterIsInstance<MpvCall.Set>().filter { it.property == property }.map { (it.value as MpvValue.Text).value }

    @Test
    fun mpvStartsInTheStyleOrInItsOwnLookWithCaptionsOff() = runTest {
        val rig = Rig(this)
        rig.opened()
        assertEquals(CaptionStyle.MPV_DEFAULTS, rig.mpv.begunWith)
        rig.captionStyle = yellow
        rig.ops(1u, tv.anistream.engine.model.PlayerOp.Close)
        rig.opened(2u)
        assertEquals("#FFFFFF00", rig.mpv.begunWith["sub-color"])
    }

    @Test
    fun aChangeReachesTheMpvPlayingNowAndTheSettingTurnsItOff() = runTest {
        val rig = Rig(this)
        rig.opened()
        // The viewer turns captions on in Accessibility, in yellow.
        rig.captionStyle = yellow
        rig.model.captionStyleChanged()
        rig.settle()
        assertEquals(listOf("#FFFFFF00"), sets(rig, "sub-color"))
        // And turns the app's setting off: mpv's own look again.
        rig.model.followsSystemCaptions = false
        rig.settle()
        assertEquals(listOf("#FFFFFF00", "#FFFFFFFF"), sets(rig, "sub-color"))
        assertEquals("outline-and-shadow", sets(rig, "sub-border-style").last())
    }
}
