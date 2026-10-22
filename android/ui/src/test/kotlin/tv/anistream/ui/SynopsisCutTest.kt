package tv.anistream.ui

import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import tv.anistream.engine.model.Event
import tv.anistream.testkit.Scene

/**
 * The synopsis's "… More" cut, worked out from one layout of the whole text, against the search
 * over every length it replaces: the same cut for every fixture synopsis, at the widths of a phone
 * and a tablet, at the system's text sizes.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class SynopsisCutTest {
    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()

    /** The search the page used to run: the longest start that fits with the ending. */
    private fun searched(text: String, width: Int, style: TextStyle, measurer: TextMeasurer): String? {
        fun fits(candidate: String) =
            !measurer.measure(candidate, style, maxLines = 2, constraints = Constraints(maxWidth = width)).hasVisualOverflow
        if (fits(text)) return null
        var low = 0
        var high = text.length
        while (low < high) {
            val mid = (low + high + 1) / 2
            if (fits(text.take(mid).trimEnd() + "… More ")) low = mid else high = mid - 1
        }
        return text.take(low).trimEnd() + "…"
    }

    @Test
    fun cuts_where_the_search_did() {
        val synopses = Scene.named("watching").events
            .filterIsInstance<Event.Detail>()
            .map { synopsisWithoutSpoilers(it.v1.synopsis) }
            .filter { it.isNotEmpty() }
            .distinct()
        val texts = synopses + synopses.map { it + " " + it } + "A short one."
        var compared = 0
        for (scale in listOf(1f, 1.3f, 2f)) {
            val measurer = TextMeasurer(createFontFamilyResolver(context), Density(2.625f, scale), LayoutDirection.Ltr)
            val style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.25.sp)
            for (width in listOf(380, 640, 997, 1280)) {
                for (text in texts) {
                    assertEquals("$width px at $scale: $text", searched(text, width, style, measurer), synopsisCut(text, width, style, measurer))
                    compared++
                }
            }
        }
        assert(compared > 0)
    }
}
