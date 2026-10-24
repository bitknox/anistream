// The calendar as one scrolling page: My list or All airing, the week's days as a strip that
// jumps to each, and every day's broadcasts under its heading, earlier days loaded on the way up.

package tv.anistream.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment

/** Whether a title is yours: under way here, or on your Watching or Planning list. */
fun Entry.isMine(): Boolean =
    progress != null || listStatus == LibrarySegment.WATCHING || listStatus == LibrarySegment.PLANNING

/** Calendar's two choices: the titles of yours, or everything airing. */
private val SHOW_CHOICES = listOf("My list", "All airing")

private val dayHeading = DateTimeFormatter.ofPattern("EEEE d MMMM", AirTime.locale)

/** "Today", "Tomorrow", "Yesterday", or "Thursday 22 October". */
private fun dayName(day: LocalDate, today: LocalDate): String = when (day) {
    today -> "Today"
    today.plusDays(1) -> "Tomorrow"
    today.minusDays(1) -> "Yesterday"
    else -> dayHeading.format(day)
}

/**
 * The page as it stands: the broadcasts by day, every day with a heading, where each heading is in
 * the list and which day each of its items belongs to, read against [now].
 */
@androidx.compose.runtime.Immutable
internal class CalendarPage(
    val now: Instant,
    val today: LocalDate,
    /** This week's days, Monday first, for the strip. */
    val week: List<LocalDate>,
    /** Every day the page heads, in order. */
    val days: List<LocalDate>,
    val byDay: Map<LocalDate, List<Broadcast>>,
    /** Whether any broadcast is of a title of yours, which picks My list when nothing else has. */
    val anyMine: Boolean,
    /** Where each day's heading is in the list. */
    val headings: Map<LocalDate, Int>,
    /** The day each of the list's items belongs to, by its index. */
    val dayOf: List<LocalDate>,
) {
    companion object {
        /** A page and what it was worked out from. */
        private class Made(
            val window: CalendarModel.Window,
            val titles: Map<AnilistId, Entry>,
            val mine: Boolean?,
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
        fun of(window: CalendarModel.Window, titles: Map<AnilistId, Entry>, mine: Boolean?, now: Instant, zone: ZoneId): CalendarPage {
            val minute = now.epochSecond / 60
            last?.let {
                if (it.window === window && it.titles === titles && it.mine == mine && it.minute == minute && it.zone == zone) return it.page
            }
            return make(window, titles, mine, now, zone).also { last = Made(window, titles, mine, minute, zone, it) }
        }

        private fun make(window: CalendarModel.Window, titles: Map<AnilistId, Entry>, mine: Boolean?, now: Instant, zone: ZoneId): CalendarPage {
            val today = LocalDate.ofInstant(now, zone)
            // The titles as they now stand: progress may have moved since the calendar came.
            val broadcasts = window.broadcasts(now).map { b ->
                val known = titles[b.entry.id]
                if (known == null || known.progress == b.entry.progress && known.listStatus == b.entry.listStatus) b
                else b.copy(entry = b.entry.copy(progress = known.progress, listStatus = known.listStatus))
            }
            val anyMine = broadcasts.any { it.entry.isMine() }
            val shown = if (mine ?: anyMine) broadcasts.filter { it.entry.isMine() } else broadcasts
            val byDay = shown.groupBy { LocalDate.ofInstant(it.airs, zone) }.toSortedMap()
            val week = (0L until 7L).map { today.with(DayOfWeek.MONDAY).plusDays(it) }
            // Every day of this week and the next seven has a heading, so today can stand at the top
            // of the page, and so does every other day with something on it.
            val ahead = (0L..7L).map { today.plusDays(it) }
            val days = (byDay.keys + week + ahead).toSortedSet().toList()
            val headings = mutableMapOf<LocalDate, Int>()
            val dayOf = mutableListOf<LocalDate>()
            for (day in days) {
                headings[day] = dayOf.size
                repeat(1 + maxOf(1, byDay[day]?.size ?: 0)) { dayOf += day }
            }
            return CalendarPage(now, today, week, days, byDay, anyMine, headings, dayOf)
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
    LaunchedEffect(Unit) { store.loadCalendar() }

    var mine by rememberSaveable { mutableStateOf<Boolean?>(null) }
    // Worked out once each time the calendar, the titles or the choice changes, never again on a
    // frame of a scroll or a transition. The time the rows read their words off is taken then too.
    val page = remember(window, titles, mine) { CalendarPage.of(window, titles, mine, AppClock.now(), AppClock.zone) }
    val showMine = mine ?: page.anyMine
    val answered = state is ListState.Loaded || state is ListState.Empty
    val offline by store.offline.collectAsStateWithLifecycle()

    // The list opens on today: it is made once the calendar is here, with today's heading at its
    // top, so the page never draws another day first and then jumps. It is kept with the
    // destination, so a later visit comes back where the last one left it.
    val list = rememberSaveable(answered, saver = LazyListState.Saver) {
        LazyListState(if (answered) page.headings[page.today] ?: 0 else 0)
    }
    val scope = rememberCoroutineScope()
    // The strip follows the page: the day at its top is selected. Read by the strip alone, so a
    // new day at the top redraws the strip and nothing else.
    val selected = remember(list, page, answered) {
        derivedStateOf { if (answered) page.dayOf.getOrNull(list.firstVisibleItemIndex) ?: page.today else page.today }
    }
    // Earlier broadcasts, as the page reaches its top: watched in a flow, so a scroll never
    // recomposes the page.
    LaunchedEffect(list, answered) {
        if (!answered) return@LaunchedEffect
        snapshotFlow { list.firstVisibleItemIndex == 0 && window.morePast }.collect { top ->
            val anchor = window.anchor
            if (top && anchor != null) store.send(Command.LoadCalendarPast(anchor, window.pages))
        }
    }

    ScreenFrame(title = "Calendar") { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            // Pinned above the days: which broadcasts, and the week's days to jump to.
            ConnectedChoice(
                choices = SHOW_CHOICES,
                selected = if (showMine) 0 else 1,
                onSelect = { mine = it == 0 },
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            WeekStrip(page.week, page.today, { selected.value }, aired = page.byDay.keys) { day ->
                page.headings[day]?.let { scope.launch { list.animateScrollToItem(it) } }
            }
            // Nothing for the first 600 ms, then placeholders, then the days in one crossfade (T8).
            // The gap above the days is outside the list, so a day scrolled to the top leaves
            // nothing of the one before it showing under the strip.
            // A failure is its own state, never a week of empty days; offline with nothing here
            // says so in place of the placeholders.
            val failed = state as? ListState.Failed
            if (failed != null) {
                FailedState("Couldn't load the calendar", failed.reason, retry = { store.loadCalendar(force = true) })
            } else {
                Loading(
                    page.takeIf { answered },
                    Modifier.padding(top = 8.dp),
                    placeholders = { if (offline) OfflineState() else CalendarPlaceholders() },
                ) {
                    Refreshable({ store.loadCalendar(force = true) }) {
                        CalendarDays(page, list) { navigator.openTitle(it, "calendar") }
                    }
                }
            }
        }
    }
}

/**
 * Every day's broadcasts under its heading. Headings, rows and "Nothing airing" all move together
 * as the rows change, each with its kind for the list to reuse.
 */
@Composable
private fun CalendarDays(page: CalendarPage, list: LazyListState, onOpen: (AnilistId) -> Unit) {
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
        for (day in page.days) {
            item(key = "day-$day", contentType = "day") {
                Text(
                    dayName(day, page.today),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = itemMotion().padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
                )
            }
            val rows = page.byDay[day].orEmpty()
            if (rows.isEmpty()) {
                item(key = "none-$day", contentType = "none") {
                    Text(
                        "Nothing airing",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = itemMotion().padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                    )
                }
            } else {
                items(rows, key = { "row-${it.entry.id}-${it.episode}-${it.airs.epochSecond}" }, contentType = { "row" }) { broadcast ->
                    Box(itemMotion()) { BroadcastRow(broadcast, page.now) { onOpen(broadcast.entry.id) } }
                }
            }
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
    val shape = animateFloatAsState(if (chosen) 1f else 0f, motion.defaultSpatialSpec(), label = "shape")
    val tone = animateFloatAsState(if (chosen) 1f else 0f, motion.defaultEffectsSpec(), label = "tone")
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

/** The week, Monday first: each day a 48 × 64 pill, today's selected, a water dot where something airs. */
@Composable
private fun WeekStrip(week: List<LocalDate>, today: LocalDate, selected: () -> LocalDate, aired: Set<LocalDate>, onDay: (LocalDate) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val chosenDay = selected()
    Row(
        Modifier.padding(top = 8.dp).padding(horizontal = 16.dp).fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        for (day in week) {
            val name = day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)
            // The pill moves from day to day on quick, as the page scrolls past them or one is
            // tapped: drawn, so it redraws the pill alone.
            val on by animateFloatAsState(if (day == chosenDay) 1f else 0f, Motion.quick(), label = "day")
            Column(
                Modifier
                    .widthIn(min = 48.dp)
                    .heightIn(min = 64.dp)
                    .clip(RoundedCornerShape(24.dp))
                    .drawBehind { drawRect(colors.primaryContainer.copy(alpha = on)) }
                    .clickable { onDay(day) }
                    .semantics {
                        contentDescription = dayHeading.format(day) + (if (day == today) ", today" else "") +
                            (if (day in aired) ", episodes airing" else "")
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
                    color = lerp(colors.onSurfaceVariant, colors.onPrimaryContainer, on),
                    maxLines = 1,
                    softWrap = false,
                )
                Text(
                    day.dayOfMonth.toString(),
                    style = MaterialTheme.typography.bodyLarge.tabular(),
                    color = lerp(colors.onSurface, colors.onPrimaryContainer, on),
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
        Modifier.fillMaxWidth().heightIn(min = 72.dp).clickable(onClickLabel = "Open title", onClick = onOpen).padding(start = 16.dp, end = 24.dp, top = 6.dp, bottom = 6.dp),
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
            AirTime.clock(broadcast.airs),
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = colors.onSurfaceVariant,
        )
    }
}
