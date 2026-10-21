// The Material 3 Expressive shape scale, and the corners of art.

package tv.anistream.design

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * 4, 8, 12, 16 and 28. Cards 16; sheets 28 on the top corners; dialogs 28; buttons, the search bar
 * and switches full; chips 8; menus and snackbars 4.
 */
val AnistreamShapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(12.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

/** The corner radii of art: a cover rounds 6 up to 40 wide and 8 above, a banner 16. */
object ArtShape {
    fun coverRadius(width: Dp): Dp = if (width <= 40.dp) 6.dp else 8.dp

    fun cover(width: Dp) = RoundedCornerShape(coverRadius(width))

    val bannerRadius = 16.dp
    val banner = RoundedCornerShape(bannerRadius)
}
