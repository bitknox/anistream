// The source picker on the TV: the same choice as the phone's sheet, in the 400-wide panel on the
// right, reached from the title page's More. Automatic first, then each source in the order they
// are tried with its coverage or note, and under a source with several releases a Releases heading
// and the releases. The chosen row sits in the amber container; Center picks and closes the panel.

package tv.anistream.tv.ui

import tv.anistream.store.plainReason
import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.SourceNote
import tv.anistream.engine.model.SourceOption
import tv.anistream.i18n.Strings
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.AppStore
import tv.anistream.store.PickerRow
import tv.anistream.store.pickerRows

/** The panel for [entry], titled [title] ("Source for this title"). */
fun sourcePickerSheet(store: AppStore, entry: Entry, title: String): ActionSheet =
    sourcePickerSheet(store, entry.id, entry.title, title)

/** The same panel for a show known by its [id] and [name], as the show settings open it. */
fun sourcePickerSheet(store: AppStore, id: AnilistId, name: String, title: String): ActionSheet =
    ActionSheet(title, body = { SourcePickerBody(store, id, name) })

@Composable
private fun SourcePickerBody(store: AppStore, id: AnilistId, name: String) {
    val host = LocalActions.current
    val context = LocalContext.current
    val options by store.sourceChoices.options.collectAsStateWithLifecycle()
    val asked by store.sourceChoices.asked.collectAsStateWithLifecycle()
    val listed = options[id]
    val loading = listed == null && id in asked
    LaunchedEffect(id) { store.loadSourceOptions(id) }
    // An answer that never came is not waited for once the panel is gone.
    DisposableEffect(id) { onDispose { store.sourceChoices.giveUp(id) } }
    val rows = pickerRows(listed)
    // Focus starts on Automatic, and moves to the chosen source once the options are there, unless
    // the viewer has already moved on.
    val automatic = remember { FocusRequester() }
    val chosen = remember { FocusRequester() }
    var onAutomatic by remember { mutableStateOf(true) }
    LaunchedEffect(Unit) { automatic.focusSoon() }
    val chosenKey = rows.firstOrNull { (it as? PickerRow.Source)?.chosen == true || (it as? PickerRow.Release)?.chosen == true }?.key
    val state = rememberLazyListState()
    LaunchedEffect(listed != null, chosenKey) {
        if (listed == null || chosenKey == null || !onAutomatic) return@LaunchedEffect
        // The chosen row may be below the panel's foot, and so not laid out to take focus yet: bring
        // it in with its source and heading above it, so the viewer sees where it sits.
        state.scrollToItem((rows.indexOfFirst { it.key == chosenKey } - 3).coerceAtLeast(0))
        chosen.focusSoon()
    }

    Text(name, style = TvTitle.list, color = ObiSilk.Haze, maxLines = 1, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp))
    LazyColumn(state = state, verticalArrangement = Arrangement.spacedBy(4.dp), contentPadding = PaddingValues(bottom = 27.dp)) {
        items(rows, key = { it.key }, contentType = { it::class.simpleName }) { row ->
            when (row) {
                is PickerRow.Automatic -> TvListRow(
                    Strings.sourcePickerAutomatic(context),
                    onClick = {
                        host?.close()
                        store.chooseSource(id, null)
                    },
                    supporting = Strings.sourcePickerAutomaticDetail(context),
                    selected = row.chosen,
                    modifier = Modifier
                        .focusRequester(automatic)
                        .onFocusChanged { if (it.isFocused) onAutomatic = true },
                )
                is PickerRow.Source -> TvListRow(
                    row.option.name,
                    onClick = {
                        host?.close()
                        store.chooseSource(id, row.option.providerId)
                    },
                    supporting = SourceWords.detail(context, row.option),
                    selected = row.chosen,
                    enabled = row.selectable,
                    modifier = (if (row.chosen) Modifier.focusRequester(chosen) else Modifier)
                        .onFocusChanged { if (it.isFocused) onAutomatic = false },
                )
                is PickerRow.ReleasesHeading -> Text(
                    Strings.sourcePickerReleases(context),
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(start = 32.dp, top = 12.dp, bottom = 4.dp).semantics { heading() },
                )
                is PickerRow.Release -> TvListRow(
                    row.candidate.title,
                    onClick = {
                        host?.close()
                        store.chooseRelease(id, row.option.providerId, row.candidate)
                    },
                    supporting = SourceWords.detail(context, row.candidate),
                    selected = row.chosen,
                    modifier = Modifier
                        .padding(start = 16.dp)
                        .then(if (row.chosen) Modifier.focusRequester(chosen) else Modifier)
                        .onFocusChanged { if (it.isFocused) onAutomatic = false },
                )
            }
        }
        if (listed != null && listed.isEmpty()) {
            item(key = "none") {
                Text(
                    Strings.sourcesEmpty(context),
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                )
            }
        }
        // Quiet while the options arrive: nothing for 600 ms, then two bars like a row's lines.
        if (loading) {
            item(key = "loading") { LoadingRow() }
        }
    }
}

@Composable
private fun LoadingRow() {
    if (!placeholdersAfterWait(true)) return
    Column(Modifier.padding(horizontal = 16.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        TextBar(180.dp, height = 18.dp)
        TextBar(120.dp)
    }
}

/** What a picker row says under its name. */
object SourceWords {
    /** A source's coverage ("6 of 13 episodes"), or why it has none. A failure is the source's own words. */
    fun detail(context: Context, option: SourceOption): String = when (val note = option.note) {
        null -> Strings.sourcePickerCoverage(context, option.covered.toInt(), option.of.toInt())
        SourceNote.Off -> Strings.sourceNoteOff(context)
        SourceNote.Resting -> Strings.sourceNoteResting(context)
        SourceNote.Gone -> Strings.sourceNoteGone(context)
        SourceNote.Ambiguous -> Strings.sourceNoteAmbiguous(context)
        is SourceNote.Failed -> plainReason(context, note.reason)
    }

    /** A release's quality, size and seeders: "1080p · 14.2 GiB · 84 seeders". */
    fun detail(context: Context, release: SourceCandidate): String? = listOfNotNull(
        release.quality?.let { "${it}p" },
        release.size,
        release.seeders?.let { Strings.releaseSeeders(context, it.toInt()) },
    ).joinToString(" · ").ifEmpty { null }
}
