// Home's shelves: Continue as wide cards, the discovery shelves as posters, and This week by day.
// Each draws the engine's HomeSection as it is; a long press opens the card's menu.

package tv.anistream.ui

import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.time.format.TextStyle
import java.util.Locale
import tv.anistream.design.ArtShape
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.model.HomeAction
import tv.anistream.engine.model.HomeItem
import tv.anistream.engine.model.HomeLink
import tv.anistream.engine.model.HomeSection
import tv.anistream.engine.model.SectionKind
import tv.anistream.engine.model.UpNextReason
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.AirTime
import tv.anistream.store.text

/** Poster width: 110 on phones, 128 on wider windows. */
@Composable
internal fun posterWidth(): Dp = if (wideWindow()) 128.dp else 110.dp

/** Continue card width: 240 on phones, 272 on wider windows. */
@Composable
internal fun cardWidth(): Dp = if (wideWindow()) 272.dp else 240.dp

/** A shelf's heading, 48 high, with the count and the link that opens the whole of it. */
@Composable
private fun ShelfHeading(section: HomeSection) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val title = section.title.text(context)
    val count = if (section.kind == SectionKind.CONTINUE && section.total > 12u) Strings.homeContinueCount(context, section.total.toInt()) else null
    val link = section.viewAll
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.weight(1f), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false).semantics { heading() })
            if (count != null) Text(count, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        if (link != null) {
            val label = when (link) {
                HomeLink.Watching -> Strings.homeViewAll(context)
                HomeLink.SeasonalView -> Strings.homeSeasonal(context)
                HomeLink.CalendarView -> Strings.homeFullCalendar(context)
                is HomeLink.Browse -> Strings.homeSearch(context)
            }
            TextButton(
                onClick = {
                    when (link) {
                        HomeLink.Watching -> navigator.go(Destination.Library)
                        HomeLink.SeasonalView -> navigator.go(Destination.Seasonal)
                        HomeLink.CalendarView -> navigator.go(Destination.Calendar)
                        is HomeLink.Browse -> {
                            store.search(link.query)
                            navigator.go(Destination.Search)
                        }
                    }
                },
                modifier = Modifier.semantics { contentDescription = "$label, $title" },
            ) { Text(label) }
        }
    }
}

/** Continue: wide cards with their meters, up to twelve. */
@Composable
internal fun ContinueShelf(section: HomeSection, items: List<HomeItem>, h: HomeHandlers) {
    Column(Modifier.padding(top = 8.dp)) {
        ShelfHeading(section)
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(items, key = { "c-${it.entry.id}" }, contentType = { "card" }) { Box(itemMotion()) { ContinueCard(it, cardWidth(), h) } }
        }
    }
}

/** The short line under a poster: the engine's, else the reason, else the first genre. */
private fun HomeItem.detail(context: Context): String? {
    line?.let { return it.text(context) }
    val reason = when (reason) {
        UpNextReason.NEW_SEASON -> Strings.homeReasonNewSeason(context)
        UpNextReason.PLANNING -> Strings.homeReasonPlanning(context)
        UpNextReason.RECOMMENDED -> Strings.homeReasonRecommended(context)
        UpNextReason.AIRING_TODAY -> Strings.upNextReasonAiringToday(context)
        null -> null
    }
    val starts = starts?.words()?.let { Strings.upNextStarts(context, it) }
    return listOfNotNull(reason, starts).joinToString(" · ").ifEmpty { null } ?: entry.genreLine
}

/** A progress meter in silk, a 4 dp gap, then the rule colour for what is left. */
@Composable
internal fun SilkMeter(fraction: Double, modifier: Modifier = Modifier) {
    LinearProgressIndicator(
        progress = { fraction.toFloat().coerceIn(0f, 1f) },
        modifier = modifier.fillMaxWidth().height(4.dp),
        color = MaterialTheme.colorScheme.onSurface,
        trackColor = MaterialTheme.colorScheme.outline,
        gapSize = 4.dp,
        drawStopIndicator = {},
    )
}

/** A water dot, 6 dp, which only ever stands beside words. */
@Composable
internal fun WaterDot() {
    Box(Modifier.size(6.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ContinueCard(item: HomeItem, width: Dp, h: HomeHandlers) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val snackbars = LocalSnackbars.current
    val colors = MaterialTheme.colorScheme
    val entry = item.entry
    val id = entry.id
    val episode = item.episode
    var menu by remember { mutableStateOf(false) }
    val playable = item.action == HomeAction.RESUME || item.action == HomeAction.PLAY
    val play: (() -> Unit)? = if (playable && episode != null) ({ h.actions.play(id, episode.toString()) }) else null
    val playLabel = if (item.action == HomeAction.RESUME) Strings.homeResumeEpisode(context, episode?.toInt() ?: 0) else Strings.homePlayEpisode(context, episode?.toInt() ?: 0)
    val markLabel = Strings.homeMarkWatched(context, episode?.toInt() ?: 0)
    val removeLabel = Strings.homeRemoveContinue(context)
    val mark = {
        if (episode != null) {
            haptics.performHapticFeedback(HapticFeedbackType.ToggleOn)
            store.setWatched(id, listOf(episode.toString()))
            snackbars.show(Strings.homeMarkedWatched(context, episode.toInt()), context.getString(R.string.action_undo)) {
                store.setWatched(id, listOf(episode.toString()), watched = false)
            }
        }
    }
    val open = { navigator.openTitle(id, "home-continue") }
    Box {
        Column(
            Modifier
                .width(width)
                .shared(Shared.Page, id, "home-continue")
                .semantics {
                    customActions = listOfNotNull(
                        play?.let { CustomAccessibilityAction(playLabel) { it(); true } },
                        CustomAccessibilityAction(markLabel) { mark(); true },
                        CustomAccessibilityAction(removeLabel) { h.remove(id); true },
                    )
                }
                .combinedClickable(
                    onClickLabel = stringResource(R.string.title_open),
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); menu = true },
                    onClick = open,
                ),
        ) {
            BannerArt(entry, Modifier.size(width, width * 9 / 16).shared(Shared.Art, id, "home-continue").clip(ArtShape.banner))
            Box(Modifier.padding(top = 8.dp).height(4.dp)) { item.progress?.let { SilkMeter(it) } }
            Text(entry.showTitle, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (item.new) WaterDot()
                Text(
                    item.line?.text(context).orEmpty(),
                    style = MaterialTheme.typography.bodyMedium.tabular(),
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Menu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(playLabel) }, leadingIcon = { Icon(Icons.Play, null) }, enabled = play != null, onClick = { menu = false; play?.invoke() })
            DropdownMenuItem(text = { Text(markLabel) }, leadingIcon = { Icon(Icons.Check, null) }, onClick = { menu = false; mark() })
            DropdownMenuItem(text = { Text(Strings.homeDetails(context)) }, leadingIcon = { Icon(Icons.External, null) }, onClick = { menu = false; open() })
            HorizontalDivider(Modifier.padding(vertical = 8.dp), color = colors.outlineVariant)
            DropdownMenuItem(text = { Text(removeLabel) }, leadingIcon = { Icon(Icons.HideRow, null) }, onClick = { menu = false; h.remove(id) })
        }
    }
}

/** A discovery shelf: posters, each with its short reason, and Not interested on a long press. */
@Composable
internal fun PosterShelf(section: HomeSection, items: List<HomeItem>, h: HomeHandlers) {
    Column(Modifier.padding(top = 8.dp)) {
        ShelfHeading(section)
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(items, key = { "p-${section.kind.name}-${it.entry.id}" }, contentType = { "poster" }) {
                Box(itemMotion()) { HomePoster(it, posterWidth(), "home-${section.kind.name}", h) }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HomePoster(item: HomeItem, width: Dp, from: String, h: HomeHandlers) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val entry = item.entry
    val id = entry.id
    var menu by remember { mutableStateOf(false) }
    val detail = item.detail(context)
    val full = item.because?.let { Strings.upNextBecauseDetail(context, it) }
    val hide = { h.notInterested(entry, item.starts != null) }
    val hideLabel = Strings.upNextNotInterested(context)
    Box {
        Column(
            Modifier
                .width(width)
                .shared(Shared.Page, id, from)
                .semantics {
                    contentDescription = listOfNotNull(entry.showTitle, full ?: detail).joinToString(", ")
                    customActions = listOf(CustomAccessibilityAction(hideLabel) { hide(); true })
                }
                .combinedClickable(
                    onClickLabel = stringResource(R.string.title_open),
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); menu = true },
                    onClick = { navigator.openTitle(id, from) },
                ),
        ) {
            CoverArt(entry, width, Modifier.size(width, width * 1.5f).shared(Shared.Cover, id, from))
            Text(entry.showTitle, style = ShowTitle.caption, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
            detail?.let {
                Row(Modifier.padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (item.new) WaterDot()
                    Text(it, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        Menu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(Strings.homeDetails(context)) }, leadingIcon = { Icon(Icons.External, null) }, onClick = { menu = false; navigator.openTitle(id, from) })
            DropdownMenuItem(text = { Text(hideLabel) }, leadingIcon = { Icon(Icons.HideRow, null) }, onClick = { menu = false; hide() })
        }
    }
}

/**
 * This week by day from today. A phone lists the next three days; wider windows set the days side
 * by side, four or seven of them.
 */
@Composable
internal fun WeekShelf(section: HomeSection, items: List<HomeItem>) {
    val width = LocalConfiguration.current.screenWidthDp
    val days = when {
        width >= 900 -> 7
        width >= 700 -> 4
        else -> 3
    }
    val byDay = items.groupBy { (it.day ?: 0u).toInt() }
    Column(Modifier.padding(top = 8.dp)) {
        ShelfHeading(section)
        if (width >= 700) {
            Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (day in 0 until days) Box(Modifier.weight(1f)) { DayCell(day, byDay[day].orEmpty(), cap = 3) }
            }
        } else {
            Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (day in 0 until days) DayCell(day, byDay[day].orEmpty(), cap = 4)
            }
        }
    }
}

/** One day: its name, then what airs in order, then how many more there are. */
@Composable
private fun DayCell(day: Int, items: List<HomeItem>, cap: Int) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val date = remember(day) { LocalDate.now().plusDays(day.toLong()) }
    val name = if (day == 0) Strings.homeToday(context) else date.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
    Column(Modifier.fillMaxWidth()) {
        Text(name, style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant, modifier = Modifier.semantics { heading() })
        HorizontalDivider(Modifier.padding(vertical = 4.dp), color = colors.outlineVariant)
        if (items.isEmpty()) {
            Text(Strings.homeNothingAiring(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        for (item in items.take(cap)) {
            val time = item.airsAt?.let { AirTime.clock(java.time.Instant.ofEpochSecond(it)) }
            Column(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .shared(Shared.Page, item.entry.id, "home-week")
                    .clickable(onClickLabel = stringResource(R.string.title_open)) { navigator.openTitle(item.entry.id, "home-week") },
                verticalArrangement = Arrangement.Center,
            ) {
                Text(item.entry.showTitle, style = ShowTitle.caption, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOfNotNull(item.episode?.let { Strings.episodeNumber(context, it.toInt()) }, time).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall.tabular(),
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                )
            }
        }
        if (items.size > cap) {
            Text(Strings.homeDayMore(context, items.size - cap), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
    }
}
