// The system's caption style, for the subtitles that have no style of their own.
//
// SRT, WebVTT and the other plain-text kinds take their look from mpv's sub-* options. With
// "Subtitles follow your system caption style" on (Settings › Playback, on by default), those come
// from the style the viewer set in Accessibility › Caption preferences, through CaptioningManager:
// the typeface, the size, the text's colour, the background behind the letters, the window behind
// the lines, and the edge. Only while captions are on there: with them off, Android's style is its
// stock white-on-black preset, which nobody chose, and the player keeps its own look (as ExoPlayer
// does).
//
// ASS subtitles keep the look their authors gave them. mpv applies the sub-* style options to them
// only with sub-ass-override at force; the player leaves it at mpv's default, scale, under which ASS
// styling stands and sub-pos still lifts ASS dialogue above the controls (no would stop that too).

package tv.anistream.player

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.view.accessibility.CaptioningManager
import java.util.Locale

/**
 * A caption style, and the mpv options that draw plain-text subtitles in it. Colours are ARGB, as
 * Android gives them.
 */
data class CaptionStyle(
    /** The typeface's generic family as Android names it ("serif", "casual"); null for the default. */
    val font: String?,
    /** The text's size against the normal one. */
    val scale: Float,
    val text: Int,
    /** Behind each line's letters. */
    val background: Int,
    /** One box behind all the lines. */
    val window: Int,
    val edge: Edge,
    /** The outline's or the shadow's colour. */
    val edgeColor: Int,
) {
    enum class Edge { NONE, OUTLINE, DROP_SHADOW, RAISED, DEPRESSED }

    /** The options that draw plain-text subtitles in this style. */
    fun mpvOptions(): Map<String, String> {
        val options = mutableMapOf(
            "sub-font" to (font ?: DEFAULT_FONT),
            // mpv's 38 is the size at a 720-line window, as its own scaling goes.
            "sub-font-size" to number(38.0 * scale.toDouble().coerceIn(0.25, 2.0), 1),
            "sub-color" to mpvColor(text),
        )
        // The outline and the shadow for the edge; a box below, where the style has one.
        var outline = 0.0
        var outlineColor = edgeColor
        var shadow = 0.0
        var shadowColor = edgeColor
        when (edge) {
            Edge.NONE -> {}
            Edge.OUTLINE -> outline = 1.65
            Edge.DROP_SHADOW -> shadow = 2.0
            // Lit from above: the edge's colour below and to the right.
            Edge.RAISED -> shadow = 1.0
            // Pressed in: a light edge below and to the right.
            Edge.DEPRESSED -> {
                shadow = 1.0
                shadowColor = 0x99FFFFFF.toInt()
            }
        }
        when {
            alpha(background) > 0 -> {
                // A box tight around each line, in the background's colour: libass's opaque box is
                // drawn by the outline, so the edge gives way to it.
                options["sub-border-style"] = "opaque-box"
                outline = 3.0
                outlineColor = background
                shadow = 0.0
            }
            alpha(window) > 0 -> {
                // One box behind all the lines, the shadow's offset its margin; the outline as the
                // edge has it.
                options["sub-border-style"] = "background-box"
                shadow = 6.0
                shadowColor = window
            }
            else -> options["sub-border-style"] = "outline-and-shadow"
        }
        options["sub-outline-size"] = number(outline, 2)
        options["sub-outline-color"] = mpvColor(outlineColor)
        options["sub-shadow-offset"] = number(shadow, 2)
        options["sub-back-color"] = mpvColor(shadowColor)
        return options
    }

    companion object {
        /** mpv's own: fonts.conf has Roboto for it. */
        const val DEFAULT_FONT = "sans-serif"

        /** mpv's own look for plain-text subtitles, where the style does not follow the system. */
        val MPV_DEFAULTS: Map<String, String> = mapOf(
            "sub-font" to DEFAULT_FONT,
            "sub-font-size" to "38.0",
            "sub-color" to "#FFFFFFFF",
            "sub-border-style" to "outline-and-shadow",
            "sub-outline-size" to "1.65",
            "sub-outline-color" to "#FF000000",
            "sub-shadow-offset" to "0.00",
            "sub-back-color" to "#AF000000",
        )

        /**
         * The families Android's caption settings offer, by the names fonts.conf gives them
         * (MpvSetup.writeFontsConf), the default first.
         */
        val FAMILIES = listOf(
            "sans-serif", "sans-serif-condensed", "sans-serif-monospace", "serif", "serif-monospace",
            "casual", "cursive", "sans-serif-smallcaps",
        )

        /** mpv's form of an ARGB colour: `#AARRGGBB`. */
        fun mpvColor(argb: Int): String = String.format(Locale.ROOT, "#%08X", argb)

        private fun number(value: Double, places: Int) = String.format(Locale.ROOT, "%.${places}f", value)

        /** An ARGB colour's alpha, 0 to 255. Plain arithmetic, so the mapping runs on the JVM. */
        private fun alpha(argb: Int) = (argb ushr 24) and 0xFF

        /**
         * The style the viewer set, while captions are on in Accessibility; null while they are off.
         */
        fun system(context: Context): CaptionStyle? {
            val manager = context.getSystemService(CaptioningManager::class.java) ?: return null
            if (!manager.isEnabled) return null
            val style = manager.userStyle
            // The settings' typeface is made by name, and Android keeps one per name.
            val typeface = style.typeface
            val family = FAMILIES.firstOrNull { typeface != null && Typeface.create(it, Typeface.NORMAL) == typeface }
            return CaptionStyle(
                font = family?.takeIf { it != DEFAULT_FONT },
                scale = manager.fontScale,
                text = if (style.hasForegroundColor()) style.foregroundColor else Color.WHITE,
                background = if (style.hasBackgroundColor()) style.backgroundColor else Color.TRANSPARENT,
                window = if (style.hasWindowColor()) style.windowColor else Color.TRANSPARENT,
                edge = when (style.edgeType) {
                    CaptioningManager.CaptionStyle.EDGE_TYPE_OUTLINE -> Edge.OUTLINE
                    CaptioningManager.CaptionStyle.EDGE_TYPE_DROP_SHADOW -> Edge.DROP_SHADOW
                    CaptioningManager.CaptionStyle.EDGE_TYPE_RAISED -> Edge.RAISED
                    CaptioningManager.CaptionStyle.EDGE_TYPE_DEPRESSED -> Edge.DEPRESSED
                    else -> Edge.NONE
                },
                edgeColor = if (style.hasEdgeColor()) style.edgeColor else Color.BLACK,
            )
        }
    }
}
