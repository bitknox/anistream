// The icons the boards draw: 24 dp, stroked at 2 with round caps and joins, and a filled form for a
// selected navigation destination.

package tv.anistream.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp
import tv.anistream.design.ObiSilk

internal object Icons {
    private fun stroked(name: String, vararg paths: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in paths) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    /** Filled shapes with marks cut into them in the indicator's colour; drawn untinted. */
    private fun filled(name: String, fill: String, marks: List<String>, markStroke: Float = 0f) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        addPath(pathData = addPathNodes(fill), fill = SolidColor(ObiSilk.OnAmberContainer))
        for (mark in marks) {
            addPath(
                pathData = addPathNodes(mark),
                fill = if (markStroke == 0f) SolidColor(ObiSilk.AmberContainer) else null,
                stroke = SolidColor(ObiSilk.AmberContainer),
                strokeLineWidth = if (markStroke == 0f) 1.5f else markStroke,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    private const val CIRCLE_9 = "M12 3 A9 9 0 1 1 11.99 3 Z"
    private const val CIRCLE_10 = "M12 2 A10 10 0 1 1 11.99 2 Z"

    val Continue = stroked("Continue", CIRCLE_9, "M10 8.5 V15.5 L15.5 12 Z")
    val ContinueSelected = filled("ContinueSelected", CIRCLE_10, listOf("M10 8.5 V15.5 L15.5 12 Z"))

    private const val CALENDAR_BODY = "M7 5 H17 A3 3 0 0 1 20 8 V17 A3 3 0 0 1 17 20 H7 A3 3 0 0 1 4 17 V8 A3 3 0 0 1 7 5 Z"
    val Calendar = stroked("Calendar", CALENDAR_BODY, "M4 10 H20 M8.5 3 V6.5 M15.5 3 V6.5")
    val CalendarSelected = filled("CalendarSelected", CALENDAR_BODY, listOf("M4 10 H20"), markStroke = 2f)

    private const val LEAF = "M5 19 C5 11 11 5 19 5 C19 13 13 19 5 19 Z"
    val Seasonal = stroked("Seasonal", LEAF, "M5 19 L12.5 11.5")
    val SeasonalSelected = filled("SeasonalSelected", LEAF, listOf("M5 19 L12.5 11.5"), markStroke = 2f)

    val Library = stroked("Library", "M5.5 4.5 V19.5 M10 4.5 V19.5 M14 5.5 L18.5 19")
    val Search = stroked("Search", "M10.5 4.5 A6 6 0 1 1 10.49 4.5 Z", "M15 15 L19.5 19.5")

    /** The play glyph on the amber action, filled in the action's text colour. */
    val Play = ImageVector.Builder(
        name = "Play", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        addPath(
            pathData = addPathNodes("M8.5 6 V18 L18 12 Z"),
            fill = SolidColor(Color.Black),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 2f,
            strokeLineJoin = StrokeJoin.Round,
        )
    }.build()

    val Forward = stroked("Forward", "M5 12 H19 M13 6 L19 12 L13 18")

    val Account = stroked(
        "Account",
        CIRCLE_9,
        "M12 7 A3 3 0 1 1 11.99 7 Z",
        "M6.7 18.4 C8.1 16.6 9.9 15.8 12 15.8 C14.1 15.8 15.9 16.6 17.3 18.4",
    )
}
