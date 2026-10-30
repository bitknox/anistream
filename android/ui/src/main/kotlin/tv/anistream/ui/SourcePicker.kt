// The source picker (AndroidSourcePicker): a modal bottom sheet from the title page's More menu.
// Automatic first, then each source in the order they are tried with what it has of the title, and
// under a source with several releases a Releases heading and the releases. A check marks the
// chosen row; picking one sends the choice and closes the sheet, and the episode list follows from
// the engine's events.

package tv.anistream.ui

import tv.anistream.store.plainReason
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ShowTitle
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.SourceNote
import tv.anistream.engine.model.SourceOption
import tv.anistream.i18n.R
import tv.anistream.store.PickerRow
import tv.anistream.store.pickerRows

/** The sheet, over the title page. Picking a row sends the choice and closes it. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun SourcePickerSheet(entry: Entry, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val options by store.sourceChoices.options.collectAsStateWithLifecycle()
    val asked by store.sourceChoices.asked.collectAsStateWithLifecycle()
    LaunchedEffect(entry.id) { store.loadSourceOptions(entry.id) }
    // An answer that never came is not waited for once the sheet is gone.
    DisposableEffect(entry.id) { onDispose { store.sourceChoices.giveUp(entry.id) } }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        // T7: the scrim black at 32 %, the handle haze at 40 %.
        scrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        SourcePickerContent(
            entry = entry,
            options = options[entry.id],
            loading = options[entry.id] == null && entry.id in asked,
            onSource = { provider ->
                store.chooseSource(entry.id, provider)
                onDismiss()
            },
            onRelease = { option, release ->
                store.chooseRelease(entry.id, option.providerId, release)
                onDismiss()
            },
        )
    }
}

/** The list inside the sheet, apart from the store so a shot and a test can fill it. */
@Composable
internal fun SourcePickerContent(
    entry: Entry,
    options: List<SourceOption>?,
    loading: Boolean,
    onSource: (provider: String?) -> Unit,
    onRelease: (SourceOption, SourceCandidate) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp)) {
            Text(stringResource(R.string.source_picker_title), style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
            Text(entry.showTitle, style = ShowTitle.row, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        for (row in pickerRows(options)) {
            when (row) {
                is PickerRow.Automatic -> PickerLine(
                    title = stringResource(R.string.source_picker_automatic),
                    detail = stringResource(R.string.source_picker_automatic_detail),
                    chosen = row.chosen,
                ) { onSource(null) }
                is PickerRow.Source -> PickerLine(
                    title = row.option.name,
                    detail = sourceDetail(row.option),
                    chosen = row.chosen,
                    dimmed = row.dimmed,
                    enabled = row.selectable,
                ) { onSource(row.option.providerId) }
                is PickerRow.ReleasesHeading -> Text(
                    stringResource(R.string.source_picker_releases),
                    style = MaterialTheme.typography.titleSmall,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(start = 32.dp, end = 16.dp, top = 12.dp, bottom = 4.dp).semantics { heading() },
                )
                is PickerRow.Release -> PickerLine(
                    title = row.candidate.title,
                    detail = releaseDetail(row.candidate),
                    chosen = row.chosen,
                    inset = true,
                ) { onRelease(row.option, row.candidate) }
            }
        }
        if (options != null && options.isEmpty()) {
            Text(
                stringResource(R.string.sources_empty),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            )
        }
        // Quiet while the options arrive: nothing for 600 ms, then two lines like a row's own.
        if (loading) {
            AfterWait {
                Column(Modifier.heightIn(min = 64.dp).padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.Center) {
                    TextPlaceholder(0.4f)
                    TextPlaceholder(0.25f, lineHeight = 16.dp)
                }
            }
        }
    }
}

/** One choice: its name, what it has under it, and a check where it is the one chosen. */
@Composable
private fun PickerLine(
    title: String,
    detail: String?,
    chosen: Boolean,
    dimmed: Boolean = false,
    enabled: Boolean = true,
    inset: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val chosenWord = stringResource(R.string.source_picker_chosen)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .selectable(selected = chosen, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(start = if (inset) 32.dp else 16.dp, end = 16.dp, top = 8.dp, bottom = 8.dp)
            .semantics(mergeDescendants = true) { if (chosen) stateDescription = chosenWord },
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (dimmed || !enabled) colors.onSurfaceVariant else colors.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (!detail.isNullOrEmpty()) {
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        // The check keeps its place whether shown or not, so the text does not move.
        Box(Modifier.size(24.dp)) { if (chosen) Icon(Icons.Check, contentDescription = null) }
    }
}

/** A source's coverage ("6 of 13 episodes"), or why it has none. A failure is the source's own words. */
@Composable
internal fun sourceDetail(option: SourceOption): String = when (val note = option.note) {
    null -> pluralStringResource(R.plurals.source_picker_coverage, option.of.toInt(), option.covered.toInt(), option.of.toInt())
    SourceNote.Off -> stringResource(R.string.source_note_off)
    SourceNote.Resting -> stringResource(R.string.source_note_resting)
    SourceNote.Gone -> stringResource(R.string.source_note_gone)
    SourceNote.Ambiguous -> stringResource(R.string.source_note_ambiguous)
    is SourceNote.Failed -> plainReason(androidx.compose.ui.platform.LocalContext.current, note.reason)
}

/** A release's quality, size and seeders: "1080p · 14.2 GiB · 84 seeders". */
@Composable
internal fun releaseDetail(release: SourceCandidate): String? = listOfNotNull(
    release.quality?.let { "${it}p" },
    release.size,
    release.seeders?.let { pluralStringResource(R.plurals.release_seeders, it.toInt(), it.toInt()) },
).joinToString(" · ").ifEmpty { null }
