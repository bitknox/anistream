// The TV's buttons, tv-material's with the brief's focus values: tonal (raised fill, silk when
// focused), the one amber action (a silk ring when focused), and the text button. Pills, no glow.

package tv.anistream.tv.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ButtonDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.IconButton
import androidx.tv.material3.IconButtonDefaults
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk

private const val FOCUSED_SCALE = 1.05f

/** Disabled is 38 % opacity, still focusable where it explains itself, with the ring only. */
private const val DISABLED = 0.38f

/**
 * A tonal button, 48 high: #343A57 with a silk label, silk with a ground label when focused.
 * [height] 56 sets it level with the amber action in the same row, its label on the same line.
 */
@Composable
fun TonalButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    enabled: Boolean = true,
    height: Dp = 48.dp,
    content: @Composable RowScope.() -> Unit,
) {
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = height),
        enabled = enabled,
        scale = ButtonDefaults.scale(focusedScale = FOCUSED_SCALE),
        glow = ButtonDefaults.glow(),
        shape = ButtonDefaults.shape(CircleShape),
        colors = ButtonDefaults.colors(
            containerColor = ObiSilk.RaisedSelected,
            contentColor = ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
            disabledContainerColor = ObiSilk.RaisedSelected.copy(alpha = DISABLED),
            disabledContentColor = ObiSilk.Silk.copy(alpha = DISABLED),
        ),
        border = ButtonDefaults.border(
            focusedDisabledBorder = Border(BorderStroke(2.dp, ObiSilk.Silk), inset = 2.dp, shape = CircleShape),
        ),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp + (height - 48.dp) / 2),
    ) {
        ButtonContent(icon, content)
    }
}

/** The one amber action per screen, 56 high: amber with a ground label, a silk ring when focused. */
@Composable
fun AmberButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = TvIcons.Play,
    enabled: Boolean = true,
    content: @Composable RowScope.() -> Unit,
) {
    val ring = Border(BorderStroke(2.dp, ObiSilk.Silk), inset = 2.dp, shape = CircleShape)
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = 56.dp),
        enabled = enabled,
        scale = ButtonDefaults.scale(focusedScale = FOCUSED_SCALE),
        glow = ButtonDefaults.glow(),
        shape = ButtonDefaults.shape(CircleShape),
        colors = ButtonDefaults.colors(
            containerColor = ObiSilk.Amber,
            contentColor = ObiSilk.Ground,
            focusedContainerColor = ObiSilk.Amber,
            focusedContentColor = ObiSilk.Ground,
            pressedContainerColor = ObiSilk.Amber,
            pressedContentColor = ObiSilk.Ground,
            disabledContainerColor = ObiSilk.Amber.copy(alpha = DISABLED),
            disabledContentColor = ObiSilk.Ground,
        ),
        border = ButtonDefaults.border(focusedBorder = ring, focusedDisabledBorder = ring),
        contentPadding = PaddingValues(horizontal = 28.dp, vertical = 16.dp),
    ) {
        ButtonContent(icon, content)
    }
}

/** A 48 × 48 tonal icon button, its label said to TalkBack. */
@Composable
fun TonalIconButton(icon: ImageVector, label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    IconButton(
        onClick = onClick,
        modifier = modifier.size(48.dp),
        scale = IconButtonDefaults.scale(focusedScale = FOCUSED_SCALE),
        glow = IconButtonDefaults.glow(),
        colors = IconButtonDefaults.colors(
            containerColor = ObiSilk.RaisedSelected,
            contentColor = ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
        ),
    ) {
        Icon(icon, contentDescription = label, modifier = Modifier.size(24.dp))
    }
}

@Composable
private fun RowScope.ButtonContent(icon: ImageVector?, content: @Composable RowScope.() -> Unit) {
    if (icon != null) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(24.dp))
        Spacer(Modifier.width(10.dp))
    }
    content()
}

/** A button's label at labelLarge. */
@Composable
fun ButtonLabel(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, maxLines = 1)
}
