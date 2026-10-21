// Nagare 流, the mascot: a hand-authored pixel sprite, 20 by 32, the same one the website and the
// Apple app draw. Shown in empty states, on first launch and in About; never in navigation, the
// player, or beside a show's art. Static, on the plain ground, always a whole number of device
// pixels per sprite pixel, so the pixels stay crisp.

package tv.anistream.design

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlin.math.floor
import kotlin.math.roundToInt

/** The sprite's pixels and colours. */
object NagareSprite {
    const val WIDTH = 20
    const val HEIGHT = 32

    /** A row per line, a character per pixel, `.` for none. */
    val rows = listOf(
        "........hH..........",
        ".........Hh.........",
        "........hH..........",
        "......hHHHHh........",
        ".....hHHHHHHh.......",
        "....hHHHHHHHHh......",
        "....HHHHHHHHHHh.....",
        "...hHFFFFhHHHHh.....",
        "...HFFFFFFhHHHh.....",
        "...HFEFFFFhHHHh.....",
        "...hFFFFFfhHHh......",
        ".ww.fFFFfhHHh.......",
        ".wWWAAAAAAHHh.......",
        ".wWaAAAAAAa..aAA....",
        "..w.aAAAa....aAAa...",
        "..SRCCCCCCc.........",
        "..SRCCCCCCCc........",
        "..SRCCCCCCCc........",
        "..SfFCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCcc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCcc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCcc....",
        "..SRCCCCCCCCCCCc....",
        "..S.BB......BB......",
        "....BB......BB......",
        "...BBB.....BBB......",
    )

    /** Each character's colour. */
    val colors: Map<Char, Color> = mapOf(
        'H' to Color(0xFFE8E2D4), 'h' to Color(0xFFC6BEA9), 'F' to Color(0xFFEFCFAC),
        'f' to Color(0xFFD0A883), 'E' to Color(0xFF232842), 'C' to Color(0xFF464E7C),
        'c' to Color(0xFF2E345C), 'R' to Color(0xFF68729E), 'A' to Color(0xFFF2A64B),
        'a' to Color(0xFFC57C2C), 'S' to Color(0xFF8A6844), 'W' to Color(0xFF7FC7D9),
        'w' to Color(0xFFDFF5FB), 'B' to Color(0xFF232842),
    )

    const val DESCRIPTION =
        "Nagare, the anistream mascot: a figure with a silk bob, an amber scarf and an indigo cloak, holding a staff with a water charm"
}

/**
 * Nagare at [cell] dp per sprite pixel: 4, 5, 6 or 8 on the boards, so 80 by 128 dp to 160 by 256.
 * The box is always that size. Each sprite pixel is drawn as a whole number of device pixels, the
 * most that fit [cell] dp, so where the screen's density makes [cell] dp a fraction of a pixel the
 * figure comes out a little smaller, standing at the foot of its box and centred, but never blurred.
 */
@Composable
fun Nagare(cell: Int = 6, modifier: Modifier = Modifier) {
    Canvas(
        modifier
            .size((cell * NagareSprite.WIDTH).dp, (cell * NagareSprite.HEIGHT).dp)
            .semantics {
                contentDescription = NagareSprite.DESCRIPTION
                role = Role.Image
            },
    ) {
        val pixel = maxOf(1, floor(cell.dp.toPx()).toInt())
        val left = ((size.width - pixel * NagareSprite.WIDTH) / 2).roundToInt()
        val top = (size.height - pixel * NagareSprite.HEIGHT).roundToInt()
        val square = Size(pixel.toFloat(), pixel.toFloat())
        NagareSprite.rows.forEachIndexed { y, row ->
            row.forEachIndexed { x, char ->
                val color = NagareSprite.colors[char] ?: return@forEachIndexed
                drawRect(color, topLeft = Offset((left + x * pixel).toFloat(), (top + y * pixel).toFloat()), size = square)
            }
        }
    }
}
