// The player's icons, as AndroidPlayer draws them: 24 dp, stroked at 2 with round caps and joins.

package tv.anistream.player.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

internal object PlayerIcons {
    private fun icon(name: String, stroked: List<String>, filled: List<String> = emptyList()) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in stroked) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
        for (path in filled) addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black))
    }.build()

    val Back = icon("Back", listOf("M19 12 H5.5 M11 6 L5 12 L11 18"))

    val Subtitles = icon(
        "Subtitles",
        listOf("M6 5 H18 A3 3 0 0 1 21 8 V16 A3 3 0 0 1 18 19 H6 A3 3 0 0 1 3 16 V8 A3 3 0 0 1 6 5 Z", "M7 12 H9.5 M12.5 12 H17 M7 15.5 H13 M16 15.5 H17"),
    )

    val Episodes = icon("Episodes", listOf("M4 6.5 H17 M4 11.5 H17 M4 16.5 H10"), listOf("M14 14.5 V20 L19 17.25 Z"))

    /** Back 10 s: the turning arrow, with its "10" drawn over it. */
    val Replay = icon("Replay", listOf("M12 5 A8 8 0 1 1 4.6 10", "M14.5 2.5 L12 5 L14.5 7.5"))

    val Forward = icon("Forward", listOf("M12 5 A8 8 0 1 0 19.4 10", "M9.5 2.5 L12 5 L9.5 7.5"))

    val Pause = icon("Pause", emptyList(), listOf("M7.5 5 H8.5 A1.5 1.5 0 0 1 10 6.5 V17.5 A1.5 1.5 0 0 1 8.5 19 H7.5 A1.5 1.5 0 0 1 6 17.5 V6.5 A1.5 1.5 0 0 1 7.5 5 Z", "M15.5 5 H16.5 A1.5 1.5 0 0 1 18 6.5 V17.5 A1.5 1.5 0 0 1 16.5 19 H15.5 A1.5 1.5 0 0 1 14 17.5 V6.5 A1.5 1.5 0 0 1 15.5 5 Z"))

    val Play = icon("Play", emptyList(), listOf("M8 5.5 V18.5 L18.5 12 Z"))

    val SkipNext = icon("SkipNext", listOf("M17.5 6 V18"), listOf("M6 6.5 V17.5 L14 12 Z"))
}
