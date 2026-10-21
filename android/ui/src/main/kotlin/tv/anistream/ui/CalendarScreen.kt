// The calendar as one scrolling page: My list or All airing, the week's days as a strip that
// jumps to each, and every day's broadcasts under its heading, earlier days loaded on the way up.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.store.AirTime
import tv.anistream.store.ListState
import tv.anistream.store.Wording
import uniffi.anistream_model.Command
import uniffi.anistream_model.Entry
import uniffi.anistream_model.LibrarySegment

/** Whether a title is yours: under way here, or on your Watching or Planning list. */
fun Entry.isMine(): Boolean =
    progress != null || listStatus == LibrarySegment.WATCHING || listStatus == LibrarySegment.PLANNING

private val dayHeading = DateTimeFormatter.ofPattern("EEEE d MMMM", AirTime.locale)
private val clock = DateTimeFormatter.ofPattern("HH:mm", AirTime.locale)

/** "Today", "Tomorrow", "Yesterday", or "Thursday 22 October". */
private fun dayName(day: LocalDate, today: LocalDate): String = when (day) {
    today -> "Today"
    today.plusDays(1) -> "Tomorrow"
    today.minusDays(1) -> "Yesterday"
    else -> dayHeading.format(day)
}

@Composable
fun CalendarScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val window by store.calendar.window.collectAsStateWithLifecycle()
    val state by store.calendar.list.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadCalendar() }

    val zone = ZoneId.systemDefault()
    val now = Instant.now()
    val today = LocalDate.ofInstant(now, zone)
    // The titles as they now stand: progress may have moved since the calendar came.
    val broadcasts = window.broadcasts(now).map { b ->
        titles[b.entry.id]?.let { b.copy(entry = b.entry.copy(progress = it.progress, listStatus = it.listStatus)) } ?: b
    }
    val anyMine = broadcasts.any { it.entry.isMine() }
    var mine by rememberSaveable { mutableStateOf<Boolean?>(null) }
    val showMine = mine ?: anyMine
    val shown = if (showMine) broadcasts.filter { it.entry.isMine() } else broadcasts
    val byDay = shown.groupBy { LocalDate.ofInstant(it.airs, zone) }.toSortedMap()
    val week = (0L until 7L).map { today.with(DayOfWeek.MONDAY).plusDays(it) }
    // Every day of this week and the next seven has a heading, so today can stand at the top of
    // the page, and so does every other day with something on it.
    val ahead = (0L..7L).map { today.plusDays(it) }
    val days = (byDay.keys + week + ahead).toSortedSet().toList()

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    // Where each day's heading is in the list, and which day each row belongs to.
    val layout = remember(days, byDay) {
        val headings = mutableMapOf<LocalDate, Int>()
        val dayOf = mutableListOf<LocalDate>()
        for (day in days) {
            headings[day] = dayOf.size
            repeat(1 + maxOf(1, byDay[day]?.size ?: 0)) { dayOf += day }
        }
        headings to dayOf
    }
    val (headings, dayOf) = layout
    var placed by rememberSaveable { mutableStateOf(false) }
    // The strip follows the page once it has opened on today: the day at its top is selected.
    val selected by remember(dayOf) {
        derivedStateOf { if (placed) dayOf.getOrNull(list.firstVisibleItemIndex) ?: today else today }
    }
    // Open on today, by its heading's key once the list has laid it out, so a list still taking
    // its shape never leaves the page on another day.
    val latestHeadings by rememberUpdatedState(headings)
    LaunchedEffect(state is ListState.Loaded) {
        if (placed || state !is ListState.Loaded) return@LaunchedEffect
        val key = "day-$today"
        snapshotFlow { list.layoutInfo.totalItemsCount }.first { it > 0 }
        repeat(3) {
            val index = latestHeadings[today] ?: return@repeat
            list.scrollToItem(index)
            if (list.layoutInfo.visibleItemsInfo.firstOrNull()?.key == key) return@repeat
        }
        placed = true
    }
    // Earlier broadcasts, as the page reaches its top.
    LaunchedEffect(list.firstVisibleItemIndex, window.morePast, placed) {
        val anchor = window.anchor
        if (placed && list.firstVisibleItemIndex == 0 && window.morePast && anchor != null) {
            store.send(Command.LoadCalendarPast(anchor, window.pages))
        }
    }

    ScreenFrame(title = "Calendar") { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            // Pinned above the days: which broadcasts, and the week's days to jump to.
            ConnectedChoice(
                choices = listOf("My list", "All airing"),
                selected = if (showMine) 0 else 1,
                onSelect = { mine = it == 0 },
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            WeekStrip(week, today, selected, aired = byDay.keys) { day ->
                headings[day]?.let { scope.launch { list.animateScrollToItem(it) } }
            }
            LazyColumn(state = list, contentPadding = PaddingValues(top = 8.dp, bottom = 24.dp)) {
                for (day in days) {
                    item(key = "day-$day") {
                        Text(
                            dayName(day, today),
                            style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
                        )
                    }
                    val rows = byDay[day].orEmpty()
                    if (rows.isEmpty()) {
                        item(key = "none-$day") {
                            Text(
                                "Nothing airing",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                            )
                        }
                    } else {
                        items(rows, key = { "row-${it.entry.id}-${it.episode}-${it.airs.epochSecond}" }) { broadcast ->
                            BroadcastRow(broadcast, now) { navigator.openTitle(broadcast.entry.id, "calendar") }
                        }
                    }
                }
            }
        }
    }
}

/**
 * A connected button group of two to four: 40 high, 2 apart, inner corners 8 and outer corners
 * full; the selected one rounds fully, with a check, on primaryContainer.
 */
@Composable
internal fun ConnectedChoice(choices: List<String>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Row(modifier.fillMaxWidth().height(48.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
        choices.forEachIndexed { index, choice ->
            val chosen = index == selected
            val shape = when {
                chosen -> RoundedCornerShape(20.dp)
                index == 0 -> RoundedCornerShape(topStart = 20.dp, bottomStart = 20.dp, topEnd = 8.dp, bottomEnd = 8.dp)
                index == choices.lastIndex -> RoundedCornerShape(topStart = 8.dp, bottomStart = 8.dp, topEnd = 20.dp, bottomEnd = 20.dp)
                else -> RoundedCornerShape(8.dp)
            }
            val content = if (chosen) colors.onPrimaryContainer else colors.onSurface
            Row(
                Modifier
                    .weight(1f)
                    .height(40.dp)
                    .clip(shape)
                    .background(if (chosen) colors.primaryContainer else colors.surfaceContainer)
                    .clickable { onSelect(index) },
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (chosen) {
                    Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                }
                Text(choice, style = MaterialTheme.typography.labelLarge, color = content)
            }
        }
    }
}

/** The week, Monday first: each day a 48 × 64 pill, today's selected, a water dot where something airs. */
@Composable
private fun WeekStrip(week: List<LocalDate>, today: LocalDate, selected: LocalDate, aired: Set<LocalDate>, onDay: (LocalDate) -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.padding(top = 8.dp).padding(horizontal = 16.dp).fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        for (day in week) {
            val chosen = day == selected
            val name = day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale).uppercase(Locale.ROOT)
            Column(
                Modifier
                    .size(48.dp, 64.dp)
                    .clip(RoundedCornerShape(24.dp))
                    .background(if (chosen) colors.primaryContainer else androidx.compose.ui.graphics.Color.Transparent)
                    .clickable { onDay(day) }
                    .semantics {
                        contentDescription = dayHeading.format(day) + (if (day == today) ", today" else "") +
                            (if (day in aired) ", episodes airing" else "")
                    },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterVertically),
            ) {
                Text(name, style = MaterialTheme.typography.labelSmall, color = if (chosen) colors.onPrimaryContainer else colors.onSurfaceVariant)
                Text(
                    day.dayOfMonth.toString(),
                    style = MaterialTheme.typography.bodyLarge.tabular(),
                    color = if (chosen) colors.onPrimaryContainer else colors.onSurface,
                )
                Box(Modifier.size(4.dp).background(if (day in aired) colors.secondary else androidx.compose.ui.graphics.Color.Transparent, CircleShape))
            }
        }
    }
}

/** A broadcast: the cover, the title, the episode and how long ago it aired or until it airs, its time. */
@Composable
private fun BroadcastRow(broadcast: Broadcast, now: Instant, onOpen: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val entry = broadcast.entry
    val seen = (entry.progress?.done ?: 0u) >= broadcast.episode
    val seconds = broadcast.airs.epochSecond - now.epochSecond
    val out = seconds <= 0
    val line = when {
        seen -> "Episode ${broadcast.episode}"
        out -> "Episode ${broadcast.episode} out ${Wording.ago(-seconds)}"
        else -> "Episode ${broadcast.episode} in ${Wording.countdown(seconds)}"
    }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).clickable(onClick = onOpen).padding(start = 16.dp, end = 24.dp, top = 6.dp, bottom = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CoverArt(entry, 40.dp, Modifier.size(40.dp, 60.dp).graphicsLayer { alpha = if (seen) 0.5f else 1f }.shared(Shared.Cover, entry.id, "calendar"))
        Column(Modifier.weight(1f)) {
            Text(
                entry.showTitle,
                style = ShowTitle.row,
                color = if (seen) colors.onSurfaceVariant else colors.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                when {
                    seen -> Icon(Icons.Check, contentDescription = "Watched", tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                    out -> Box(Modifier.size(6.dp).background(colors.secondary, CircleShape))
                }
                Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Text(
            clock.format(broadcast.airs.atZone(ZoneId.systemDefault())),
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = colors.onSurfaceVariant,
        )
    }
}
