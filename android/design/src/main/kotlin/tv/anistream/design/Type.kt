// Type. Show titles, and only show titles, are set in Zen Kaku Gothic New 700; everything else is
// Roboto Flex at the Material type roles, in sp so the system's font size scales it.

package tv.anistream.design

import androidx.compose.material3.Typography
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * Roboto Flex at [weight], with its optical size set to [size]: the variable font draws small text
 * open and large text tight, as its optical size axis follows the size it is set at.
 */
@OptIn(ExperimentalTextApi::class)
fun robotoFlex(weight: FontWeight, size: TextUnit): FontFamily = FontFamily(
    Font(
        R.font.roboto_flex,
        weight = weight,
        variationSettings = FontVariation.Settings(
            FontVariation.weight(weight.weight),
            FontVariation.Setting("opsz", size.value.coerceIn(8f, 144f)),
        ),
    ),
)

/** Zen Kaku Gothic New 700, for show titles. */
val ShowTitleFamily = FontFamily(Font(R.font.zen_kaku_gothic_new_bold, FontWeight.Bold))

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

/** The brief's roles, and the rest of Material's set at the same scale. */
val AnistreamType = Typography(
    displayLarge = role(57, 64, FontWeight.Normal),
    displayMedium = role(45, 52, FontWeight.Normal),
    displaySmall = role(36, 44, FontWeight.Normal),
    headlineLarge = role(32, 40, FontWeight.Normal),
    /** The large top app bar's title ("Continue"). */
    headlineMedium = role(28, 36, FontWeight.Normal),
    headlineSmall = role(24, 32, FontWeight.Normal),
    /** The small top app bar, sheet and dialog titles. */
    titleLarge = role(22, 28, FontWeight.Normal),
    /** List item headlines, section headers. */
    titleMedium = role(16, 24, FontWeight.Medium),
    titleSmall = role(14, 20, FontWeight.Medium),
    /** Body text; supporting text when it leads. */
    bodyLarge = role(16, 24, FontWeight.Normal),
    /** Supporting text, snackbar messages. */
    bodyMedium = role(14, 20, FontWeight.Normal),
    /** Notes and the legal line. */
    bodySmall = role(12, 16, FontWeight.Normal),
    /** Buttons, menu items, tabs. */
    labelLarge = role(14, 20, FontWeight.Medium),
    /** Navigation bar and rail labels, chips. */
    labelMedium = role(12, 16, FontWeight.Medium),
    /** Metadata lines in capitals, tracked 0.06 em. */
    labelSmall = role(11, 16, FontWeight.Medium, tracking = 0.06.em),
)

/** Show titles: 22 on title pages and the Continue hero, 16 in rows and cards, 14 under posters. */
object ShowTitle {
    private fun title(size: Int, line: Int) = TextStyle(
        fontFamily = ShowTitleFamily,
        fontWeight = FontWeight.Bold,
        fontSize = size.sp,
        lineHeight = line.sp,
        lineHeightStyle = Lines,
    )

    val page = title(22, 28)
    val row = title(16, 24)
    val caption = title(14, 20)
}

/** Tabular figures, for timecodes, counts, scores and countdowns. */
fun TextStyle.tabular(): TextStyle = copy(fontFeatureSettings = "tnum")
