// Continue. On a first launch, with nothing watched yet: a compact setup card with Nagare, then
// what airs today and this season, which AniList's public data gives with no account.

package tv.anistream.ui

import android.content.Context
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import java.time.format.DateTimeFormatter
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
const val LEGAL_LINE = "anistream ships no sources of its own and hosts no content. Every source is external."

/** Whether the setup card has been hidden on this device. */
private const val SETUP_HIDDEN = "setupHidden"

private fun Context.preferences() = getSharedPreferences("anistream", Context.MODE_PRIVATE)

@Composable
fun ContinueScreen(actions: AppActions, openCalendar: () -> Unit, openSeasonal: () -> Unit) {
    val store = LocalAppStore.current
    val state by store.continuing.list.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadContinue() }
    val empty = state == ListState.Empty
    val user = trackers.values.firstOrNull { it.connected }?.user
    // A tablet with something under way: the queue beside the selected title.
    if (wideWindow() && state is ListState.Loaded) {
        val queue = store.continuing.queue(titles)
        if (!queue.isEmpty) {
            TabletQueue(queue, actions)
            return
        }
    }
    // The engine's status, when it has one: offline, or changes waiting to sync.
    val status by store.notices.status.collectAsStateWithLifecycle()
    ScreenFrame(
        title = "Continue",
        subtitle = status ?: if (empty) "Nothing watched yet" else null,
        account = Account(user, actions.openSettings),
    ) { padding ->
        LazyColumn(contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 24.dp)) {
            when (val list = state) {
                ListState.Empty -> welcome(actions, openCalendar, openSeasonal)
                is ListState.Failed -> item { Quiet(list.reason) }
                is ListState.Loaded -> queue(store.continuing.queue(titles), actions)
                else -> {}
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
private fun LazyListScope.queue(queue: ContinueQueue, actions: AppActions) {
    val hero = queue.inProgress.firstOrNull() ?: queue.newThisWeek.firstOrNull() ?: return
    item(key = "hero") { Hero(hero, actions) }
    val rail = queue.inProgress.drop(1)
    if (rail.isNotEmpty()) {
        item(key = "rail") {
            Column(Modifier.padding(top = 16.dp)) {
                Heading("In progress")
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 8.dp),
                ) {
                    items(rail, key = { it.id.toLong() }) { Box(itemMotion()) { ProgressCard(it) } }
                }
            }
        }
    }
    val fresh = if (queue.inProgress.isEmpty()) queue.newThisWeek.drop(1) else queue.newThisWeek
    if (fresh.isNotEmpty()) {
        item(key = "new") { Heading("New this week", Modifier.padding(top = 16.dp)) }
        items(fresh, key = { "new-${it.id}" }) { Box(itemMotion()) { NewRow(it) } }
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
    return if (name != null) "Episode ${item.episode} · $name" else "Episode ${item.episode}"
}

/** The next episode: its wide art, the title, the episode, how far into it, and the one amber action. */
@Composable
private fun Hero(item: ContinueItem, actions: AppActions) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val navigator = LocalNavigator.current
    LaunchedEffect(item.id) { store.loadEpisodes(item.id) }
    Column(Modifier.padding(horizontal = 16.dp)) {
        // Not clipped to the banner's corners: those are the art's, and would cut into the
        // episode line's first letter at the foot of the block.
        Column(
            Modifier
                .shared(Shared.Page, item.id, "hero")
                .clickable { navigator.openTitle(item.id, "hero") },
        ) {
        BannerArt(item.entry, Modifier.fillMaxWidth().aspectRatio(16f / 9f).shared(Shared.Art, item.id, "hero").clip(ArtShape.banner))
        Text(item.entry.showTitle, style = ShowTitle.page, modifier = Modifier.padding(top = 8.dp))
        Text(episodeLine(item), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        Row(
            Modifier.padding(top = 12.dp).fillMaxWidth().height(56.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                item.fraction?.let { Meter(it) }
                item.minutesLeft?.let {
                    Text("$it min left", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                }
            }
            Button(
                onClick = { actions.play(item.id, item.episode.toString()) },
                enabled = item.playable,
                modifier = Modifier.height(56.dp),
                contentPadding = PaddingValues(horizontal = 24.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Text(item.action, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
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
private fun ProgressCard(item: ContinueItem) {
    val navigator = LocalNavigator.current
    Column(
        Modifier
            .width(208.dp)
            .shared(Shared.Page, item.id, "rail")
            .clickable { navigator.openTitle(item.id, "rail") },
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
            item.minutesLeft?.let { "E${item.episode} · $it min left" } ?: item.action,
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
    }
}

/** A new episode: the cover, the title, and what plays with how long it has been out. */
@Composable
private fun NewRow(item: ContinueItem) {
    val colors = MaterialTheme.colorScheme
    val navigator = LocalNavigator.current
    Row(
        Modifier
            .fillMaxWidth()
            .height(72.dp)
            .clickable { navigator.openTitle(item.id, "new") }
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

private fun LazyListScope.welcome(actions: AppActions, openCalendar: () -> Unit, openSeasonal: () -> Unit) {
    item(key = "setup") {
        val context = LocalContext.current
        var hidden by remember { mutableStateOf(context.preferences().getBoolean(SETUP_HIDDEN, false)) }
        if (!hidden) {
            SetupCard(
                actions,
                hide = {
                    hidden = true
                    context.preferences().edit().putBoolean(SETUP_HIDDEN, true).apply()
                },
            )
        }
    }
    item(key = "airing") { AiringToday(openCalendar) }
    item(key = "season") { ThisSeason(openSeasonal) }
}

/**
 * The two things to set up: AniList, which is optional, and a plugin store, which is how anistream
 * plays episodes. And the legal line.
 */
@Composable
private fun SetupCard(actions: AppActions, hide: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val type = MaterialTheme.typography
    Column(
        Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(MaterialTheme.shapes.large)
            .background(colors.surfaceContainer)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Nagare(cell = 4)
            Column(Modifier.weight(1f)) {
                Row(Modifier.height(40.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "Set up anistream",
                        style = type.titleMedium,
                        modifier = Modifier.weight(1f).semantics { heading() },
                    )
                    TextButton(
                        onClick = hide,
                        colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                        contentPadding = PaddingValues(horizontal = 12.dp),
                        // Its label flush with the rows' trailing edge, as the board sets it.
                        modifier = Modifier.offset(x = 12.dp).semantics { contentDescription = "Hide setup" },
                    ) { Text("Hide") }
                }
                SetupRow("AniList", note = "Optional", detail = "Syncs your lists") {
                    FilledTonalButton(
                        onClick = actions.connectAniList,
                        contentPadding = PaddingValues(horizontal = 16.dp),
                        modifier = Modifier.widthIn(min = 88.dp).semantics { contentDescription = "Connect AniList" },
                    ) { Text("Connect") }
                }
                HorizontalDivider(color = colors.outlineVariant)
                SetupRow("Plugin store", note = null, detail = "Needed for playback") {
                    Button(
                        onClick = actions.addStore,
                        contentPadding = PaddingValues(horizontal = 16.dp),
                        modifier = Modifier.widthIn(min = 88.dp).semantics { contentDescription = "Add plugin store" },
                    ) { Text("Add") }
                }
            }
        }
        Text(LEGAL_LINE, style = type.bodySmall, color = colors.onSurfaceVariant)
    }
}

@Composable
private fun SetupRow(name: String, note: String?, detail: String, action: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val type = MaterialTheme.typography
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Bottom) {
                Text(name, style = type.bodyLarge)
                if (note != null) Text(note, style = type.bodySmall, color = colors.onSurfaceVariant, modifier = Modifier.padding(bottom = 2.dp))
            }
            Text(detail, style = type.bodyMedium, color = colors.onSurfaceVariant)
        }
        action()
    }
}

/** A section's heading, 48 high, with the arrow that opens the whole of it. */
@Composable
private fun SectionHeading(title: String, open: String, onOpen: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f).semantics { heading() })
        IconButton(onClick = onOpen, modifier = Modifier.semantics { contentDescription = open }) {
            Icon(Icons.Forward, contentDescription = null)
        }
    }
}

/** What airs today, every airing title's, as a rail of wide cards. */
@Composable
private fun AiringToday(openCalendar: () -> Unit) {
    val store = LocalAppStore.current
    val window by store.calendar.window.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadCalendar() }
    val now = AppClock.now()
    val today = window.broadcasts(now).today(now)
    if (today.isEmpty()) return
    Column(Modifier.padding(top = 16.dp)) {
        SectionHeading("Airing today", "Show airing today in Calendar", openCalendar)
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(today, key = { "${it.entry.id}-${it.episode}" }) { broadcast -> Box(itemMotion()) { AiringCard(broadcast, now) } }
        }
    }
}

private val clock = DateTimeFormatter.ofPattern("HH:mm")

@Composable
private fun AiringCard(broadcast: Broadcast, now: Instant) {
    val colors = MaterialTheme.colorScheme
    val seconds = broadcast.airs.epochSecond - now.epochSecond
    val time = clock.format(broadcast.airs.atZone(AppClock.zone))
    val out = seconds <= 0
    val line = "E${broadcast.episode} · $time · " + if (out) "out ${Wording.ago(-seconds)}" else "in ${Wording.countdown(seconds)}"
    val navigator = LocalNavigator.current
    val id = broadcast.entry.id
    Column(
        Modifier
            .width(208.dp)
            .shared(Shared.Page, id, "airing")
            .clickable { navigator.openTitle(id, "airing") },
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
) {
    val navigator = LocalNavigator.current
    Column(
        modifier
            .shared(Shared.Page, entry.id, from)
            .clickable { navigator.openTitle(entry.id, from) },
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
