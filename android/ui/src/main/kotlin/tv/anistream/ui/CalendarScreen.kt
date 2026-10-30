// The calendar as one scrolling page: My list or All airing, the week's days as a strip that
// jumps to each, and every day's broadcasts under its heading, earlier days loaded on the way up.

package tv.anistream.ui

import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.platform.LocalContext
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorProducer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.lerp
import androidx.compose.foundation.layout.widthIn
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.store.AirTime
import tv.anistream.store.AppClock
import tv.anistream.store.CalendarModel
import tv.anistream.store.ListState
import tv.anistream.store.Wording
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.airTime
import tv.anistream.engine.ffi.calendarMark
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment

/** Whether a title is yours: under way here, or on your Watching or Planning list. */
fun Entry.isMine(): Boolean =
    progress != null || listStatus == LibrarySegment.WATCHING || listStatus == LibrarySegment.PLANNING

private val dayHeading = DateTimeFormatter.ofPattern("EEEE d MMMM", AirTime.locale)

/** "Today", "Tomorrow", "Yesterday", or "Thursday 22 October". */
private fun dayName(context: Context, day: LocalDate, today: LocalDate): String = when (day) {
    today -> Strings.dayToday(context)
    today.plusDays(1) -> Strings.dayTomorrow(context)
    today.minusDays(1) -> Strings.dayYesterday(context)
    else -> dayHeading.format(day)
}

/** What stands at an end of the page besides days: a floor or ceiling line, a load, a failure, or nothing. */
internal enum class Edge { None, Floor, Ceiling, End, Loading, Failed }

/**
 * The page as it stands: the broadcasts by day, every day with a heading, where each heading is in
 * the list and which day each of its items belongs to, read against [now].
 */
@androidx.compose.runtime.Immutable
internal class CalendarPage(
    val now: Instant,
    val today: LocalDate,
    /** Every day the page heads, in order. */
    val days: List<LocalDate>,
    val byDay: Map<LocalDate, List<Broadcast>>,
    /** Whether the page shows only the person's list. */
    val mine: Boolean,
    /** My list is chosen and nothing on it airs from today on. */
    val mineAheadEmpty: Boolean,
    /** How many of today's rows have aired: the now line stands after them. `null` when today has no rows. */
    val nowAfter: Int?,
    /** What stands above the first day and below the last. */
    val top: Edge,
    val bottom: Edge,
    /** The first and last day the calendar reaches, when the engine has said. */
    val firstDay: LocalDate?,
    val lastDay: LocalDate?,
    /** Where each day's heading is in the list. */
    val headings: Map<LocalDate, Int>,
    /** The day each of the list's items belongs to, by its index. */
    val dayOf: List<LocalDate>,
) {
    /** The number of list items the day takes: its heading, its rows or its one line, and the now line. */
    fun itemsOf(day: LocalDate): Int {
        val rows = byDay[day].orEmpty().size
        val line = if (day == today && nowAfter != null) 1 else 0
        return 1 + maxOf(1, rows) + line
    }

    /** Whether [day] is inside what the calendar reaches. */
    fun reaches(day: LocalDate): Boolean = (firstDay == null || day >= firstDay) && (lastDay == null || day <= lastDay)

    companion object {
        /** A page and what it was worked out from. */
        private class Made(
            val window: CalendarModel.Window,
            val titles: Map<AnilistId, Entry>,
            val mine: Boolean,
            val minute: Long,
            val zone: ZoneId,
            val page: CalendarPage,
        )

        /** The last page worked out, kept from one visit to the next. */
        private var last: Made? = null

        /**
         * The page for [window] as [titles] now stand, read at [now]: the last one again while
         * nothing it was made from has changed within the minute, so a return to Calendar does
         * none of this work in the frame it opens on.
         */
        fun of(window: CalendarModel.Window, titles: Map<AnilistId, Entry>, mine: Boolean, now: Instant, zone: ZoneId): CalendarPage {
            val minute = now.epochSecond / 60
            last?.let {
                if (it.window === window && it.titles === titles && it.mine == mine && it.minute == minute && it.zone == zone) return it.page
            }
            return make(window, titles, mine, now, zone).also { last = Made(window, titles, mine, minute, zone, it) }
        }

        private fun make(window: CalendarModel.Window, titles: Map<AnilistId, Entry>, mine: Boolean, now: Instant, zone: ZoneId): CalendarPage {
            val today = LocalDate.ofInstant(now, zone)
            // The titles as they now stand: progress may have moved since the calendar came.
            val broadcasts = window.broadcasts(now).map { b ->
                val known = titles[b.entry.id]
                if (known == null || known.progress == b.entry.progress && known.listStatus == b.entry.listStatus) b
                else b.copy(entry = b.entry.copy(progress = known.progress, listStatus = known.listStatus))
            }
            val shown = if (mine) broadcasts.filter { it.entry.isMine() } else broadcasts
            val byDay = shown.groupBy { LocalDate.ofInstant(it.airs, zone) }.toSortedMap()
            val mineAheadEmpty = mine && byDay.keys.none { it >= today }
            // Every day of this week and the next seven has a heading, so today can stand at the top
            // of the page, and so does every other day with something on it. With nothing on your
            // list ahead, the days after today are left out: the line says so under Today.
            val week = (0L until 7L).map { today.with(DayOfWeek.MONDAY).plusDays(it) }
            val ahead = (0L..7L).map { today.plusDays(it) }
            val days = (byDay.keys + (week + ahead).filter { !mineAheadEmpty || it <= today }).toSortedSet().toList()
            val todayRows = byDay[today].orEmpty()
            val nowAfter = if (todayRows.isEmpty()) null else todayRows.count { !it.airs.isAfter(now) }
            val firstDay = window.floor?.let { LocalDate.ofInstant(Instant.ofEpochSecond(it), zone) }
            val lastDay = window.ceiling?.let { LocalDate.ofInstant(Instant.ofEpochSecond(it), zone) }
            val top = when {
                window.pastFailed -> Edge.Failed
                window.loadingPast || window.morePast -> Edge.Loading
                firstDay != null -> Edge.Floor
                else -> Edge.None
            }
            val lastLoaded = byDay.keys.lastOrNull()
            val bottom = when {
                window.aheadFailed -> Edge.Failed
                window.loadingAhead || window.moreAhead -> Edge.Loading
                lastDay != null && (lastLoaded == null || lastLoaded.plusDays(2) >= lastDay) -> Edge.Ceiling
                window.ceiling != null -> Edge.End
                else -> Edge.None
            }
            val headings = mutableMapOf<LocalDate, Int>()
            val dayOf = mutableListOf<LocalDate>()
            // The top edge is the list's first item and the bottom edge its last.
            dayOf += days.firstOrNull() ?: today
            val page = CalendarPage(now, today, days, byDay, mine, mineAheadEmpty, nowAfter, top, bottom, firstDay, lastDay, headings, dayOf)
            for (day in days) {
                headings[day] = dayOf.size
                repeat(page.itemsOf(day)) { dayOf += day }
            }
            dayOf += days.lastOrNull() ?: today
            return page
        }
    }
}

@Composable
fun CalendarScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val window by store.calendar.window.collectAsStateWithLifecycle()
    val state by store.calendar.list.state.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadCalendar() }

    // All airing until the person chooses My list, and then it stays chosen.
    val mine = remember(settings) { store.calendarMine() }
    // The minute the hairline reads: the page is worked out again each time it turns.
    val minute by produceState(AppClock.now().epochSecond / 60) {
        while (true) {
            delay(60_000 - AppClock.now().toEpochMilli() % 60_000)
            value = AppClock.now().epochSecond / 60
        }
    }
    // Worked out once each time the calendar, the titles, the choice or the minute changes, never
    // again on a frame of a scroll or a transition.
    val page = remember(window, titles, mine, minute) { CalendarPage.of(window, titles, mine, AppClock.now(), AppClock.zone) }
    val answered = state is ListState.Loaded || state is ListState.Empty
    val offline by store.offline.collectAsStateWithLifecycle()

    // The list opens on today: it is made once the calendar is here, with today's heading at its
    // top, so the page never draws another day first and then jumps. It is kept with the
    // destination, so a later visit comes back where the last one left it.
    val list = rememberSaveable(answered, saver = LazyListState.Saver) {
        LazyListState(if (answered) page.headings[page.today.plusDays(store.calendar.opensOn.toLong())] ?: 0 else 0)
    }
    val scope = rememberCoroutineScope()
    // The strip follows the page: the day at its top is selected. Read by the strip alone, so a
    // new day at the top redraws the strip and nothing else.
    val selected = remember(list, page, answered) {
        derivedStateOf { if (answered) page.dayOf.getOrNull(list.firstVisibleItemIndex) ?: page.today else page.today }
    }
    // Today's section has left the screen: the Today button appears.
    val away = remember(list, page, answered) {
        derivedStateOf {
            val start = page.headings[page.today]
            if (!answered || start == null) false
            else {
                val end = start + page.itemsOf(page.today) - 1
                val info = list.layoutInfo
                val last = info.visibleItemsInfo.lastOrNull()?.index ?: 0
                list.firstVisibleItemIndex > end || last < start
            }
        }
    }
    // A day the strip asked for that is not in the list yet: scrolled to once its stretch arrives.
    var pendingDay by remember { mutableStateOf<LocalDate?>(null) }
    LaunchedEffect(pendingDay, page) {
        val day = pendingDay ?: return@LaunchedEffect
        val heading = page.headings[day]
        if (heading != null) {
            list.animateScrollToItem(heading)
            pendingDay = null
        } else if (!window.loadingPast && !window.loadingAhead) {
            // The stretch is in and the day has nothing on it: stop at the nearest day after it.
            page.days.firstOrNull { it > day }?.let { page.headings[it] }?.let { list.animateScrollToItem(it) }
            pendingDay = null
        }
    }
    // More of the calendar as either end comes within a screen, watched in a flow so a scroll never
    // recomposes the page.
    LaunchedEffect(list, answered) {
        if (!answered) return@LaunchedEffect
        snapshotFlow {
            val info = list.layoutInfo
            val lastVisible = info.visibleItemsInfo.lastOrNull()?.index ?: 0
            (list.firstVisibleItemIndex <= EDGE_ITEMS) to (lastVisible >= info.totalItemsCount - 1 - EDGE_ITEMS)
        }.collect { (top, bottom) ->
            if (top) store.loadCalendarPast()
            if (bottom) store.loadCalendarAhead()
        }
    }

    ScreenFrame(
        title = stringResource(R.string.nav_calendar),
        actions = {
            // Fades in as the last of today leaves the screen, and out as any of it returns.
            AnimatedVisibility(away.value, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
                TextButton(onClick = {
                    scope.launch {
                        val start = page.headings[page.today] ?: return@launch
                        // Jump to one screen short of today, then scroll the last stretch on standard.
                        val below = list.firstVisibleItemIndex > start
                        list.scrollToItem((if (below) start + EDGE_ITEMS else start - EDGE_ITEMS).coerceAtLeast(0))
                        list.animateScrollToItem(start)
                    }
                }) { Text(stringResource(R.string.day_today)) }
            }
        },
    ) { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            // Pinned above the days: which broadcasts, and the week's days to jump to.
            ConnectedChoice(
                choices = listOf(stringResource(R.string.calendar_filter_all), stringResource(R.string.calendar_filter_mine)),
                selected = if (mine) 1 else 0,
                onSelect = { store.setCalendarMine(it == 1) },
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            WeekStrip(page, { selected.value }) { day ->
                val heading = page.headings[day]
                if (heading != null) {
                    scope.launch { list.animateScrollToItem(heading) }
                } else if (page.firstDay != null && day < page.days.first()) {
                    pendingDay = day
                    store.loadCalendarThrough(day.atStartOfDay(AppClock.zone).toEpochSecond(), earlier = true)
                } else {
                    pendingDay = day
                    store.loadCalendarThrough(day.plusDays(1).atStartOfDay(AppClock.zone).toEpochSecond(), earlier = false)
                }
            }
            // Nothing for the first 600 ms, then placeholders, then the days in one crossfade (T8).
            // The gap above the days is outside the list, so a day scrolled to the top leaves
            // nothing of the one before it showing under the strip.
            // A failure is its own state, never a week of empty days; offline with nothing here
            // says so in place of the placeholders.
            val failed = state as? ListState.Failed
            if (failed != null) {
                FailedState(stringResource(R.string.calendar_load_failed), failed.reason, retry = { store.loadCalendar(force = true) })
            } else {
                Loading(
                    page.takeIf { answered },
                    Modifier.padding(top = 8.dp),
                    placeholders = { if (offline) OfflineState() else CalendarPlaceholders() },
                ) {
                    Refreshable({ store.loadCalendar(force = true) }) {
                        CalendarDays(
                            page,
                            list,
                            onOpen = { navigator.openTitle(it, "calendar") },
                            onShowAll = { store.setCalendarMine(false) },
                            onRetryPast = { store.retryCalendarPast() },
                            onRetryAhead = { store.loadCalendarAhead(retry = true) },
                        )
                    }
                }
            }
        }
    }
}

/** How many list items make a screen: the ends are asked for, and Today jumps, this far off. */
private const val EDGE_ITEMS = 6

/**
 * Every day's broadcasts under its heading. Headings, rows and "Nothing airing" all move together
 * as the rows change, each with its kind for the list to reuse. The first and last items are the
 * ends: the floor or ceiling line, a load, or a failure with Try again.
 */
@Composable
private fun CalendarDays(
    page: CalendarPage,
    list: LazyListState,
    onOpen: (AnilistId) -> Unit,
    onShowAll: () -> Unit,
    onRetryPast: () -> Unit,
    onRetryAhead: () -> Unit,
) {
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
        item(key = "edge-top", contentType = "edge") { EdgeLine(page.top, earlier = true, onRetry = onRetryPast) }
        for (day in page.days) {
            item(key = "day-$day", contentType = "day") {
                Text(
                    dayName(LocalContext.current, day, page.today),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = itemMotion().padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
                )
            }
            val rows = page.byDay[day].orEmpty()
            if (day == page.today && page.mineAheadEmpty) {
                item(key = "none-$day", contentType = "none") {
                    Column(itemMotion().padding(start = 16.dp, end = 16.dp, bottom = 8.dp)) {
                        Text(
                            stringResource(R.string.calendar_empty_mine_ahead),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        TextButton(onClick = onShowAll) { Text(stringResource(R.string.calendar_show_all_airing)) }
                    }
                }
            } else if (rows.isEmpty()) {
                item(key = "none-$day", contentType = "none") {
                    Text(
                        stringResource(R.string.calendar_empty_title),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = itemMotion().padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                    )
                }
            } else {
                val line = if (day == page.today) page.nowAfter else null
                rows.forEachIndexed { index, broadcast ->
                    if (line == index) item(key = "now-$day", contentType = "now") { NowLine(page.now) }
                    item(key = "row-${broadcast.entry.id}-${broadcast.episode}-${broadcast.airs.epochSecond}", contentType = "row") {
                        Box(itemMotion()) { BroadcastRow(broadcast, page.now) { onOpen(broadcast.entry.id) } }
                    }
                }
                // Every broadcast has aired: the line closes the day.
                if (line == rows.size) item(key = "now-$day", contentType = "now") { NowLine(page.now) }
            }
        }
        item(key = "edge-bottom", contentType = "edge") { EdgeLine(page.bottom, earlier = false, onRetry = onRetryAhead) }
    }
}

/**
 * The line that marks the time now, between the last broadcast that has aired and the next: silk,
 * 1 dp, with the time at the trailing edge where the times sit. It overlays the separator and
 * takes no room of its own beyond the label.
 */
@Composable
private fun NowLine(now: Instant) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val time = airTime(AirTime.clock(now), false)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 24.dp)
            .semantics(mergeDescendants = true) { contentDescription = Strings.calendarNowLabel(context, time) },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(Modifier.weight(1f).height(1.dp).background(colors.onSurface))
        Text(time, style = MaterialTheme.typography.labelMedium.tabular(), color = colors.onSurface)
    }
}

/**
 * What stands at an end of the calendar, quiet: the floor or ceiling line, nothing for the first
 * 600 ms of a load, placeholders after that and the activity line after 4 s; a failure keeps what
 * arrived and puts one line with Try again in place.
 */
@Composable
private fun EdgeLine(edge: Edge, earlier: Boolean, onRetry: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    when (edge) {
        Edge.None -> Spacer(Modifier.height(1.dp))
        Edge.Floor, Edge.Ceiling, Edge.End -> Text(
            when (edge) {
                Edge.Floor -> Strings.calendarFloorLine(context)
                Edge.Ceiling -> Strings.calendarCeilingLine(context)
                else -> Strings.calendarEndLine(context)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 16.dp),
        )
        Edge.Loading -> {
            var waited by remember { mutableStateOf(0) }
            LaunchedEffect(Unit) {
                delay(600)
                waited = 1
                delay(3_400)
                waited = 2
            }
            if (waited >= 1) {
                Column {
                    TextPlaceholder(0.25f, 24.dp, Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                    TextPlaceholder(0.7f, 24.dp, Modifier.padding(horizontal = 16.dp))
                    if (waited >= 2) {
                        Text(
                            if (earlier) Strings.calendarLoadingEarlierLine(context) else Strings.calendarLoadingLaterLine(context),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        )
                    }
                }
            } else {
                Spacer(Modifier.height(1.dp))
            }
        }
        Edge.Failed -> Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Vermilion on the glyph only.
            Icon(Icons.Problem, contentDescription = null, tint = colors.error, modifier = Modifier.size(20.dp))
            Text(
                if (earlier) Strings.calendarEarlierFailedLine(context) else Strings.calendarLaterFailedLine(context),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onRetry) { Text(stringResource(R.string.action_try_again)) }
        }
    }
}

/** The days before the calendar arrives: a heading and rows as flat shapes, at their real sizes (T8). */
@Composable
private fun CalendarPlaceholders() {
    Column {
        TextPlaceholder(0.25f, 24.dp, Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        repeat(6) {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(start = 16.dp, end = 24.dp, top = 6.dp, bottom = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.size(40.dp, 60.dp).clip(ArtShape.cover(40.dp)).background(MaterialTheme.colorScheme.surfaceContainer))
                Column(Modifier.weight(1f)) {
                    TextPlaceholder(0.7f, 24.dp)
                    TextPlaceholder(0.45f, 20.dp)
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
    Row(modifier.fillMaxWidth().heightIn(min = 48.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
        choices.forEachIndexed { index, choice ->
            ConnectedButton(
                chosen = index == selected,
                first = index == 0,
                last = index == choices.lastIndex,
                onClick = { onSelect(index) },
                gap = 8.dp,
            ) { color ->
                BasicText(choice, style = MaterialTheme.typography.labelLarge, color = color)
            }
        }
    }
}

/**
 * One button of a connected group, [content] beside its check: fully round outside and all round
 * once chosen, 8 between, on primaryContainer once chosen, [weight] wide against the others. A new
 * choice morphs its corners on the motion scheme's spatial spring and its colours on its effects
 * spring, as a ToggleButton's do, read as the button is drawn, so the morph recomposes nothing;
 * the check fades in on fade-in as its room opens on standard, and the old one's leaves on quick
 * (T7). [content] draws in the colour it is handed, as it stands at each frame.
 */
@Composable
internal fun RowScope.ConnectedButton(
    chosen: Boolean,
    first: Boolean,
    last: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    /** As the finger lands, before it is known to be a tap. */
    onPress: (() -> Unit)? = null,
    /** As it lifts or the touch is taken away, a tap or not. */
    onRelease: (() -> Unit)? = null,
    enabled: Boolean = true,
    weight: Float = 1f,
    gap: Dp = 6.dp,
    content: @Composable RowScope.(color: ColorProducer) -> Unit,
) = ConnectedButtonContent(chosen, first, last, onClick, modifier.weight(weight), onPress, onRelease, enabled, gap, content)

/**
 * [ConnectedButton] as wide as its parent measures it, for a group that lays its buttons out
 * itself, as the season picker does.
 */
@Composable
internal fun ConnectedButtonContent(
    chosen: Boolean,
    first: Boolean,
    last: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onPress: (() -> Unit)? = null,
    onRelease: (() -> Unit)? = null,
    enabled: Boolean = true,
    gap: Dp = 6.dp,
    content: @Composable RowScope.(color: ColorProducer) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val motion = MaterialTheme.motionScheme
    // How far chosen, 0 to 1: one spring for the shape, one for the colours.
    val shape = animateFloatAsState(if (chosen) 1f else 0f, motion.defaultSpatialSpec(), label = "shape") // i18n-skip
    val tone = animateFloatAsState(if (chosen) 1f else 0f, motion.defaultEffectsSpec(), label = "tone") // i18n-skip
    val color = remember(colors, tone) { ColorProducer { lerp(colors.onSurface, colors.onPrimaryContainer, tone.value) } }
    Row(
        modifier
            .heightIn(min = 40.dp)
            .graphicsLayer {
                val start = lerp(if (first) 20.dp else 8.dp, 20.dp, shape.value)
                val end = lerp(if (last) 20.dp else 8.dp, 20.dp, shape.value)
                this.shape = RoundedCornerShape(topStart = start, bottomStart = start, topEnd = end, bottomEnd = end)
                clip = true
            }
            .drawBehind { drawRect(lerp(colors.surfaceContainer, colors.primaryContainer, tone.value)) }
            .then(if (onPress == null || !enabled) Modifier else Modifier.pressWatch(onPress, onRelease))
            .clickable(enabled = enabled, onClick = onClick)
            .semantics { selected = chosen },
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AnimatedVisibility(chosen, enter = markIn, exit = markOut) {
            Icon(Icons.Check, contentDescription = null, tint = colors.onPrimaryContainer, modifier = Modifier.padding(end = gap).size(18.dp))
        }
        content(color)
    }
}

/**
 * [onPress] as a finger lands and [onRelease] as it lifts or the touch is taken away, watched
 * ahead of the button's own handling and consuming nothing.
 */
@Composable
private fun Modifier.pressWatch(onPress: () -> Unit, onRelease: (() -> Unit)?): Modifier {
    val press by rememberUpdatedState(onPress)
    val release by rememberUpdatedState(onRelease)
    return pointerInput(Unit) {
        awaitEachGesture {
            awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            press()
            waitForUpOrCancellation(PointerEventPass.Initial)
            release?.invoke()
        }
    }
}

/** A connected button's check, or anything else that comes with the choice: in as its room opens. */
internal val markIn = fadeIn(Motion.fadeIn()) + expandHorizontally(Motion.standard())

/** The old choice's check leaving: out on quick as its room closes. */
internal val markOut = fadeOut(Motion.quick()) + shrinkHorizontally(Motion.standard())

/**
 * The week of the selected day, Monday first: each day a 48 × 64 pill, the selected one tinted, a
 * water dot where something airs. Today wears a 1 dp ring round its numeral while another day is
 * selected, and its weekday turns silk. Days outside what the calendar reaches drop to 32 % and
 * cannot be picked.
 */
@Composable
private fun WeekStrip(page: CalendarPage, selected: () -> LocalDate, onDay: (LocalDate) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val chosenDay = selected()
    val week = (0L until 7L).map { chosenDay.with(DayOfWeek.MONDAY).plusDays(it) }
    val today = page.today
    val aired = page.byDay.keys
    Row(
        Modifier.padding(top = 8.dp).padding(horizontal = 16.dp).fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        for (day in week) {
            val name = day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)
            // The pill moves from day to day on quick, as the page scrolls past them or one is
            // tapped: drawn, so it redraws the pill alone.
            val dayText = dayHeading.format(day)
            val context = LocalContext.current
            val dayLabel = when {
                day == today && day in aired -> Strings.calendarDayLabelTodayAiring(context, dayText)
                day == today -> Strings.calendarDayLabelToday(context, dayText)
                day in aired -> Strings.calendarDayLabelAiring(context, dayText)
                else -> Strings.calendarDayLabel(context, dayText)
            }
            val on by animateFloatAsState(if (day == chosenDay) 1f else 0f, Motion.quick(), label = "day") // i18n-skip
            val reaches = page.reaches(day)
            val ringed = day == today && day != chosenDay
            Column(
                Modifier
                    .widthIn(min = 48.dp)
                    .heightIn(min = 64.dp)
                    .graphicsLayer { alpha = if (reaches) 1f else 0.32f }
                    .clip(RoundedCornerShape(24.dp))
                    .drawBehind { drawRect(colors.primaryContainer.copy(alpha = on)) }
                    .clickable(enabled = reaches) { onDay(day) }
                    .semantics {
                        contentDescription = dayLabel
                    },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterVertically),
            ) {
                // One line however large the text: three letters fit the pill at every size the
                // system's font scale reaches, so the name never wraps and nothing has to measure
                // it again at smaller sizes.
                Text(
                    name,
                    style = MaterialTheme.typography.labelSmall,
                    color = lerp(if (ringed) colors.onSurface else colors.onSurfaceVariant, colors.onPrimaryContainer, on),
                    maxLines = 1,
                    softWrap = false,
                )
                Box(
                    Modifier.size(30.dp).then(if (ringed) Modifier.border(1.dp, colors.onSurface, CircleShape) else Modifier),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        day.dayOfMonth.toString(),
                        style = MaterialTheme.typography.bodyLarge.tabular(),
                        color = lerp(colors.onSurface, colors.onPrimaryContainer, on),
                    )
                }
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
    // Dub, the only mark, goes with the episode it is about: "Episode 10 · Dub out 2 hours ago".
    val context = LocalContext.current
    val episode = if (calendarMark(entry) != null) Strings.episodeLongDubbed(context, broadcast.episode.toInt()) else Strings.episodeNumber(context, broadcast.episode.toInt())
    val line = when {
        seen -> episode
        out -> Strings.calendarLineOut(context, episode, Wording.ago(context, -seconds))
        else -> Strings.calendarLineIn(context, episode, Wording.countdown(context, seconds))
    }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).shared(Shared.Page, entry.id, "calendar").clickable(onClickLabel = stringResource(R.string.title_open), onClick = onOpen).padding(start = 16.dp, end = 24.dp, top = 6.dp, bottom = 6.dp),
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
                    seen -> Icon(Icons.Check, contentDescription = stringResource(R.string.episode_watched), tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                    out -> Box(Modifier.size(6.dp).background(colors.secondary, CircleShape))
                }
                Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Text(
            airTime(AirTime.clock(broadcast.airs), entry.airingApproximate),
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = colors.onSurfaceVariant,
        )
    }
}
