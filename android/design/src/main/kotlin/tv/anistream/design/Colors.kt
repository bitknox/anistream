// The Obi & Silk palette as a fixed Material colour scheme. Dark first, and only dark for now: no
// Android board draws the light appearance yet. Dynamic colour from the wallpaper is never used.

package tv.anistream.design

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp

/** The palette's own names, for what the colour roles do not name. */
object ObiSilk {
    /** 藍 ground: screens, the rail, top app bars at rest. */
    val Ground = Color(0xFF161A2E)

    /** surfaceContainerLow: the expanded rail, as a modal drawer. */
    val RaisedLow = Color(0xFF1C2034)

    /** Raised: cards, sheets, the navigation bar, scrolled top app bars. */
    val Raised = Color(0xFF222638)

    /** surfaceContainerHigh: menus, dialogs, the search bar, snackbars. */
    val RaisedHigh = Color(0xFF2A2F45)

    /** Raised selected: the text field fill and the tonal button. */
    val RaisedSelected = Color(0xFF343A57)

    /** 絹 silk: text, icons and text buttons. */
    val Silk = Color(0xFFE4E0D6)

    /** 霞 haze: supporting text, inactive icons. */
    val Haze = Color(0xFF9298B5)

    /** 琥珀 amber: the one filled action, progress, the current selection, switches on. */
    val Amber = Color(0xFFF2A64B)

    /** Amber at low tone: selection containers. */
    val AmberContainer = Color(0xFF4A3A24)
    val OnAmberContainer = Color(0xFFF7C98B)

    /** 水 water: states, such as a new episode, airing, synced, downloading. */
    val Water = Color(0xFF7FC7D9)

    /** 朱 vermilion: errors only. */
    val Vermilion = Color(0xFFE06B54)

    /** Rule: the progress track, outlined buttons and chips. */
    val Rule = Color(0xFF434653)

    /** Faint rule: hairlines and dividers. */
    val FaintRule = Color(0xFF2F3242)
}

/**
 * The fixed scheme, from the Android brief's role table. Every role is set, so nothing falls back
 * to the baseline purple: the ones the table leaves out follow AndroidFoundations.
 */
val ObiSilkDark: ColorScheme = darkColorScheme(
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
    // Tonal elevation is off, and a tint of the surface itself would change nothing were it on.
    surfaceTint = ObiSilk.Ground,
    inverseSurface = ObiSilk.Silk,
    inverseOnSurface = ObiSilk.Ground,
    error = ObiSilk.Vermilion,
    onError = ObiSilk.Ground,
    errorContainer = lerp(ObiSilk.Ground, ObiSilk.Vermilion, 0.3f),
    onErrorContainer = ObiSilk.Silk,
    outline = ObiSilk.Rule,
    outlineVariant = ObiSilk.FaintRule,
    scrim = Color.Black,
    surfaceBright = ObiSilk.RaisedSelected,
    surfaceDim = ObiSilk.Ground,
    surfaceContainerLowest = ObiSilk.Ground,
    surfaceContainerLow = ObiSilk.RaisedLow,
    surfaceContainer = ObiSilk.Raised,
    surfaceContainerHigh = ObiSilk.RaisedHigh,
    surfaceContainerHighest = ObiSilk.RaisedSelected,
)
