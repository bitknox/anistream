// The match sheet (AndroidMatchSheet): which of a source's titles a show is, when the source could
// not tell or got it wrong. The titles the source offered, a search of the source by hand, and
// Match automatically to forget a choice. A modal bottom sheet on a phone, a 560 wide dialog at
// expanded widths. A sibling of the source picker, whose last row opens it.

package tv.anistream.ui

import android.content.Context
import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.alpha
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.CurrentMatch
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.MatchCandidate
import tv.anistream.engine.model.MediaFormat
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.MatchPickerRequest
import tv.anistream.store.MatchShown
import tv.anistream.store.MatchWords
import tv.anistream.store.matchShown
import tv.anistream.store.MatchSearch
import tv.anistream.store.MatchState
import tv.anistream.store.text

/** How long a wait is silent before it shows. */
private const val WAIT_MILLIS = 600L

/** The sheet for whichever title the store names, over the whole app. */
@Composable
internal fun MatchPickerHost() {
    val store = LocalAppStore.current
    val request by store.match.request.collectAsStateWithLifecycle()
    val open = request ?: return
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val entry = entries[open.id] ?: return
    if (wideWindow()) MatchDialog(entry, open) else MatchSheet(entry, open)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MatchSheet(entry: Entry, request: MatchPickerRequest) {
    val store = LocalAppStore.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false)
    val scope = rememberCoroutineScope()
    val close: () -> Unit = { scope.launch { sheetState.hide() }.invokeOnCompletion { store.match.dismiss() } }
    // From the menu the field is filled and the sheet full; asked by the page, the candidates stand
    // at the middle and the field takes the sheet up when it takes focus.
    LaunchedEffect(request) { if (request.fromMenu) sheetState.expand() }
    ModalBottomSheet(
        onDismissRequest = { store.match.dismiss() },
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        scrimColor = MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        MatchContent(entry, request, dialog = false, close = close, onFieldFocus = { scope.launch { sheetState.expand() } })
    }
}

@Composable
private fun MatchDialog(entry: Entry, request: MatchPickerRequest) {
    val store = LocalAppStore.current
    Dialog(onDismissRequest = { store.match.dismiss() }, properties = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false)) {
        androidx.compose.material3.Surface(
            modifier = Modifier.width(560.dp),
            shape = MaterialTheme.shapes.extraLarge,
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) {
            MatchContent(entry, request, dialog = true, close = { store.match.dismiss() }, onFieldFocus = {})
        }
    }
}

@Composable
private fun MatchContent(entry: Entry, request: MatchPickerRequest, dialog: Boolean, close: () -> Unit, onFieldFocus: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val id = request.id
    val state by store.match.state.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    var query by rememberSaveable(id) { mutableStateOf(if (request.fromMenu) entry.showTitle else "") }
    var searchSlow by remember { mutableStateOf(false) }
    var pickSlow by remember { mutableStateOf(false) }

    val source = state.source(id) ?: providers.firstOrNull { it.usable }?.let { it.id to it.name } ?: (null to "")
    val providerId: String? = source.first
    val sourceName: String = source.second
    val shown = matchShown(state, id, sourceName)
    val now = if (state.asks(id)) null else state.current[id]
    val picking = state.picking[id]

    fun search() = store.searchMatch(id, query, providerId)

    // From the menu: the AniList title, searched at once.
    LaunchedEffect(request) { if (request.fromMenu && query.isNotBlank()) search() }
    DisposableEffect(id) { onDispose { store.match.closed(id) } }
    // The pick settled: its episodes came, and the sheet goes down.
    LaunchedEffect(state.settled) { if (state.settled?.id == id && picking == null) close() }
    LaunchedEffect(state.searches[id]) {
        searchSlow = false
        if (state.searches[id] is MatchSearch.Searching) {
            delay(WAIT_MILLIS)
            searchSlow = true
        }
    }
    LaunchedEffect(picking) {
        pickSlow = false
        if (picking != null) {
            delay(WAIT_MILLIS)
            pickSlow = true
        }
    }

    Column(Modifier.fillMaxWidth().imePadding().verticalScroll(rememberScrollState())) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = if (dialog) 24.dp else 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                stringResource(R.string.match_choose),
                style = if (dialog) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() },
            )
            Text(entry.showTitle, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, top = 16.dp)
                .onFocusChanged { if (it.isFocused) onFieldFocus() },
            singleLine = true,
            label = { Text(stringResource(R.string.match_search_field, sourceName)) },
            leadingIcon = { Icon(Icons.Search, contentDescription = null) },
            trailingIcon = {
                if (query.isNotEmpty()) {
                    IconButton(onClick = { query = "" }) { Icon(Icons.Clear, contentDescription = stringResource(R.string.search_clear)) }
                }
            },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { search() }),
        )
        MatchWords.anilist(context, entry).takeIf { it.isNotEmpty() }?.let {
            Text(it, style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(start = 30.dp, end = 24.dp, top = 6.dp))
        }

        // The list area keeps its height while its state changes, so the footer never jumps.
        Column(Modifier.fillMaxWidth().heightIn(min = 280.dp).padding(top = 12.dp)) {
            if (now != null) {
                SectionHeader(stringResource(R.string.match_now))
                MatchLine(
                    title = now.title ?: now.source,
                    detail = MatchWords.chosen(context, now),
                    trailing = null,
                    alpha = 1f,
                    selected = false,
                    matching = false,
                    enabled = false,
                    onClick = {},
                )
                HorizontalDivider(Modifier.padding(horizontal = 24.dp, vertical = 4.dp), color = colors.outlineVariant)
            }
            state.pickFailed[id]?.let { reason ->
                FailedLine(stringResource(R.string.match_fix_failed), reason.text(context))
            }
            Crossfade(shown, label = "match-list") { area -> // i18n-skip
                Column {
                    when (area) {
                        is MatchShown.Candidates -> {
                            SectionHeader(stringResource(R.string.match_section_source, area.source))
                            for (candidate in area.candidates) {
                                val picked = picking == candidate.key
                                val dim = when {
                                    picking == null -> if (candidate.rejected != null) 0.55f else 1f
                                    picked -> 1f
                                    candidate.rejected != null -> 0.3f
                                    else -> 0.5f
                                }
                                MatchLine(
                                    title = candidate.title,
                                    detail = MatchWords.facts(context, candidate),
                                    trailing = if (picked && pickSlow) stringResource(R.string.match_picking) else MatchWords.closeness(context, candidate),
                                    alpha = dim,
                                    selected = picked,
                                    matching = picked && pickSlow,
                                    enabled = picking == null,
                                    label = Strings.matchRowLabel(context, candidate.title, MatchWords.facts(context, candidate), MatchWords.closeness(context, candidate)),
                                ) { store.pickMatch(id, area.providerId, area.source, candidate) }
                            }
                        }
                        is MatchShown.Searching -> {
                            SectionHeader(stringResource(R.string.match_section_source, area.source))
                            Column(
                                Modifier.fillMaxWidth().height(200.dp).padding(horizontal = 32.dp),
                                verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
                                horizontalAlignment = Alignment.CenterHorizontally,
                            ) {
                                // Silent for 600 ms, then the wait shows.
                                if (searchSlow) {
                                    CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
                                    Text(
                                        stringResource(R.string.match_searching, area.source, area.query),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = colors.onSurfaceVariant,
                                        textAlign = TextAlign.Center,
                                    )
                                }
                            }
                        }
                        is MatchShown.Empty -> Column(
                            Modifier.fillMaxWidth().height(220.dp).padding(horizontal = 32.dp),
                            verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Text(stringResource(R.string.match_search_empty, area.source, area.query), style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
                            Text(stringResource(R.string.match_search_empty_hint), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, textAlign = TextAlign.Center)
                        }
                        is MatchShown.Failed -> Column(
                            Modifier.fillMaxWidth().height(240.dp).padding(horizontal = 32.dp),
                            verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                Box(Modifier.size(8.dp).background(colors.error, CircleShape))
                                Text(stringResource(R.string.match_search_failed, area.source), style = MaterialTheme.typography.bodyLarge)
                            }
                            Text(area.reason.text(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, textAlign = TextAlign.Center)
                            FilledTonalButton(onClick = ::search, modifier = Modifier.padding(top = 12.dp)) { Text(stringResource(R.string.action_try_again)) }
                        }
                        MatchShown.Nothing -> Spacer(Modifier.height(1.dp))
                    }
                }
            }
        }

        // Only while the match is one the person chose; no confirmation, a notice says it happened.
        val chosen = state.current[id]?.chosenByYou == true
        if (dialog) {
            Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 24.dp, top = 12.dp, bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                if (chosen) TextButton(onClick = { store.matchAutomatically(id); close() }) { Text(stringResource(R.string.match_automatic)) }
                TextButton(onClick = close) { Text(stringResource(R.string.action_cancel)) }
            }
        } else if (chosen) {
            Column(Modifier.padding(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                TextButton(onClick = { store.matchAutomatically(id); close() }) { Text(stringResource(R.string.match_automatic)) }
                Text(
                    stringResource(R.string.match_automatic_detail),
                    style = MaterialTheme.typography.labelMedium,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 12.dp),
                )
            }
        } else {
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth().heightIn(min = 40.dp).padding(horizontal = 24.dp).wrapHeading(),
    )
}

private fun Modifier.wrapHeading(): Modifier = this.semantics { heading() }.padding(top = 10.dp)

/** One title: its name, what it lists under it, and how close it is, or why it was ruled out. */
@Composable
private fun MatchLine(
    title: String,
    detail: String,
    trailing: String?,
    alpha: Float,
    selected: Boolean,
    matching: Boolean,
    enabled: Boolean,
    label: String? = null,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .background(if (selected) colors.onSurface.copy(alpha = 0.1f) else androidx.compose.ui.graphics.Color.Transparent)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .alpha(alpha)
            .padding(horizontal = 24.dp, vertical = 8.dp)
            .semantics(mergeDescendants = true) { if (label != null) contentDescription = label },
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (detail.isNotEmpty()) Text(detail, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        if (matching) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
        if (trailing != null) {
            Text(trailing, style = MaterialTheme.typography.labelMedium, color = if (matching) colors.onSurface else colors.onSurfaceVariant, maxLines = 1)
        }
    }
}

/** A pick that could not be used, above the list, with the engine's reason. */
@Composable
private fun FailedLine(title: String, reason: String) {
    val colors = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        Box(Modifier.padding(top = 7.dp).size(8.dp).background(colors.error, CircleShape))
        Column {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(reason, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
    }
}
