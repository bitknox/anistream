// The match picker on the TV: the Choose the match panel, 400 wide on the right like every TV
// panel, and the match search screen it leads to, with the Search screen's own keyboard. A paired
// phone can type into the search. No spinners: a wait is static placeholders after 600 ms.

package tv.anistream.tv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.MatchCandidate
import tv.anistream.i18n.Strings
import tv.anistream.store.AppStore
import tv.anistream.store.MatchSearch
import tv.anistream.store.MatchShown
import tv.anistream.store.MatchWords
import tv.anistream.store.matchShown
import tv.anistream.store.text
import tv.anistream.tv.screens.Keyboard
import tv.anistream.tv.screens.QueryField

/** How long a wait is silent before it shows. */
private const val WAIT_MILLIS = 600L

/**
 * The Choose the match panel for [entry]. Asked by the page, first focus lands on the best match;
 * from Fix this match ([fromMenu]) it searches the AniList title at once and focus waits on
 * Search for another title until results come. [search] opens the search screen.
 */
fun matchPanel(context: android.content.Context, store: AppStore, entry: Entry, fromMenu: Boolean, host: ActionsHost?): ActionSheet {
    val title = if (fromMenu) Strings.matchFix(context) else Strings.matchChoose(context)
    return ActionSheet(
        title,
        body = { MatchPanelBody(store, entry, fromMenu) { host?.open(matchSearchSheet(store, entry, Strings.matchSearchAnother(context))) } },
    )
}

@Composable
private fun MatchPanelBody(store: AppStore, entry: Entry, fromMenu: Boolean, openSearch: () -> Unit) {
    val host = LocalActions.current
    val context = LocalContext.current
    val id = entry.id
    val state by store.match.state.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val source = state.source(id) ?: providers.firstOrNull { it.usable }?.let { it.id to it.name } ?: (null to "")
    val shown = matchShown(state, id, source.second)
    val now = if (state.asks(id)) null else state.current[id]
    val picking = state.picking[id]
    var pickSlow by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        if (fromMenu && state.searches[id] == null) store.searchMatch(id, entry.title, source.first)
    }
    DisposableEffect(id) { onDispose { store.match.closed(id) } }
    // The pick settled: its episodes came, and the panel closes.
    LaunchedEffect(state.settled) { if (state.settled?.id == id && picking == null) host?.close() }
    LaunchedEffect(picking) {
        pickSlow = false
        if (picking != null) {
            delay(WAIT_MILLIS)
            pickSlow = true
        }
    }

    val search = remember { FocusRequester() }
    val best = remember { FocusRequester() }
    val candidates = (shown as? MatchShown.Candidates)?.candidates.orEmpty()
    val bestKey = (candidates.firstOrNull { it.rejected == null } ?: candidates.firstOrNull())?.key
    val listState = rememberLazyListState()
    // First focus: the best match when the page asked; Search while a menu search is out, then the
    // first result once it arrives.
    LaunchedEffect(fromMenu, bestKey) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            val target = if (fromMenu && bestKey == null) search else if (bestKey != null) best else search
            if (target.tryFocus()) return@LaunchedEffect
        }
    }

    Text(
        "${entry.title} · ${MatchWords.anilist(context, entry)}".trimEnd(' ', '·'),
        style = MaterialTheme.typography.bodySmall,
        color = ObiSilk.Haze,
        maxLines = 2,
        modifier = Modifier.padding(start = 16.dp, bottom = 10.dp),
    )
    LazyColumn(state = listState, verticalArrangement = Arrangement.spacedBy(4.dp), contentPadding = PaddingValues(bottom = 27.dp)) {
        if (now != null) {
            item(key = "now-label") { Label(Strings.matchNow(context)) }
            item(key = "now") {
                TvInfoRow(now.title ?: now.source, supporting = MatchWords.chosen(context, now), modifier = Modifier.alpha(0.8f))
            }
        }
        item(key = "search") {
            TvListRow(
                Strings.matchSearchAnother(context),
                onClick = openSearch,
                height = 56.dp,
                modifier = Modifier.focusRequester(search),
            )
        }
        when (shown) {
            is MatchShown.Candidates -> {
                item(key = "label") { Label(Strings.matchSectionSource(context, shown.source)) }
                items(candidates.size, key = { candidates[it].key }) { index ->
                    val candidate = candidates[index]
                    val picked = picking == candidate.key
                    val dim = when {
                        picking == null -> if (candidate.rejected != null) 0.55f else 1f
                        picked -> 1f
                        candidate.rejected != null -> 0.3f
                        else -> 0.5f
                    }
                    TvListRow(
                        candidate.title,
                        onClick = { if (picking == null) store.pickMatch(id, shown.providerId, shown.source, candidate) },
                        supporting = MatchWords.facts(context, candidate),
                        value = if (picked && pickSlow) Strings.matchPicking(context) else MatchWords.closeness(context, candidate),
                        modifier = Modifier
                            .alpha(dim)
                            .then(if (candidate.key == bestKey) Modifier.focusRequester(best) else Modifier),
                    )
                }
            }
            is MatchShown.Searching -> {
                item(key = "label") { Label(Strings.matchSectionSource(context, shown.source)) }
                item(key = "wait") { Placeholders(2) }
            }
            is MatchShown.Empty -> {
                item(key = "label") { Label(Strings.matchSectionSource(context, shown.source)) }
                item(key = "empty") { Message(Strings.matchSearchEmpty(context, shown.source, shown.query), Strings.matchSearchEmptyHint(context)) }
            }
            is MatchShown.Failed -> {
                item(key = "label") { Label(Strings.matchSectionSource(context, shown.source)) }
                item(key = "failed") { Message(Strings.matchSearchFailed(context, shown.source), shown.reason.text(context)) }
            }
            MatchShown.Nothing -> {}
        }
        state.pickFailed[id]?.let { reason ->
            item(key = "pick-failed") { Message(Strings.matchFixFailed(context), reason.text(context)) }
        }
        if (state.current[id]?.chosenByYou == true) {
            item(key = "automatic") {
                TvListRow(
                    Strings.matchAutomatic(context),
                    onClick = {
                        store.matchAutomatically(id)
                        host?.close()
                    },
                    supporting = Strings.matchAutomaticDetail(context),
                    supportingLines = 2,
                )
            }
        }
    }
}

@Composable
private fun Label(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = ObiSilk.Haze,
        modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp).semantics { heading() },
    )
}

@Composable
private fun Message(title: String, detail: String) {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
        Text(detail, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
}

/** Static placeholder rows, after the wait: no spinner on the TV. */
@Composable
private fun Placeholders(count: Int) {
    if (!placeholdersAfterWait(true)) return
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        repeat(count) { index ->
            Column(Modifier.fillMaxWidth().height(72.dp).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp, androidx.compose.ui.Alignment.CenterVertically)) {
                TextBar(if (index % 2 == 0) 220.dp else 170.dp, height = 14.dp)
                TextBar(120.dp, height = 10.dp)
            }
        }
    }
}

/**
 * The match search: the AniList title or what was typed, with the Search screen's keyboard and
 * the source's answer beside it. A paired phone types into the field; the Search key runs it.
 */
fun matchSearchSheet(store: AppStore, entry: Entry, title: String): ActionSheet =
    ActionSheet(title, width = 864.dp, body = { MatchSearchBody(store, entry) })

@Composable
private fun MatchSearchBody(store: AppStore, entry: Entry) {
    val context = LocalContext.current
    val id = entry.id
    val state by store.match.state.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val memory = remember { FocusMemory() }
    val source = state.source(id) ?: providers.firstOrNull { it.usable }?.let { it.id to it.name } ?: (null to "")
    val shown = matchShown(state, id, source.second)
    var query by remember { mutableStateOf((state.searches[id]?.query) ?: entry.title) }
    var phone by remember { mutableStateOf(false) }
    val host = LocalActions.current
    val picking = state.picking[id]

    fun search() = store.searchMatch(id, query, source.first)

    DisposableEffect(Unit) {
        store.match.tvSearching = true
        onDispose { store.match.tvSearching = false }
    }
    // The phone's text replaces the field; its Search runs it.
    LaunchedEffect(Unit) {
        store.match.phone.collect { (text, submit) ->
            query = text
            phone = !submit
            if (submit) search()
        }
    }
    LaunchedEffect(state.settled) { if (state.settled?.id == id && picking == null) host?.close() }

    EnterFocus(memory)
    Text(
        Strings.matchSearchField(context, source.second),
        style = MaterialTheme.typography.bodySmall,
        color = ObiSilk.Haze,
        modifier = Modifier.padding(start = 16.dp, bottom = 6.dp),
    )
    Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
        QueryField(
            androidx.compose.ui.text.input.TextFieldValue(query, androidx.compose.ui.text.TextRange(query.length)),
            onText = { phone = false; query = it.text },
            focus = remember { FocusRequester() },
            typing = phone,
            modifier = Modifier.width(800.dp),
        )
    }
    Spacer(Modifier.height(16.dp))
    Row(Modifier.fillMaxSize()) {
        Column(Modifier.width(288.dp).padding(start = 16.dp)) {
            Keyboard(memory, onKey = {
                phone = false
                query = tv.anistream.tv.screens.SearchKeys.type(query, it)
            })
        }
        Spacer(Modifier.width(32.dp))
        LazyColumn(Modifier.width(480.dp), verticalArrangement = Arrangement.spacedBy(4.dp), contentPadding = PaddingValues(bottom = 27.dp)) {
            // Right of the keyboard, so Right from its keys reaches it; the field keeps Left and Right for its caret.
            item(key = "run") {
                TonalButton(onClick = ::search, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp)) { ButtonLabel(Strings.navSearch(context)) }
            }
            when (val area = shown) {
                is MatchShown.Candidates -> {
                    item(key = "label") { Label(Strings.matchSectionSource(context, area.source)) }
                    items(area.candidates.size, key = { area.candidates[it].key }) { index ->
                        val candidate: MatchCandidate = area.candidates[index]
                        val picked = picking == candidate.key
                        TvListRow(
                            candidate.title,
                            onClick = { if (picking == null) store.pickMatch(id, area.providerId, area.source, candidate) },
                            supporting = MatchWords.facts(context, candidate),
                            value = if (picked) Strings.matchPicking(context) else MatchWords.closeness(context, candidate),
                            modifier = Modifier.alpha(if (candidate.rejected != null && !picked) 0.55f else 1f),
                        )
                    }
                }
                is MatchShown.Searching -> {
                    item(key = "label") { Label(Strings.matchSectionSource(context, area.source)) }
                    item(key = "wait") { Placeholders(3) }
                }
                is MatchShown.Empty -> {
                    item(key = "label") { Label(Strings.matchSectionSource(context, area.source)) }
                    item(key = "empty") { Message(Strings.matchSearchEmpty(context, area.source, area.query), Strings.matchSearchEmptyHint(context)) }
                }
                is MatchShown.Failed -> {
                    item(key = "failed") {
                        Column {
                            Message(Strings.matchSearchFailed(context, area.source), area.reason.text(context))
                            TonalButton(onClick = ::search, modifier = Modifier.padding(start = 16.dp, top = 8.dp)) { ButtonLabel(Strings.actionTryAgain(context)) }
                        }
                    }
                }
                MatchShown.Nothing -> {}
            }
        }
    }
}
