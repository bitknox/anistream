// Continue. With nothing watched yet: what airs today and this season, which AniList's public
// data gives with no account. Setting up is first launch's, not a card here.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.AirTime
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import tv.anistream.design.ArtShape
import tv.anistream.design.Nagare
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.store.AppClock
import tv.anistream.store.ContinueItem
import tv.anistream.store.ContinueQueue
import tv.anistream.store.ListState
import tv.anistream.store.Wording
import tv.anistream.engine.model.Entry

/** The legal line, everywhere it appears. */
const val LEGAL_LINE = "anistream hosts no content."

@Composable
fun ContinueScreen(actions: AppActions, openCalendar: () -> Unit, openSeasonal: () -> Unit) {
    val store = LocalAppStore.current
    val state by store.continuing.list.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadContinue() }
    val empty = state == ListState.Empty
    val user = trackers.values.firstOrNull { it.connected }?.user
    // The queue's selection and scroll, held here so they outlive a fold (AT3).
    val queueState = rememberQueueState()
    val wide = wideWindow()
    FoldFollowsSelection(wide, queueState)
    // A tablet with something under way: the queue beside the selected title.
    if (wide && state is ListState.Loaded) {
        val queue = store.continuing.queue(titles)
        if (!queue.isEmpty) {
            TabletQueue(queue, actions, queueState)
            return
        }
    }
    // The engine's status, when it has one: changes waiting to sync.
    val status by store.notices.status.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    // Rows swiped off, gone from the list while their Undo stands.
    val leaving = remember { mutableStateListOf<AnilistId>() }
    val leave = rememberLeave(leaving)
    ScreenFrame(
        title = "Continue",
        subtitle = status ?: if (empty) "Nothing watched yet" else null,
        account = Account(user, actions.openSettings),
    ) { padding ->
        Refreshable({ store.loadContinue(force = true) }, top = padding.calculateTopPadding()) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 24.dp)) {
                when (val list = state) {
                    ListState.Empty -> welcome(openCalendar, openSeasonal)
                    is ListState.Failed -> item(key = "failed") {
                        FailedState("Couldn't load Continue", list.reason, retry = { store.loadContinue(force = true) })
                    }
                    is ListState.Loaded -> queue(store.continuing.queue(titles).without(leaving), actions, leave)
                    // Nothing cached: offline says so, and a slow load draws the layout to come.
                    else -> item(key = "loading") { AfterWait { if (offline) OfflineState() else ContinuePlaceholders() } }
                }
            }
        }
    }
}

/** The queue without the titles swiped off it. */
private fun ContinueQueue.without(leaving: List<AnilistId>): ContinueQueue =
    if (leaving.isEmpty()) this else ContinueQueue(inProgress.filterNot { it.id in leaving }, newThisWeek.filterNot { it.id in leaving })

/**
 * Hide from Continue by a swipe: the row goes at once and a notice offers Undo; only once the
 * notice has gone does the engine hide it, as there is nothing to undo there.
 */
@Composable
private fun rememberLeave(leaving: MutableList<AnilistId>): (ContinueItem) -> Unit {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val scope = rememberCoroutineScope()
    return remember(store, snackbars) {
        { item ->
            leaving += item.id
            scope.launch {
                var undone = false
                try {
                    val result = kotlinx.coroutines.withTimeoutOrNull(snackbarMillis("Hidden from Continue", true)) {
                        snackbars.host.currentSnackbarData?.dismiss()
                        snackbars.host.showSnackbar("Hidden from Continue", actionLabel = "Undo", duration = SnackbarDuration.Indefinite)
                    }
                    if (result == null) snackbars.host.currentSnackbarData?.dismiss()
                    undone = result == SnackbarResult.ActionPerformed
                } finally {
                    if (!undone) store.hideFromContinue(item.id)
                    leaving -= item.id
                }
            }
        }
    }
}

/** Continue's layout before it loads: the hero's banner and lines, then two rows. */
@Composable
private fun ContinuePlaceholders() {
    Column(Modifier.padding(horizontal = 16.dp)) {
        tv.anistream.design.CoverPlate(null, Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(ArtShape.banner))
        TextPlaceholder(0.6f, 28.dp, Modifier.padding(top = 8.dp))
        TextPlaceholder(0.4f, 20.dp)
        TextPlaceholder(1f, 56.dp, Modifier.padding(top = 12.dp))
        repeat(2) {
            Row(
                Modifier.padding(top = 16.dp).heightIn(min = 72.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                tv.anistream.design.CoverPlate(null, Modifier.size(40.dp, 60.dp).clip(ArtShape.cover(40.dp)))
                Column(Modifier.weight(1f)) {
                    TextPlaceholder(0.7f, 24.dp)
                    TextPlaceholder(0.4f, 20.dp)
                }
            }
        }
    }
}

/** A plain line of haze text where a screen has nothing else to say. */
@Composable
internal fun Quiet(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/**
 * The queue: the next episode as the hero, the rest under way as a rail of wide cards, then New
 * this week as rows.
 */
private fun LazyListScope.queue(queue: ContinueQueue, actions: AppActions, leave: (ContinueItem) -> Unit) {
    val hero = queue.lead ?: return
    // Every item moves as one list does, the headings with the rows, so a change never slides a
    // row over a heading that snapped into place.
    item(key = "hero", contentType = "hero") { Box(itemMotion()) { Hero(hero, actions) } }
    val rail = queue.inProgress.drop(1)
    if (rail.isNotEmpty()) {
        item(key = "rail", contentType = "rail") {
            Column(itemMotion().padding(top = 16.dp)) {
                Heading("In progress")
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 8.dp),
                ) {
                    items(rail, key = { it.id.toLong() }, contentType = { "card" }) { Box(itemMotion()) { ProgressCard(it, actions) } }
                }
            }
        }
    }
    val fresh = if (queue.inProgress.isEmpty()) queue.newThisWeek.drop(1) else queue.newThisWeek
    if (fresh.isNotEmpty()) {
        item(key = "new", contentType = "heading") { Heading("New this week", itemMotion().padding(top = 16.dp)) }
        items(fresh, key = { "new-${it.id}" }, contentType = { "new-row" }) { Box(itemMotion()) { SwipeRow(it, actions, leave) { NewRow(it, actions) } } }
    }
}

@Composable
private fun Heading(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        modifier = modifier.padding(horizontal = 16.dp).semantics { heading() },
    )
}

/** "Episode 7 · The Lantern Road", with the episode's name when its table has one. */
@Composable
private fun episodeLine(item: ContinueItem): String {
    val store = LocalAppStore.current
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val name = episodes[item.id]?.firstOrNull { it.number == item.episode.toString() }?.title
    return if (name != null) "${item.longEpisode} · $name" else item.longEpisode
}

/** The next episode: its wide art, the title, the episode, how far into it, and the one amber action. */
@Composable
private fun Hero(item: ContinueItem, actions: AppActions) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val navigator = LocalNavigator.current
    LaunchedEffect(item.id) { store.loadEpisodes(item.id) }
    val menuActions = continueMenuActions(item, actions, "hero")
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.padding(horizontal = 16.dp)) {
        // Not clipped to the banner's corners: those are the art's, and would cut into the
        // episode line's first letter at the foot of the block.
        Box {
        Column(
            Modifier
                .shared(Shared.Page, item.id, "hero")
                .openOrMenu(menuActions) { menu = true },
        ) {
        BannerArt(
            item.entry,
            Modifier.fillMaxWidth().aspectRatio(16f / 9f)
                // Resume plays from here: the player grows out of the hero's banner (T2).
                .shared(Shared.Player, item.id)
                .shared(Shared.Art, item.id, "hero")
                .clip(ArtShape.banner),
        )
        Text(item.entry.showTitle, style = ShowTitle.page, modifier = Modifier.padding(top = 8.dp))
        Text(episodeLine(item), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        ContinueMenu(menuActions, expanded = menu, onDismiss = { menu = false })
        }
        // Side by side, or, with text large enough to crowd the button, the progress above it.
        val stacked = LocalDensity.current.fontScale > 1.3f
        val progress: @Composable (Modifier) -> Unit = { modifier ->
            Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                item.fraction?.let { Meter(it) }
                item.minutesLeft?.let {
                    Text("$it min left", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                }
            }
        }
        val play: @Composable (Modifier) -> Unit = { modifier ->
            Button(
                onClick = { actions.play(item.id, item.episode.toString()) },
                enabled = item.playable,
                modifier = modifier.heightIn(min = 56.dp),
                contentPadding = PaddingValues(horizontal = 24.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Text(item.action, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
            }
        }
        if (stacked) {
            Column(Modifier.padding(top = 12.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                progress(Modifier.fillMaxWidth())
                play(Modifier.fillMaxWidth())
            }
        } else {
            Row(
                Modifier.padding(top = 12.dp).fillMaxWidth().heightIn(min = 56.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                progress(Modifier.weight(1f))
                play(Modifier)
            }
        }
    }
}

/** A progress meter: amber, then a 4 dp gap, then the rule colour for what is left. */
@Composable
private fun Meter(fraction: Double, modifier: Modifier = Modifier) {
    LinearProgressIndicator(
        progress = { fraction.toFloat() },
        modifier = modifier.fillMaxWidth().height(4.dp),
        color = MaterialTheme.colorScheme.primary,
        trackColor = MaterialTheme.colorScheme.outline,
        gapSize = 4.dp,
        drawStopIndicator = {},
    )
}

/** A title under way: its banner, its meter, its name and what it plays. */
@Composable
private fun ProgressCard(item: ContinueItem, actions: AppActions) {
    val menuActions = continueMenuActions(item, actions, "rail")
    var menu by remember { mutableStateOf(false) }
    Box {
    ContinueMenu(menuActions, expanded = menu, onDismiss = { menu = false })
    Column(
        Modifier
            .width(208.dp)
            .shared(Shared.Page, item.id, "rail")
            .openOrMenu(menuActions) { menu = true },
    ) {
        BannerArt(item.entry, Modifier.size(208.dp, 117.dp).shared(Shared.Art, item.id, "rail").clip(ArtShape.banner))
        Box(Modifier.padding(top = 8.dp).height(4.dp)) { item.fraction?.let { Meter(it) } }
        Text(
            item.entry.showTitle,
            style = ShowTitle.row,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 4.dp),
        )
        Text(
            item.minutesLeft?.let { "${item.shortEpisode} · $it min left" } ?: item.action,
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
    }
    }
}

/** A new episode: the cover, the title, and what plays with how long it has been out. */
@Composable
private fun NewRow(item: ContinueItem, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val menuActions = continueMenuActions(item, actions, "new")
    var menu by remember { mutableStateOf(false) }
    ContinueMenu(menuActions, expanded = menu, onDismiss = { menu = false })
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .background(colors.surface)
            .openOrMenu(menuActions) { menu = true }
            .padding(start = 16.dp, end = 24.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        CoverArt(item.entry, 40.dp, Modifier.size(40.dp, 60.dp).shared(Shared.Cover, item.id, "new"))
        Column(Modifier.weight(1f)) {
            Text(item.entry.showTitle, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.size(6.dp).background(colors.secondary, CircleShape))
                Text(item.action, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant, maxLines = 1)
            }
        }
    }
}

private fun LazyListScope.welcome(openCalendar: () -> Unit, openSeasonal: () -> Unit) {
    item(key = "airing", contentType = "airing") { Box(itemMotion()) { AiringToday(openCalendar) } }
    item(key = "season", contentType = "season") { Box(itemMotion()) { ThisSeason(openSeasonal) } }
}

/** A section's heading, 48 high, with the arrow that opens the whole of it. */
@Composable
private fun SectionHeading(title: String, open: String, onOpen: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f).semantics { heading() })
        IconButton(onClick = onOpen, modifier = Modifier.semantics { contentDescription = open }) {
            Icon(Icons.Forward, contentDescription = null)
        }
    }
}

/**
 * Folding to one pane with a title picked shows that title's page (AT3); its page goes back to
 * the list and detail when the window widens again.
 */
@Composable
private fun FoldFollowsSelection(wide: Boolean, state: QueueState) {
    val navigator = LocalNavigator.current
    var was by remember { mutableStateOf(wide) }
    LaunchedEffect(wide) {
        val id = state.selected?.toUInt()
        if (was && !wide && state.picked && id != null) {
            navigator.openedByFold = id
            navigator.openTitle(id, "fold")
        }
        was = wide
    }
}

/** What airs today, every airing title's, as a rail of wide cards. */
@Composable
private fun AiringToday(openCalendar: () -> Unit) {
    val store = LocalAppStore.current
    val window by store.calendar.window.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadCalendar() }
    // Dated once a minute, not on every pass: the same list, and the same now, let the cards
    // below skip when anything else on Continue changes.
    val minute = AppClock.now().epochSecond / 60
    val (now, today) = remember(window, minute) {
        val now = AppClock.now()
        now to window.broadcasts(now).today(now)
    }
    if (today.isEmpty()) return
    Column(Modifier.padding(top = 16.dp)) {
        SectionHeading("Airing today", "Show airing today in Calendar", openCalendar)
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(today, key = { "${it.entry.id}-${it.episode}" }, contentType = { "card" }) { broadcast -> Box(itemMotion()) { AiringCard(broadcast, now) } }
        }
    }
}


@Composable
private fun AiringCard(broadcast: Broadcast, now: Instant) {
    val colors = MaterialTheme.colorScheme
    val seconds = broadcast.airs.epochSecond - now.epochSecond
    val time = AirTime.clock(broadcast.airs)
    val out = seconds <= 0
    val line = "E${broadcast.episode} · $time · " + if (out) "out ${Wording.ago(-seconds)}" else "in ${Wording.countdown(seconds)}"
    val navigator = LocalNavigator.current
    val id = broadcast.entry.id
    Column(
        Modifier
            .width(208.dp)
            .shared(Shared.Page, id, "airing")
            .clickable(onClickLabel = "Open title") { navigator.openTitle(id, "airing") },
    ) {
        BannerArt(broadcast.entry, Modifier.size(208.dp, 117.dp).shared(Shared.Art, id, "airing").clip(ArtShape.banner))
        Text(
            broadcast.entry.showTitle,
            style = ShowTitle.row,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (out) Box(Modifier.size(6.dp).background(colors.secondary, CircleShape))
            Text(
                line,
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = colors.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** This season's titles: the first six as a poster grid of three columns. */
@Composable
private fun ThisSeason(openSeasonal: () -> Unit) {
    val store = LocalAppStore.current
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadSeasonal() }
    val season = state.ids.mapNotNull { titles[it] }.take(if (wideWindow()) 12 else 6)
    if (season.isEmpty()) return
    Column(Modifier.padding(top = 16.dp)) {
        SectionHeading("This season", "Show this season in Seasonal", openSeasonal)
        PosterRows(season, columns = if (wideWindow()) 6 else 3)
    }
}

/** Posters in rows of [columns], in a list that scrolls as one. */
@Composable
internal fun PosterRows(entries: List<Entry>, columns: Int, gap: Dp = 13.dp) {
    Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        for (row in entries.chunked(columns)) {
            Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                for (entry in row) PosterCell(entry, Modifier.weight(1f), from = "this-season")
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/**
 * A poster with its title in two lines beneath, and a detail line: the first genre unless [detail]
 * says otherwise, with a water dot when [fresh]. Opens the title, flying from [from].
 */
@Composable
internal fun PosterCell(
    entry: Entry,
    modifier: Modifier = Modifier,
    from: String = "poster",
    detail: String? = entry.genreLine,
    fresh: Boolean = false,
    /** Said as the title opens, before the page goes up. */
    onOpen: (() -> Unit)? = null,
) {
    val navigator = LocalNavigator.current
    Column(
        modifier
            .shared(Shared.Page, entry.id, from)
            .clickable(onClickLabel = "Open title") {
                onOpen?.invoke()
                navigator.openTitle(entry.id, from)
            },
    ) {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            CoverArt(entry, maxWidth, Modifier.size(maxWidth, maxWidth * 1.5f).shared(Shared.Cover, entry.id, from))
        }
        Text(
            entry.showTitle,
            style = ShowTitle.caption,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        detail?.let {
            Row(
                Modifier.padding(top = 2.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (fresh) Box(Modifier.size(6.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** A poster's place before its title arrives: the plate, and its caption's lines as bars. */
@Composable
internal fun PosterPlaceholder() {
    Column {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            tv.anistream.design.CoverPlate(null, Modifier.size(maxWidth, maxWidth * 1.5f).clip(ArtShape.cover(maxWidth)))
        }
        TextPlaceholder(0.8f, 20.dp, Modifier.padding(top = 8.dp))
        TextPlaceholder(0.5f, 16.dp, Modifier.padding(top = 2.dp))
    }
}
