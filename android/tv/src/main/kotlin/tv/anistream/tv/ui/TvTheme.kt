// The TV theme: tv-material's MaterialTheme on the values :design holds, the Obi & Silk colours
// with the focus roles TV adds, the type set larger for distance, and the canvas the boards are
// drawn on.

package tv.anistream.tv.ui

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.LocalTextStyle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Shapes
import androidx.tv.material3.Typography
import androidx.tv.material3.darkColorScheme
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitleFamily
import tv.anistream.design.robotoFlex

/**
 * The fixed scheme: dark only, no dynamic colour. Focus is silk (a filled container, or the ring on
 * art), selection is amber's low tone, and the one action is amber.
 */
val TvColors = darkColorScheme(
    primary = ObiSilk.Amber,
    onPrimary = ObiSilk.Ground,
    primaryContainer = ObiSilk.AmberContainer,
    onPrimaryContainer = ObiSilk.OnAmberContainer,
    inversePrimary = ObiSilk.AmberContainer,
    secondary = ObiSilk.Water,
    onSecondary = ObiSilk.Ground,
    secondaryContainer = ObiSilk.RaisedSelected,
    onSecondaryContainer = ObiSilk.Silk,
    tertiary = ObiSilk.Water,
    onTertiary = ObiSilk.Ground,
    tertiaryContainer = ObiSilk.RaisedSelected,
    onTertiaryContainer = ObiSilk.Silk,
    background = ObiSilk.Ground,
    onBackground = ObiSilk.Silk,
    surface = ObiSilk.Ground,
    onSurface = ObiSilk.Silk,
    surfaceVariant = ObiSilk.RaisedSelected,
    onSurfaceVariant = ObiSilk.Haze,
    surfaceTint = ObiSilk.Ground,
    inverseSurface = ObiSilk.Silk,
    inverseOnSurface = ObiSilk.Ground,
    error = ObiSilk.Vermilion,
    onError = ObiSilk.Ground,
    errorContainer = ObiSilk.RaisedHigh,
    onErrorContainer = ObiSilk.Silk,
    border = ObiSilk.Silk,
    borderVariant = ObiSilk.Rule,
    scrim = Color.Black,
)

/** Lines centred in their height, as the boards set them, with nothing trimmed. */
private val Lines = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None)

private fun role(size: Int, line: Int, weight: FontWeight, tracking: TextUnit = 0.sp) = TextStyle(
    fontFamily = robotoFlex(weight, size.sp),
    fontWeight = weight,
    fontSize = size.sp,
    lineHeight = line.sp,
    letterSpacing = tracking,
    lineHeightStyle = Lines,
)

/** The brief's TV roles, in sp on the 960 × 540 canvas. Nothing is smaller than 12. */
val TvType = Typography(
    displayLarge = role(57, 64, FontWeight.Normal),
    displayMedium = role(45, 52, FontWeight.Normal),
    displaySmall = role(36, 44, FontWeight.Normal),
    headlineLarge = role(32, 40, FontWeight.Normal),
    headlineMedium = role(28, 36, FontWeight.Normal),
    headlineSmall = role(24, 32, FontWeight.Normal),
    titleLarge = role(22, 28, FontWeight.Medium),
    titleMedium = role(18, 24, FontWeight.Medium),
    titleSmall = role(16, 24, FontWeight.Medium),
    bodyLarge = role(16, 24, FontWeight.Normal),
    bodyMedium = role(14, 20, FontWeight.Normal),
    bodySmall = role(12, 16, FontWeight.Normal),
    labelLarge = role(16, 20, FontWeight.Medium),
    labelMedium = role(14, 18, FontWeight.Medium),
    labelSmall = role(12, 16, FontWeight.Medium, tracking = 0.06.em),
)

/** Show titles in Zen Kaku Gothic New 700, at the TV's sizes. */
object TvTitle {
    private fun title(size: Int, line: Int) = TextStyle(
        fontFamily = ShowTitleFamily,
        fontWeight = FontWeight.Bold,
        fontSize = size.sp,
        lineHeight = line.sp,
        lineHeightStyle = Lines,
    )

    /** The title page and the Continue hero. */
    val page = title(36, 44)

    /** The player's header and the Up Next card. */
    val player = title(22, 28)

    /** Lists. */
    val list = title(16, 20)

    /** Under posters and cards, two lines then an ellipsis. */
    val card = title(14, 18)
}

val TvShapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(12.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

@Composable
fun TvTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = TvColors, shapes = TvShapes, typography = TvType) {
        CompositionLocalProvider(
            LocalContentColor provides ObiSilk.Silk,
            LocalTextStyle provides TvType.bodyLarge,
            content = content,
        )
    }
}

/**
 * The canvas: 960 × 540 dp, the UI at 1080p on density 2. Text, focusable items and their rings
 * stay inside the safe area; art and backgrounds bleed to the edges.
 */
object Canvas {
    /** Google's 5 %: 48 left and right. */
    val SafeX = 48.dp

    /** 27 top and bottom. */
    val SafeY = 27.dp

    /** Where every screen's rows, grids and headings start. */
    val Keyline = 124.dp

    /** The collapsed rail, from the screen's edge. */
    val Rail = 96.dp

    /** The expanded rail, a drawer over the content. */
    val Drawer = 240.dp

    /** Between cards, and from a row's header to its cards. */
    val CardGap = 16.dp
    val HeaderGap = 12.dp

    /** Between rows. */
    val RowGap = 28.dp

    /** Above and below the art inside a row, so the focus scale and ring are never clipped. */
    val FocusRoom = 6.dp

    /** A poster: 6 columns of 118 between the keyline and the safe area's right edge. */
    val PosterWidth = 118.dp
    val PosterHeight = 177.dp

    /** A 16:9 card: 4 columns of 185. */
    val CardWidth = 185.dp
    val CardHeight = 104.dp

    /** The safe area's right edge, from the screen's left. */
    val SafeRight = 912.dp
}
