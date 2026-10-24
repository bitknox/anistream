// Calendar: a week as day tabs, and the chosen day's broadcasts as 72-high rows in air order, so
// the text stays large. The next airing today is the first focus; past rows sit in haze above it.

package tv.anistream.tv.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import kotlin.math.abs
import java.time.format.TextStyle
import tv.anistream.design.CoverPlate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.store.AirTime
import tv.anistream.store.AppClock
import tv.anistream.store.CalendarModel
import tv.anistream.store.ListState
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.ArtImage
import tv.anistream.tv.ui.OfflineLine
import tv.anistream.tv.ui.StateBlock
import tv.anistream.tv.ui.TextBar
import tv.anistream.tv.ui.accessibilityActions
import tv.anistream.tv.ui.isOffline
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.plainReason
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.swapFade
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.SafeSpec
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.TvTab
import tv.anistream.tv.ui.TvTitle
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.pixels
import tv.anistream.tv.ui.planSheet

/** One broadcast: a title, its episode, and when it airs. */
data class Airing(val entry: Entry, val episode: UInt, val at: Instant)

/** The window's broadcasts, each dated against its anchor, in air order. */
fun CalendarModel.Window.airings(now: Instant = AppClock.now()): List<Airing> {
    val from = anchor?.let(Instant::ofEpochSecond) ?: now
    return rows.mapNotNull { row ->
        val seconds = row.airingIn ?: return@mapNotNull null
        val episode = row.airingEpisode ?: return@mapNotNull null
        Airing(row, episode, from.plusSeconds(seconds))
    }.sortedBy { it.at }
}

/** "Today", then "Wed 21" and so on. */
fun dayLabel(day: LocalDate, today: LocalDate): String =
    if (day == today) "Today" else "${day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)} ${day.dayOfMonth}"

/**
 * When it airs on the TV's clock: "16:30", or "16:30 (was 23:00)" when AniList moved it, as the
 * phone writes it.
 */
fun Airing.timeLine(): String {
    val time = AirTime.clock(at)
    val was = entry.delayedFrom?.let { AirTime.clock(Instant.ofEpochSecond(it)) }
    return if (was != null && was != time) "$time (was $was)" else time
}

/** "E7 of 24" for a show on a list, as text on its row. */
private fun Airing.listProgress(): String? {
    if (entry.listStatus == null) return null
    val done = entry.progress?.done ?: return null
    return entry.episodes?.let { "E$done of $it" } ?: "E$done"
}

private const val DAYS = 7

@Composable
fun CalendarScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) { store.loadCalendar() }
    val window by store.calendar.window.collectAsStateWithLifecycle()
    val state by store.calendar.list.state.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val signedIn = trackers.values.any { it.connected }
    var mine by rememberSaveable { mutableStateOf(true) }
    val onlyMine = mine && signedIn
    val now = AppClock.now()
    val zone = AppClock.zone
    val today = now.atZone(zone).toLocalDate()
    var offset by rememberSaveable { mutableIntStateOf(0) }
    val day = today.plusDays(offset.toLong())
    val airings = remember(window) { window.airings(now) }
    val shown = airings.filter { it.at.atZone(zone).toLocalDate() == day && (!onlyMine || it.entry.listStatus != null) }
    // The next airing today leads; on another day, its first.
    val lead = shown.indexOfFirst { it.at >= now }.takeIf { it >= 0 } ?: 0
    val list = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, lead - 1))
    ToTop(nav, memory, scrolled = { list.firstVisibleItemIndex > maxOf(0, lead - 1) }) { list.scrollToItem(maxOf(0, lead - 1)) }
    val loading = (state is ListState.Loading || state is ListState.Idle) && airings.isEmpty()
    val placeholders = placeholdersAfterWait(loading)
    val failed = state as? ListState.Failed

    // Left and Right on a row: the neighbouring day, at the row nearest the same time of day.
    var aim by remember { mutableStateOf<LocalTime?>(null) }
    LaunchedEffect(offset, aim) {
        val time = aim ?: return@LaunchedEffect
        aim = null
        if (shown.isEmpty()) {
            memory.focus("day/$offset")
            return@LaunchedEffect
        }
        val nearest = shown.indices.minBy { index ->
            abs(Duration.between(time, shown[index].at.atZone(zone).toLocalTime()).toMinutes())
        }
        list.scrollToItem(maxOf(0, nearest - 1))
        val key = shown[nearest].key
        repeat(5) {
            withFrameNanos { }
            if (memory.focus(key)) return@LaunchedEffect
        }
    }

    Column(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY, end = Canvas.SafeX)) {
        Row(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            repeat(DAYS) { index ->
                TvTab(
                    dayLabel(today.plusDays(index.toLong()), today),
                    selected = index == offset,
                    onSelect = { offset = index },
                    modifier = Modifier.firstFocus(memory, "day/$index", shown.isEmpty() && index == offset),
                )
            }
        }
        // Under the days: My list when signed in, All airing otherwise; not changeable until sign-in.
        Row(Modifier.padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (signedIn) {
                    TvTab("My list", selected = onlyMine, onSelect = { mine = true }, settle = null, chip = true, modifier = Modifier.remembered(memory, "chip/mine"))
                }
                TvTab("All airing", selected = !onlyMine, onSelect = { mine = false }, settle = null, chip = true, modifier = Modifier.remembered(memory, "chip/all"))
            }
            if (!signedIn) {
                Text(
                    "Sign in to see only your shows",
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(start = 16.dp),
                )
            }
        }
        if (isOffline()) OfflineLine(Modifier.padding(top = 8.dp))
        Spacer(Modifier.height(20.dp))
        when {
            placeholders -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) { repeat(5) { AiringPlaceholder() } }
            failed != null && airings.isEmpty() -> StateBlock(
                "Couldn't load the calendar.",
                Modifier.padding(top = 48.dp),
                detail = plainReason(failed.reason),
                action = "Try again",
                onAction = { store.loadCalendar(force = true) },
                actionModifier = Modifier.firstFocus(memory, "retry"),
            )
            shown.isEmpty() && !loading -> {
                val name = if (offset == 0) "today" else "on ${day.dayOfWeek.getDisplayName(TextStyle.FULL, AirTime.locale)}"
                Text("Nothing airs $name.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
        val density = LocalDensity.current
        val spec = remember(density) { with(density) { SafeSpec(Canvas.FocusRoom.toPx(), 40.dp.toPx()) } }
        ScrollRule(spec) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = 60.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(shown, key = { it.key }, contentType = { "airing" }) { airing ->
                    val index = shown.indexOf(airing)
                    AiringRow(
                        airing,
                        past = airing.at < now,
                        onClick = { onOpen(airing.entry.id) },
                        sheet = { planSheet(store, airing.entry, onOpen) },
                        onDay = { step ->
                            val next = offset + step
                            if (next !in 0 until DAYS) return@AiringRow false
                            offset = next
                            aim = airing.at.atZone(zone).toLocalTime()
                            true
                        },
                        modifier = swapFade().firstFocus(memory, airing.key, index == lead),
                    )
                }
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

/** A row's key in the list and in focus memory. */
private val Airing.key: String get() = "airing/${entry.id}/$episode/${at.epochSecond}"

@Composable
private fun AiringRow(
    airing: Airing,
    past: Boolean,
    onClick: () -> Unit,
    sheet: () -> ActionSheet,
    onDay: (Int) -> Boolean,
    modifier: Modifier,
) {
    val host = LocalActions.current
    var focused by remember { mutableStateOf(false) }
    val state = if (past) "Aired" else "Airing"
    val progress = airing.listProgress()
    Surface(
        onClick = onClick,
        onLongClick = { host?.open(sheet()) },
        modifier = modifier
            .fillMaxWidth()
            .height(72.dp)
            .onFocusChanged { focused = it.isFocused }
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (event.key) {
                    Key.DirectionLeft -> onDay(-1)
                    Key.DirectionRight -> onDay(1)
                    else -> false
                }
            }
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull(airing.timeLine(), airing.entry.title, "Episode ${airing.episode}", progress, state).joinToString(", ")
                if (host != null) customActions = sheet().accessibilityActions(host)
            },
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = Color.Transparent,
            contentColor = if (past) ObiSilk.Haze else ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                airing.timeLine(),
                style = MaterialTheme.typography.labelLarge,
                maxLines = 2,
                modifier = Modifier.width(84.dp).clearAndSetSemantics {},
            )
            ArtImage(
                airing.entry.bannerUrl ?: airing.entry.coverUrl,
                CoverPlate.parse(airing.entry.coverColor),
                pixels(96.dp),
                Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)),
            )
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f).clearAndSetSemantics {}) {
                Text(airing.entry.title, style = TvTitle.list, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("Episode ${airing.episode}", style = MaterialTheme.typography.bodyMedium, color = LocalContentColor.current.copy(alpha = 0.72f))
            }
            if (progress != null) {
                Text(
                    progress,
                    style = MaterialTheme.typography.labelMedium,
                    color = LocalContentColor.current.copy(alpha = 0.72f),
                    modifier = Modifier.padding(end = 16.dp).clearAndSetSemantics {},
                )
            }
            // On the silk of focus, the state takes the row's own colour: water on silk is unreadable.
            Text(
                state,
                style = MaterialTheme.typography.labelMedium,
                color = if (past || focused) LocalContentColor.current else ObiSilk.Water,
                modifier = Modifier.clearAndSetSemantics {},
            )
        }
    }
}

/** A row's placeholder: the time and title as bars, the art as a flat rect. */
@Composable
private fun AiringPlaceholder() {
    Row(Modifier.fillMaxWidth().height(72.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        TextBar(48.dp, Modifier.padding(end = 36.dp))
        Box(Modifier.size(96.dp, 54.dp).background(ObiSilk.Raised, RoundedCornerShape(8.dp)))
        Spacer(Modifier.width(16.dp))
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            TextBar(220.dp, height = 16.dp)
            TextBar(96.dp)
        }
    }
}
