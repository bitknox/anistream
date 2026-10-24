// Calendar: a week as day tabs, and the chosen day's broadcasts as 72-high rows in air order, so
// the text stays large. The next airing today is the first focus; past rows sit in haze above it.

package tv.anistream.tv.screens

import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import java.time.Instant
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import tv.anistream.design.CoverPlate
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.store.AirTime
import tv.anistream.store.AppClock
import tv.anistream.store.CalendarModel
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ArtImage
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

private val Clock = DateTimeFormatter.ofPattern("HH:mm")

private const val DAYS = 7

@Composable
fun CalendarScreen(nav: TvNavigator, memory: FocusMemory, onOpen: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    LaunchedEffect(Unit) { store.loadCalendar() }
    val window by store.calendar.window.collectAsStateWithLifecycle()
    val now = AppClock.now()
    val zone = AppClock.zone
    val today = now.atZone(zone).toLocalDate()
    var offset by rememberSaveable { mutableIntStateOf(0) }
    val day = today.plusDays(offset.toLong())
    val airings = remember(window) { window.airings(now) }
    val shown = airings.filter { it.at.atZone(zone).toLocalDate() == day }
    // The next airing today leads; on another day, its first.
    val lead = shown.indexOfFirst { it.at >= now }.takeIf { it >= 0 } ?: 0
    val list = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, lead - 1))
    ToTop(nav, memory, scrolled = { list.firstVisibleItemIndex > maxOf(0, lead - 1) }) { list.scrollToItem(maxOf(0, lead - 1)) }

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
        Spacer(Modifier.height(20.dp))
        if (shown.isEmpty()) {
            val name = if (offset == 0) "today" else "on ${day.dayOfWeek.getDisplayName(TextStyle.FULL, AirTime.locale)}"
            Text("Nothing airs $name.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        }
        val density = LocalDensity.current
        val spec = remember(density) { with(density) { SafeSpec(Canvas.FocusRoom.toPx(), 40.dp.toPx()) } }
        ScrollRule(spec) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = 60.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(shown, key = { "${it.entry.id}/${it.episode}/${it.at.epochSecond}" }, contentType = { "airing" }) { airing ->
                    val index = shown.indexOf(airing)
                    AiringRow(
                        airing,
                        past = airing.at < now,
                        onClick = { onOpen(airing.entry.id) },
                        onLongClick = { actions?.open(planSheet(store, airing.entry, onOpen)) },
                        modifier = Modifier.firstFocus(memory, "airing/${airing.entry.id}/${airing.episode}", index == lead),
                    )
                }
            }
        }
    }
    EnterFocus(memory, entry = shown.isNotEmpty())
}

@Composable
private fun AiringRow(airing: Airing, past: Boolean, onClick: () -> Unit, onLongClick: () -> Unit, modifier: Modifier) {
    Surface(
        onClick = onClick,
        onLongClick = onLongClick,
        modifier = modifier.fillMaxWidth().height(72.dp),
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = androidx.compose.ui.graphics.Color.Transparent,
            contentColor = if (past) ObiSilk.Haze else ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                airing.at.atZone(AppClock.zone).format(Clock),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.width(72.dp),
            )
            ArtImage(
                airing.entry.bannerUrl ?: airing.entry.coverUrl,
                CoverPlate.parse(airing.entry.coverColor),
                pixels(96.dp),
                Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)),
            )
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(airing.entry.title, style = TvTitle.list, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("Episode ${airing.episode}", style = MaterialTheme.typography.bodyMedium, color = LocalContentColor.current.copy(alpha = 0.72f))
            }
            Text(if (past) "Aired" else "Airing", style = MaterialTheme.typography.labelMedium, color = if (past) LocalContentColor.current else ObiSilk.Water)
        }
    }
}
