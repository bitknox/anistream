package tv.anistream.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The bottom bar's labels: one line at normal sizes, shrunk to fit before anything breaks; at
 * large scales a label that still does not fit breaks at a space; a single long word ends in an
 * ellipsis.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class BarLabelTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun layout(text: String, scale: Float, width: Int = 72): TextLayoutResult {
        compose.setContent {
            CompositionLocalProvider(LocalDensity provides Density(2f, scale)) {
                MaterialTheme { Row(Modifier.width(width.dp)) { BarLabel(text) } }
            }
        }
        compose.waitForIdle()
        val results = mutableListOf<TextLayoutResult>()
        compose.onNodeWithText(text).fetchSemanticsNode().config[androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult]
            .action?.invoke(results)
        return results.single()
    }

    @Test
    fun a_normal_scale_keeps_one_line() {
        assertEquals(1, layout("Calendar", 1.0f).lineCount)
    }

    @Test
    fun a_label_shrinks_before_it_breaks() {
        val shrunk = layout("Seasonal", 1.6f)
        assertEquals(1, shrunk.lineCount)
    }

    @Test
    fun a_label_that_cannot_fit_breaks_at_its_space() {
        val broken = layout("Plugin stores", 2.0f, width = 60)
        assertEquals(2, broken.lineCount)
        assertEquals(false, broken.isLineEllipsized(0))
        assertEquals(false, broken.isLineEllipsized(1))
    }

    @Test
    fun a_single_long_word_ends_in_an_ellipsis() {
        val cut = layout("Supercalifragilistic", 2.0f, width = 60)
        assertEquals(1, cut.lineCount)
        assertTrue(cut.isLineEllipsized(0))
    }
}
