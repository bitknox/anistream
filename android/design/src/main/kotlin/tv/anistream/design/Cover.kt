// The plate that stands where a cover or banner has not arrived: never a shimmer, a grey skeleton
// or a broken-image glyph, but a flat rect in a muted version of the cover's own colour, which the
// art fades in over.

package tv.anistream.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp

object CoverPlate {
    /**
     * How much of the cover's colour shows through the ground: enough to tell covers apart, little
     * enough that a column of plates reads as quiet.
     */
    const val STRENGTH = 0.4f

    /** [color] muted into the ground, in a perceptual space; the raised colour when there is none. */
    fun muted(color: Color?): Color = color?.let { lerp(ObiSilk.Ground, it, STRENGTH) } ?: ObiSilk.Raised

    /** [hex] as AniList writes it (`#e4a15d`), or `null`. */
    fun parse(hex: String?): Color? {
        val digits = hex?.trim()?.removePrefix("#") ?: return null
        if (digits.length != 6) return null
        return digits.toLongOrNull(16)?.let { Color(0xFF000000 or it) }
    }
}

/** The plate itself, filling its box. */
@Composable
fun CoverPlate(color: Color?, modifier: Modifier = Modifier) {
    Box(modifier.background(CoverPlate.muted(color)))
}
