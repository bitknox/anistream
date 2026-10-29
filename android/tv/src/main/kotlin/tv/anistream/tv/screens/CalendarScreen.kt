// Calendar: a week as day tabs, and the chosen day's broadcasts as 72-high rows in air order, so
// the text stays large. The next airing today is the first focus; past rows sit in haze above it.

package tv.anistream.tv.screens

import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import android.content.Context
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.TonalButton
import kotlinx.coroutines.launch
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
import tv.anistream.engine.ffi.airTime
import tv.anistream.engine.ffi.calendarMark
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
import tv.anistream.store.plainReason
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
import androidx.compose.ui.platform.LocalContext

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

/** "Today", then "Wed 21" and so on; the 1st names its month: "Sun 1 Nov". */
fun dayLabel(context: Context, day: LocalDate, today: LocalDate): String = when {
    day == today -> Strings.dayToday(context)
    day.dayOfMonth == 1 ->
        "${day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)} 1 ${day.month.getDisplayName(TextStyle.SHORT, AirTime.locale)}"
    else -> "${day.dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)} ${day.dayOfMonth}"
}

/**
 * When it airs on the TV's clock: "16:30", "~16:30" for an estimate, or "16:30 (was 23:00)" when
 * AniList moved it, as the phone writes it.
 */
fun Airing.timeLine(context: Context): String {
    val time = airTime(AirTime.clock(at), entry.airingApproximate)
    val was = entry.delayedFrom?.let { AirTime.clock(Instant.ofEpochSecond(it)) }
    return if (was != null && was != time) Strings.tvCalendarTimeWas(context, time, was) else time
}

/** "Dub" when the engine marks the row as the dub's, else null: only dubs are marked. */
fun Airing.dubbed(): Boolean = calendarMark(entry) != null

/** The line under the title: "Episode 10", with Dub after it when the engine marks one: "Episode 10 · Dub". */
fun Airing.episodeLine(context: Context): String =
    if (dubbed()) Strings.episodeLongDubbed(context, episode.toInt()) else Strings.episodeNumber(context, episode.toInt())

/** [timeLine] drawn: the time in the row's colour, "(was 23:00)" after it in [quiet]. */
private fun Airing.timeText(context: Context, quiet: Color): AnnotatedString {
    val line = timeLine(context)
    val was = line.indexOf(" (was ")
    if (was < 0) return AnnotatedString(line)
    return buildAnnotatedString {
        append(line.substring(0, was))
        withStyle(SpanStyle(color = quiet)) { append(line.substring(was)) }
    }
}

/** "E7 of 24" for a show on a list, as text on its row. */
private fun Airing.listProgress(context: Context): String? {
    if (entry.listStatus == null) return null
    val done = entry.progress?.done ?: return null
    return entry.episodes?.let { Strings.widgetLineOf(context, done.toInt(), it.toInt()) } ?: Strings.episodeNumberShort(context, done.toInt())
}

/** The tabs start two weeks back; the last is the ceiling the engine reports, four weeks ahead until it says. */
private const val DAYS_BACK = 14
private const val DAYS_AHEAD = 28

@Composable
fun CalendarScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    LaunchedEffect(Unit) { store.loadCalendar() }
    val window by store.calendar.window.collectAsStateWithLifecycle()
    val state by store.calendar.list.state.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val signedIn = trackers.values.any { it.connected }
    val settings by store.settings.view.collectAsStateWithLifecycle()
    // All airing until the person chooses My list, and then it stays chosen.
    val mine = remember(settings) { store.calendarMine() }
    val onlyMine = mine && signedIn
    val now = AppClock.now()
    val zone = AppClock.zone
    val today = now.atZone(zone).toLocalDate()
    // The last tab is the ceiling, counted from the anchor the engine reckoned it against.
    val ceilingDays = window.ceiling?.let { ceiling -> window.anchor?.let { ((ceiling - it) / 86_400L).toInt() } }
        ?.coerceAtLeast(0) ?: DAYS_AHEAD
    var offset by rememberSaveable { mutableIntStateOf(store.calendar.opensOn.coerceIn(-DAYS_BACK, ceilingDays)) }
    val day = today.plusDays(offset.toLong())
    val airings = remember(window) { window.airings(now) }
    val shown = airings.filter { it.at.atZone(zone).toLocalDate() == day && (!onlyMine || it.entry.listStatus != null) }
    // The next airing today leads; on another day, its first.
    val lead = shown.indexOfFirst { it.at >= now }.takeIf { it >= 0 } ?: 0
    // The line that marks now stands before the lead row; with every broadcast aired, after the last.
    val nowAt = if (offset == 0 && shown.isNotEmpty()) shown.indexOfFirst { it.at >= now }.takeIf { it >= 0 } ?: shown.size else null
    val list = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, lead - 1))
    val tabs = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, DAYS_BACK + offset - 1))
    val scope = rememberCoroutineScope()
    // Back steps from a row to its tab, then to Today, then lets the rail have it.
    DisposableEffect(nav, memory) {
        val mine = {
            when {
                memory.last?.startsWith("airing/") == true && memory.focus("day/$offset") -> true
                offset != 0 -> {
                    offset = 0
                    scope.launch {
                        tabs.scrollToItem(maxOf(0, DAYS_BACK - 1))
                        repeat(5) {
                            withFrameNanos { }
                            if (memory.focus("day/0")) return@launch
                        }
                    }
                    true
                }
                else -> false
            }
        }
        nav.toTop = mine
        onDispose { if (nav.toTop === mine) nav.toTop = null }
    }
    // A day outside what is loaded asks the engine for the stretch it sits in.
    LaunchedEffect(offset, window.anchor, window.morePast, window.moreAhead, window.loadingPast, window.loadingAhead) {
        val start = day.atStartOfDay(zone).toInstant()
        val first = airings.firstOrNull()?.at
        val last = airings.lastOrNull()?.at
        if (window.morePast && first != null && start < first) store.loadCalendarThrough(start.epochSecond, earlier = true)
        if (window.moreAhead && last != null && start > last) store.loadCalendarThrough(start.plusSeconds(86_400).epochSecond, earlier = false)
    }
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
        androidx.compose.foundation.lazy.LazyRow(state = tabs, modifier = Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(DAYS_BACK + ceilingDays + 1, key = { it }) { index ->
                val tabOffset = index - DAYS_BACK
                TvTab(
                    dayLabel(context, today.plusDays(tabOffset.toLong()), today),
                    selected = tabOffset == offset,
                    onSelect = { offset = tabOffset },
                    modifier = Modifier.firstFocus(memory, "day/$tabOffset", shown.isEmpty() && tabOffset == offset),
                )
            }
        }
        // Under the days: My list when signed in, All airing otherwise; not changeable until sign-in.
        Row(Modifier.padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvTab(stringResource(R.string.calendar_filter_all), selected = !onlyMine, onSelect = { store.setCalendarMine(false) }, settle = null, chip = true, modifier = Modifier.remembered(memory, "chip/all"))
                if (signedIn) {
                    TvTab(stringResource(R.string.calendar_filter_mine), selected = onlyMine, onSelect = { store.setCalendarMine(true) }, settle = null, chip = true, modifier = Modifier.remembered(memory, "chip/mine"))
                }
            }
            if (!signedIn) {
                Text(
                    stringResource(R.string.tv_calendar_signed_out_note),
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
                stringResource(R.string.calendar_load_failed),
                Modifier.padding(top = 48.dp),
                detail = plainReason(LocalContext.current, failed.reason),
                action = stringResource(R.string.action_try_again),
                onAction = { store.loadCalendar(force = true) },
                actionModifier = Modifier.firstFocus(memory, "retry"),
            )
            shown.isEmpty() && !loading && onlyMine && airings.none { it.entry.listStatus != null && it.at >= now } -> {
                Text(stringResource(R.string.calendar_empty_mine_ahead), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                Spacer(Modifier.height(12.dp))
                TonalButton(onClick = { store.setCalendarMine(false) }, modifier = Modifier.remembered(memory, "show-all")) { ButtonLabel(stringResource(R.string.calendar_show_all_airing)) }
            }
            shown.isEmpty() && !loading -> {
                Text(if (offset == 0) stringResource(R.string.calendar_empty_today) else stringResource(R.string.calendar_empty_day, day.dayOfWeek.getDisplayName(TextStyle.FULL, AirTime.locale)), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
        val density = LocalDensity.current
        val spec = remember(density) { with(density) { SafeSpec(Canvas.FocusRoom.toPx(), 40.dp.toPx()) } }
        ScrollRule(spec) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = 60.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                shown.forEachIndexed { index, airing ->
                    if (nowAt == index) item(key = "now", contentType = "now") { NowRule(now) }
                    item(key = airing.key, contentType = "airing") {
                        AiringRow(
                            airing,
                            past = airing.at < now,
                            onClick = { onOpen(airing.entry.id) },
                            sheet = { planSheet(context, store, airing.entry, onOpen) },
                            onDay = { step ->
                                val next = offset + step
                                if (next !in -DAYS_BACK..ceilingDays) return@AiringRow false
                                offset = next
                                aim = airing.at.atZone(zone).toLocalTime()
                                true
                            },
                            modifier = swapFade().firstFocus(memory, airing.key, index == lead),
                        )
                    }
                }
                if (nowAt == shown.size) item(key = "now", contentType = "now") { NowRule(now) }
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

/**
 * The line that marks the time now, between the last broadcast that has aired and the next: silk,
 * 1 dp, with the time leading. It takes no focus.
 */
@Composable
private fun NowRule(now: Instant) {
    val context = LocalContext.current
    val time = airTime(AirTime.clock(now), false)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp)
            .semantics(mergeDescendants = true) { contentDescription = Strings.calendarNowLabel(context, time) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(time, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk, modifier = Modifier.width(84.dp).clearAndSetSemantics {})
        Box(Modifier.weight(1f).height(1.dp).background(ObiSilk.Silk))
    }
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
    val context = LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val state = stringResource(if (past) R.string.media_status_aired else R.string.media_status_airing)
    val progress = airing.listProgress(context)
    val timeLine = airing.timeLine(context)
    val episodeLine = airing.episodeLine(context)
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
                contentDescription = listOfNotNull(timeLine, airing.entry.title, episodeLine, progress, state).joinToString(", ")
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
            // Lines under the title are haze, or the row's own colour once focus turns it silk;
            // never a faded colour, which falls under 4.5:1 on a past row.
            val quiet = if (focused) LocalContentColor.current else ObiSilk.Haze
            Text(airing.timeText(context, quiet), style = MaterialTheme.typography.labelLarge, maxLines = 2, modifier = Modifier.width(84.dp).clearAndSetSemantics {})
            ArtImage(
                airing.entry.bannerUrl ?: airing.entry.coverUrl,
                CoverPlate.parse(airing.entry.coverColor),
                pixels(96.dp),
                Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)),
            )
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f).clearAndSetSemantics {}) {
                Text(airing.entry.title, style = TvTitle.list, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(episodeLine, style = MaterialTheme.typography.bodyMedium, color = quiet)
            }
            if (progress != null) {
                Text(
                    progress,
                    style = MaterialTheme.typography.labelMedium,
                    color = quiet,
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
