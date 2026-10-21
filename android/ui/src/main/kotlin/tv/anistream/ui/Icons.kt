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

    val Back = stroked("Back", "M19 12 H5 M11 6 L5 12 L11 18")
    val More = ImageVector.Builder(
        name = "More", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (y in listOf(6f, 12f, 18f)) {
            addPath(addPathNodes("M12 ${y - 1.5f} A1.5 1.5 0 1 1 11.99 ${y - 1.5f} Z"), fill = SolidColor(Color.Black))
        }
    }.build()
    val Down = stroked("Down", "M7 10 L12 15 L17 10")

    /** A menu's own arrow, the filled triangle a dropdown row ends with; tinted like the rest. */
    val MenuArrow = ImageVector.Builder(
        name = "MenuArrow", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply { addPath(pathData = addPathNodes("M7 10 L12 15 L17 10 Z"), fill = SolidColor(Color.Black)) }.build()
    val Chevron = stroked("Chevron", "M10 7 L15 12 L10 17")
    val Check = stroked("Check", "M5 12.5 L10 17.5 L19 7")
    val Trailer = stroked("Trailer", "M8 6.5 V17.5 L17 12 Z")
    val Hidden = stroked(
        "Hidden",
        "M3 3 L21 21",
        "M10.6 6.1 C11.06 6.04 11.53 6 12 6 C16.5 6 20 9.5 21.5 12 C20.9 13 20 14.2 18.8 15.2",
        "M15.4 17.3 C14.3 17.75 13.2 18 12 18 C7.5 18 4 14.5 2.5 12 C3.3 10.6 4.6 9 6.3 7.8",
        "M9.9 9.9 A3 3 0 0 0 14.1 14.1",
    )
    val Settings = stroked(
        "Settings",
        "M12 9 A3 3 0 1 1 11.99 9 Z",
        "M12 2.5 V5 M12 19 V21.5 M21.5 12 H19 M5 12 H2.5 M18.7 5.3 L17 7 M7 17 L5.3 18.7 M18.7 18.7 L17 17 M7 7 L5.3 5.3",
    )
    val HideRow = stroked("HideRow", "M5 12 H19", "M12 3 A9 9 0 1 1 11.99 3 Z")
    val External = stroked("External", "M14 4 H20 V10 M20 4 L11 13", "M18 14 V19 A1 1 0 0 1 17 20 H5 A1 1 0 0 1 4 19 V7 A1 1 0 0 1 5 6 H10")
    val Restart = stroked("Restart", "M4 12 A8 8 0 1 0 6.3 6.3", "M4 4 V8 H8")
    val Downloads = stroked("Downloads", "M12 4 V15 M7 10 L12 15 L17 10", "M5 20 H19")
    val Sort = stroked("Sort", "M4 7 H20 M7 12 H17 M10 17 H14")
    val Clear = stroked("Clear", "M6 6 L18 18 M18 6 L6 18")
    val Menu = stroked("Menu", "M4 7 H20 M4 12 H20 M4 17 H20")

    // Settings' pages, as AndroidSettings draws them.
    val PlaybackPage = stroked("PlaybackPage", CIRCLE_9, "M10 8.5 L15.5 12 L10 15.5 Z")

    /** Notifications' bell. */
    val NotificationsPage = stroked("NotificationsPage", "M18 16 V11 A6 6 0 0 0 6 11 V16 L4.5 18.5 H19.5 Z", "M10 21 H14")
    val DownloadsPage = stroked("DownloadsPage", "M12 4 V15 M7.5 10.5 L12 15 L16.5 10.5 M5 19.5 H19")
    val SourcesPage = stroked("SourcesPage", "M9 3 V7 M15 3 V7 M6 7 H18 V11 A6 6 0 0 1 6 11 Z M12 17 V21")
    val AccountsPage = stroked("AccountsPage", "M12 4 A4 4 0 1 1 11.99 4 Z", "M4.5 20 C5.5 15.5 18.5 15.5 19.5 20")
    val About = ImageVector.Builder(
        name = "About", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        addPath(addPathNodes(CIRCLE_9), stroke = SolidColor(Color.Black), strokeLineWidth = 2f)
        addPath(addPathNodes("M12 11 V16.5"), stroke = SolidColor(Color.Black), strokeLineWidth = 2f, strokeLineCap = StrokeCap.Round)
        addPath(addPathNodes("M12 6.65 A1.1 1.1 0 1 1 11.99 6.65 Z"), fill = SolidColor(Color.Black))
    }.build()

    /** The drawer's Providers and Plugin stores (AT1). */
    val Providers = stroked("Providers", "M8.5 3 V7 M15.5 3 V7 M6 7 H18 V11 C18 14.5 15.3 17 12 17 C8.7 17 6 14.5 6 11 Z M12 17 V21")
    val Stores = stroked("Stores", "M5 8 H19 L17.8 20 H6.2 Z M8.5 8 C8.5 3.5 15.5 3.5 15.5 8")

    /** Downloads' play, pause and resume. */
    val PlayRow = stroked("PlayRow", "M8.5 6 V18 L18 12 Z")
    val Pause = stroked("Pause", "M9 5.5 V18.5 M15 5.5 V18.5")
    val Resume = stroked("Resume", "M12 4 V14.5 M7.5 10 L12 14.5 L16.5 10 M5 19.5 H19")

    /** Providers' Reorder, and the handle a source is dragged by. */
    val Reorder = stroked("Reorder", "M8 19 V5 M4 9 L8 5 L12 9 M16 5 V19 M12 15 L16 19 L20 15")
    val DragHandle = stroked("DragHandle", "M5 9.5 H19 M5 14.5 H19")

    /** A store's key matching its invite. */
    val Shield = stroked("Shield", "M12 3 L19 6 V11 C19 15.5 16 19 12 21 C8 19 5 15.5 5 11 V6 Z", "M9 12 L11.2 14.2 L15.5 9.8")

    /** A menu row's indicator: the filled triangle of a dropdown. */
    val DropDown = ImageVector.Builder(
        name = "DropDown", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        addPath(
            addPathNodes("M7.5 10 L12 14.5 L16.5 10 Z"),
            fill = SolidColor(Color.Black),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.5f,
            strokeLineJoin = StrokeJoin.Round,
        )
    }.build()

    /** The rail's menu button while the drawer is open. */
    val MenuOpen = stroked("MenuOpen", "M4 7 H15 M4 12 H12 M4 17 H15", "M20 9 L17 12 L20 15")

    val Account = stroked(
        "Account",
        CIRCLE_9,
        "M12 7 A3 3 0 1 1 11.99 7 Z",
        "M6.7 18.4 C8.1 16.6 9.9 15.8 12 15.8 C14.1 15.8 15.9 16.6 17.3 18.4",
    )
}
