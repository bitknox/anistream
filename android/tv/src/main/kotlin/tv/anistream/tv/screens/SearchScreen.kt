// Search: the field across the top, our keyboard on the left (a to z, 1 to 0, Space, Delete,
// Clear, and a key for the system keyboard), results on the right, 300 ms after the last change.

package tv.anistream.tv.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.ListState
import tv.anistream.tv.posterLine
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardPlaceholder
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.PosterCard
import tv.anistream.tv.ui.SafeSpec
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.tryFocus

/** Our keyboard's keys and what each does to the text. */
object SearchKeys {
    /** The character keys, row by row in six columns: a to z, then 1 to 0. */
    val characters: List<Char> = ('a'..'z').toList() + ('1'..'9').toList() + '0'

    const val COLUMNS = 6

    sealed interface Key {
        data class Character(val char: Char) : Key
        data object Space : Key
        data object Delete : Key
        data object Clear : Key
    }

    /** [text] after [key]: never a leading space nor two in a row. */
    fun type(text: String, key: Key): String = when (key) {
        is Key.Character -> text + key.char
        Key.Space -> if (text.isEmpty() || text.endsWith(' ')) text else "$text "
        Key.Delete -> text.dropLast(1)
        Key.Clear -> ""
    }
}

/** How long after the last change the search runs. */
private const val SEARCH_DELAY = 300L

/** How long a phone's submit waits for the first result: 40 tries 100 ms apart. */
private const val SUBMIT_TRIES = 40
private const val SUBMIT_PAUSE = 100L

private val KeySize = 48.dp

@Composable
fun SearchScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val text = nav.searchText
    LaunchedEffect(Unit) { store.loadSeasonal() }
    LaunchedEffect(text) {
        val query = text.trim()
        if (query.isEmpty()) return@LaunchedEffect
        delay(SEARCH_DELAY)
        store.search(query)
    }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val results by store.search.list.state.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val field = remember { FocusRequester() }
    val remote = LocalRemoteHost.current
    // A phone's Enter: focus the first result once the search has them.
    val submits = nav.searchSubmits
    LaunchedEffect(submits) {
        if (submits == 0) return@LaunchedEffect
        repeat(SUBMIT_TRIES) {
            val first = store.search.list.state.value.ids.firstOrNull()
            if (first != null && memory.focus("result/$first")) {
                remote?.typing = false
                return@LaunchedEffect
            }
            delay(SUBMIT_PAUSE)
        }
    }
    DisposableEffect(remote) { onDispose { remote?.typing = false } }

    DisposableEffect(nav) {
        val mine = {
            // Back from the results returns to the keyboard; from the keyboard, the rail opens.
            val onKeyboard = memory.last?.startsWith("key/") ?: true
            if (!onKeyboard) {
                memory.reset()
                memory.focusFirst()
            }
            !onKeyboard
        }
        nav.toTop = mine
        onDispose { if (nav.toTop === mine) nav.toTop = null }
    }

    Column(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY)) {
        QueryField(text, onText = { nav.searchText = it }, focus = field, modifier = Modifier.width(Canvas.SafeRight - Canvas.Keyline))
        if (remote?.typing == true) {
            Row(Modifier.padding(top = 8.dp, start = 24.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(TvIcons.Phone, contentDescription = null, tint = ObiSilk.Haze, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(8.dp))
                Text("Typing from a phone", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            }
        }
        Spacer(Modifier.height(24.dp))
        Row(Modifier.fillMaxSize()) {
            Column(Modifier.width(288.dp)) {
                Keyboard(memory, onKey = {
                    remote?.typing = false
                    nav.searchText = SearchKeys.type(nav.searchText, it)
                })
                Spacer(Modifier.height(16.dp))
                val keyboard = LocalSoftwareKeyboardController.current
                TonalButton(
                    onClick = {
                        if (field.tryFocus()) keyboard?.show()
                    },
                    icon = TvIcons.Keyboard,
                    modifier = Modifier.remembered(memory, "key/system"),
                ) { ButtonLabel("System keyboard") }
            }
            Spacer(Modifier.width(32.dp))
            Results(text.trim(), results, seasonal, entries, memory, onOpen)
        }
    }
    EnterFocus(memory)
}

/** The field across the top: what is typed, or a quiet prompt. The system keyboard types here. */
@Composable
private fun QueryField(text: String, onText: (String) -> Unit, focus: FocusRequester, modifier: Modifier) {
    Box(
        modifier.height(56.dp).background(ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(TvIcons.Search, contentDescription = null, tint = ObiSilk.Haze, modifier = Modifier.size(24.dp))
            Spacer(Modifier.width(16.dp))
            Box(contentAlignment = Alignment.CenterStart) {
                if (text.isEmpty()) {
                    Text("Search titles", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                }
                BasicTextField(
                    value = text,
                    onValueChange = onText,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
                    cursorBrush = SolidColor(ObiSilk.Amber),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { onText(text) }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                )
            }
        }
    }
}

/** Six columns of 48 × 48 keys, then Space, Delete and Clear two columns each. */
@Composable
private fun Keyboard(memory: FocusMemory, onKey: (SearchKeys.Key) -> Unit) {
    Column {
        SearchKeys.characters.chunked(SearchKeys.COLUMNS).forEach { line ->
            Row {
                line.forEach { char ->
                    KeyCap(
                        label = char.toString(),
                        width = KeySize,
                        onClick = { onKey(SearchKeys.Key.Character(char)) },
                        modifier = Modifier
                            .firstFocus(memory, "key/$char", char == 'a'),
                    )
                }
            }
        }
        Row {
            KeyCap("Space", KeySize * 2, onClick = { onKey(SearchKeys.Key.Space) }, modifier = Modifier.remembered(memory, "key/space"))
            KeyCap(
                "Delete",
                KeySize * 2,
                onClick = { onKey(SearchKeys.Key.Delete) },
                icon = TvIcons.Delete,
                // Holding Center deletes on, a character each repeat.
                modifier = Modifier
                    .onPreviewKeyEvent { event ->
                        val center = event.key == Key.DirectionCenter || event.key == Key.Enter
                        if (center && event.type == KeyEventType.KeyDown && event.nativeKeyEvent.repeatCount > 0) {
                            onKey(SearchKeys.Key.Delete)
                            true
                        } else {
                            false
                        }
                    }
                    .remembered(memory, "key/delete"),
            )
            KeyCap("Clear", KeySize * 2, onClick = { onKey(SearchKeys.Key.Clear) }, modifier = Modifier.remembered(memory, "key/clear"))
        }
    }
}

/** A tonal key; focus fills it silk. */
@Composable
private fun KeyCap(
    label: String,
    width: Dp,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    Box(Modifier.size(width, KeySize).padding(2.dp)) {
        Surface(
            onClick = onClick,
            modifier = modifier.fillMaxSize(),
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
            colors = ClickableSurfaceDefaults.colors(
                containerColor = ObiSilk.RaisedSelected,
                contentColor = ObiSilk.Silk,
                focusedContainerColor = ObiSilk.Silk,
                focusedContentColor = ObiSilk.Ground,
                pressedContainerColor = ObiSilk.Silk,
                pressedContentColor = ObiSilk.Ground,
            ),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
        ) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                if (icon != null) {
                    Icon(icon, contentDescription = label, modifier = Modifier.size(20.dp))
                } else {
                    Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1)
                }
            }
        }
    }
}

/**
 * The right side: before typing, This season; then the results in three columns of posters, or
 * a line naming the query when there are none.
 */
@Composable
private fun Results(
    query: String,
    results: ListState,
    seasonal: ListState,
    entries: Map<AnilistId, tv.anistream.engine.model.Entry>,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
) {
    val leading = with(LocalDensity.current) { Canvas.FocusRoom.toPx() * 2 }
    val trailing = with(LocalDensity.current) { 120.dp.toPx() }
    val spec = remember(leading, trailing) { SafeSpec(leading, trailing) }
    val searching = query.isNotEmpty()
    val found = if (searching) results.ids.mapNotNull { entries[it] } else emptyList()
    val season = seasonal.ids.mapNotNull { entries[it] }
    val placeholders = placeholdersAfterWait(searching && results is ListState.Loading)
    val heading: String?
    val shown = when {
        !searching -> {
            heading = "This season"
            season
        }
        found.isNotEmpty() -> {
            heading = null
            found
        }
        results is ListState.Failed -> {
            heading = "Search isn't reachable right now."
            emptyList()
        }
        results is ListState.Empty -> {
            heading = "Nothing for “$query”."
            emptyList()
        }
        else -> {
            heading = null
            emptyList()
        }
    }
    ScrollRule(spec) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            modifier = Modifier.width(Canvas.SafeRight - 444.dp).fillMaxHeight(),
            contentPadding = PaddingValues(top = Canvas.FocusRoom, bottom = 80.dp, start = Canvas.FocusRoom, end = Canvas.FocusRoom),
            horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            if (heading != null) {
                item(key = "heading", span = { GridItemSpan(maxLineSpan) }, contentType = "heading") {
                    Text(
                        heading,
                        style = if (searching) MaterialTheme.typography.bodyLarge else MaterialTheme.typography.titleMedium,
                        color = ObiSilk.Silk,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (placeholders && found.isEmpty()) {
                items(6, key = { "placeholder/$it" }, contentType = { "poster" }) {
                    CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight)
                }
            }
            items(shown, key = { it.id.toLong() }, contentType = { "poster" }) { entry ->
                PosterCard(
                    entry,
                    onClick = { onOpen(entry.id) },
                    line = entry.posterLine,
                    modifier = Modifier.remembered(memory, "result/${entry.id}"),
                )
            }
        }
    }
}
