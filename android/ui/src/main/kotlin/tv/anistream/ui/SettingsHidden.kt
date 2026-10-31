// Settings, Hidden titles: the titles marked Not interested, each with Restore.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ShowTitle
import tv.anistream.engine.model.HiddenTitle
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.Wording

/** The hidden titles, most recent first, and a line for when there are none. */
@Composable
internal fun HiddenPane() {
    val store = LocalAppStore.current
    val hidden by store.home.hidden.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadHiddenTitles() }
    val list = hidden
    Pane {
        when {
            list == null -> Unit
            list.isEmpty() -> Quiet(stringResource(R.string.settings_hidden_empty))
            else -> SettingsGroup(
                footer = stringResource(R.string.settings_hidden_titles_body),
                rows = list.map { title -> @Composable { place: Place -> HiddenRow(place, title) } },
            )
        }
    }
}

@Composable
private fun HiddenRow(place: Place, hidden: HiddenTitle) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val entry = hidden.entry
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clip(place.shape)
            .background(colors.surfaceContainer)
            .padding(start = 16.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        CoverArt(entry, 40.dp, Modifier.size(40.dp, 60.dp))
        Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
            Text(entry.showTitle, style = ShowTitle.row, maxLines = 2, overflow = TextOverflow.Ellipsis)
            val line = if (hidden.untilAirs) Strings.settingsHiddenUntilAirs(context) else entry.genreLine
            line?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1) }
            hiddenLine(context, hidden)?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1) }
        }
        TextButton(
            onClick = { store.restoreTitle(entry.id); store.loadHiddenTitles() },
            colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
        ) { Text(Strings.settingsRestore(context)) }
    }
}

/** "Hidden 3d ago", from the time the engine kept; nothing when it was not kept. */
private fun hiddenLine(context: android.content.Context, hidden: HiddenTitle): String? {
    if (hidden.hiddenAt <= 0) return null
    val seconds = (System.currentTimeMillis() / 1000 - hidden.hiddenAt).coerceAtLeast(0)
    return Strings.settingsHiddenOn(context, Wording.ago(context, seconds))
}
