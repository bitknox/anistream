// The rows Settings is built from, as AndroidSettings and AndroidSettingsPlayback draw them:
// grouped list items on surfaceContainer, 16 at a group's ends and 4 between, 2 apart; menus that
// drop down with a check on the current choice; switches on the trailing side; links to a page.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular

/** One row's place in its group, which sets its corners: 16 at the group's ends, 4 between. */
enum class Place {
    Only, First, Middle, Last;

    internal val shape
        get() = when (this) {
            Only -> RoundedCornerShape(16.dp)
            First -> RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp, bottomStart = 4.dp, bottomEnd = 4.dp)
            Middle -> RoundedCornerShape(4.dp)
            Last -> RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp, bottomStart = 16.dp, bottomEnd = 16.dp)
        }

    companion object {
        /** The place of the row at [index] of [count]. */
        fun of(index: Int, count: Int): Place = when {
            count == 1 -> Only
            index == 0 -> First
            index == count - 1 -> Last
            else -> Middle
        }
    }
}

/**
 * Which row the column beside a pane is about, on a tablet: the row tapped last, filled in
 * primaryContainer. A phone has no such column, and no row is ever selected there.
 */
@Stable
class RowFocus(initial: String?) {
    var key: String? by mutableStateOf(initial)
}

val LocalRowFocus = compositionLocalOf<RowFocus?> { null }

/**
 * A group: its header in titleMedium haze, its rows 2 apart, each told its place, and a note under
 * them. Each row is a composable of its place, so a row that is not shown takes no place.
 */
@Composable
fun SettingsGroup(
    title: String? = null,
    footer: String? = null,
    modifier: Modifier = Modifier,
    rows: List<@Composable (Place) -> Unit>,
) {
    Column(modifier) {
        title?.let { SettingsHeader(it) }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            rows.forEachIndexed { index, row -> row(Place.of(index, rows.size)) }
        }
        footer?.let { SettingsFootnote(it) }
    }
}

/** A group's header: "Language", in titleMedium on the ground. */
@Composable
fun SettingsHeader(title: String, modifier: Modifier = Modifier) {
    Text(
        title,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
    )
}

/** A note under a group, in bodyMedium haze. */
@Composable
fun SettingsFootnote(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp),
    )
}

/**
 * A row's frame: surfaceContainer at its place's corners, 56 high and more as its lines need, or
 * primaryContainer while the column beside the pane is about it. [focus] names the row for that
 * column; tapping the row makes it the one.
 */
@Composable
internal fun RowFrame(
    place: Place,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    focus: String? = null,
    content: @Composable (selected: Boolean) -> Unit,
) {
    val rowFocus = LocalRowFocus.current
    val selected = focus != null && rowFocus?.key == focus
    val colors = MaterialTheme.colorScheme
    val click: (() -> Unit)? = when {
        rowFocus != null && focus != null -> ({
            rowFocus.key = focus
            onClick?.invoke()
        })
        else -> onClick
    }
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(place.shape)
            .background(if (selected) colors.primaryContainer else colors.surfaceContainer)
            .then(if (click != null) Modifier.clickable(onClick = click) else Modifier)
            .semantics { if (focus != null && rowFocus != null) this.selected = selected }
            .padding(horizontal = 16.dp, vertical = 8.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        CompositionLocalProvider(LocalContentColor provides if (selected) colors.primary else colors.onSurface) {
            content(selected)
        }
    }
}

/** The row's headline in titleMedium, its value under it, and a note under that, both haze. */
@Composable
internal fun RowText(title: String, value: String?, note: String?, selected: Boolean, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val secondary = if (selected) colors.onPrimaryContainer else colors.onSurfaceVariant
    Column(modifier) {
        Text(title, style = MaterialTheme.typography.titleMedium, color = LocalContentColor.current)
        value?.let { Text(it, style = MaterialTheme.typography.bodyMedium.tabular(), color = secondary) }
        note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = secondary) }
    }
}

/** One choice in a menu: its words, and an example title under them where it helps. */
data class MenuChoice<T>(val value: T, val label: String, val example: String? = null)

/**
 * A setting chosen from a list: its value under its name and the dropdown's triangle at the end;
 * the menu on surfaceContainerHigh at corners 4, 16 in from the row's start, the current choice
 * with a silk check and the rest keeping its 24 dp slot, so the labels line up. Items with an
 * example title are 64 high, the rest 48. Nothing in it is amber.
 */
@Composable
fun <T> MenuRow(
    place: Place,
    title: String,
    value: String?,
    choices: List<MenuChoice<T>>,
    current: T?,
    note: String? = null,
    enabled: Boolean = true,
    focus: String? = null,
    menuWidth: Int = 280,
    onPick: (T) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val usable = enabled && choices.isNotEmpty()
    Box(Modifier.alpha(if (enabled) 1f else 0.38f)) {
        RowFrame(
            place,
            onClick = if (usable) ({ open = true }) else null,
            focus = focus,
            modifier = Modifier.heightIn(min = if (note != null) 88.dp else 72.dp),
        ) { selected ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                RowText(title, value, note, selected, Modifier.weight(1f))
                Icon(
                    Icons.DropDown,
                    contentDescription = null,
                    tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            offset = DpOffset(16.dp, (-8).dp),
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            modifier = Modifier.widthIn(min = menuWidth.dp),
        ) {
            for (choice in choices) MenuItem(choice, choice.value == current) {
                open = false
                onPick(choice.value)
            }
        }
    }
}

@Composable
private fun <T> MenuItem(choice: MenuChoice<T>, picked: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = {
            Column {
                Text(choice.label, style = MaterialTheme.typography.labelLarge)
                choice.example?.let {
                    Text(
                        it,
                        style = ShowTitle.caption,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        },
        leadingIcon = {
            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                if (picked) Icon(Icons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface)
            }
        },
        modifier = Modifier
            .heightIn(min = if (choice.example != null) 64.dp else 48.dp)
            .semantics { if (picked) stateDescription = "Selected" },
        onClick = onClick,
    )
}

/** A switch as the boards draw it: amber with a ground thumb when on, haze on a ring when off. */
@Composable
fun SettingSwitchMark(checked: Boolean, enabled: Boolean = true) {
    val colors = MaterialTheme.colorScheme
    Switch(
        checked = checked,
        onCheckedChange = null,
        enabled = enabled,
        colors = SwitchDefaults.colors(
            checkedTrackColor = colors.primary,
            checkedThumbColor = colors.onPrimary,
            checkedBorderColor = Color.Transparent,
            uncheckedTrackColor = colors.surfaceContainerHighest,
            uncheckedThumbColor = colors.onSurfaceVariant,
            uncheckedBorderColor = colors.onSurfaceVariant,
        ),
    )
}

/** A setting that is on or off: the whole row toggles it, the switch on the trailing side. */
@Composable
fun SwitchRow(
    place: Place,
    title: String,
    checked: Boolean,
    note: String? = null,
    enabled: Boolean = true,
    focus: String? = null,
    onChange: (Boolean) -> Unit,
) {
    RowFrame(
        place,
        onClick = if (enabled) ({ onChange(!checked) }) else null,
        focus = focus,
        modifier = Modifier
            .heightIn(min = if (note != null) 72.dp else 56.dp)
            .alpha(if (enabled) 1f else 0.38f)
            .semantics(mergeDescendants = true) {
                role = Role.Switch
                stateDescription = if (checked) "On" else "Off"
            },
    ) { selected ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            RowText(title, null, note, selected, Modifier.weight(1f))
            SettingSwitchMark(checked, enabled)
        }
    }
}

/** A row that opens a page: its icon, its name, and what it says of itself. 72 high. */
@Composable
fun LinkRow(
    place: Place,
    title: String,
    value: String?,
    icon: ImageVector? = null,
    selected: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clip(place.shape)
            .background(if (selected) colors.primaryContainer else colors.surfaceContainer)
            .clickable(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 16.dp, vertical = 8.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            icon?.let { Icon(it, contentDescription = null, tint = if (selected) colors.primary else colors.onSurfaceVariant) }
            CompositionLocalProvider(LocalContentColor provides if (selected) colors.primary else colors.onSurface) {
                RowText(title, value, null, selected, Modifier.weight(1f))
            }
        }
    }
}

/** A row that says something, with an action of its own at the end when it has one. */
@Composable
fun InfoRow(
    place: Place,
    title: String,
    value: String?,
    note: String? = null,
    focus: String? = null,
    onClick: (() -> Unit)? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    RowFrame(place, onClick = onClick, focus = focus, modifier = modifier.heightIn(min = if (value != null) 72.dp else 56.dp)) { selected ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            leading?.invoke()
            RowText(title, value, note, selected, Modifier.weight(1f))
            trailing?.invoke()
        }
    }
}

/**
 * A dot beside a state's words, 6 across: water once all is well, vermilion for a problem, and a
 * haze ring for a state that is neither, such as off.
 */
@Composable
fun StateDot(color: Color?, modifier: Modifier = Modifier) {
    val ring = MaterialTheme.colorScheme.onSurfaceVariant
    androidx.compose.foundation.Canvas(modifier.size(6.dp)) {
        if (color != null) {
            drawCircle(color)
        } else {
            drawCircle(ring, radius = size.minDimension / 2 - 0.75.dp.toPx(), style = androidx.compose.ui.graphics.drawscope.Stroke(1.5.dp.toPx()))
        }
    }
}
