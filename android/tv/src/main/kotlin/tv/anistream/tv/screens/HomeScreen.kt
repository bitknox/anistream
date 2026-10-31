// Home: the spotlight first, with its main action focused, then the engine's sections as rows in
// the order it sends them. The page scrolls by row: once focus leaves the spotlight, the focused
// row's header sits at y 48.

package tv.anistream.tv.screens

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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import java.time.LocalDate
import java.time.format.TextStyle
import java.util.Locale
import kotlinx.coroutines.launch
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.HomeAction
import tv.anistream.engine.model.HomeItem
import tv.anistream.engine.model.HomeEnd
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.Playable
import tv.anistream.engine.model.SectionKind
import tv.anistream.engine.model.Spotlight
import tv.anistream.engine.model.UpNextReason
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.AirTime
import tv.anistream.store.AppStore
import tv.anistream.store.text
import tv.anistream.store.without
import tv.anistream.tv.player.LocalAddStore
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.BrowseRow
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.CardPlaceholder
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.HeldScroll
import tv.anistream.tv.ui.HeroHeader
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.OfflineLine
import tv.anistream.tv.ui.PosterCard
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.StateBlock
import tv.anistream.tv.ui.TextBar
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.WideCard
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.isOffline
import tv.anistream.tv.ui.placeholdersAfterWait
import tv.anistream.tv.ui.remembered
import tv.anistream.tv.ui.settlesAt
import tv.anistream.tv.ui.swapFade

/** The y a focused row's header scrolls to. */
private val RowTop = 48.dp

@Composable
fun HomeScreen(
    nav: TvNavigator,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    LaunchedEffect(Unit) { store.loadHome() }
    val view by store.home.view.collectAsStateWithLifecycle()
    val checks by store.home.playable.collectAsStateWithLifecycle()
    val offline = isOffline()
    // Titles taken off by Not interested or Remove from Continue, gone until the next send.
    val gone = remember { mutableStateListOf<AnilistId>() }
    val home = view
    val spot = home?.spotlight?.takeUnless { it.entry.id in gone }
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    LaunchedEffect(spot?.entry?.id) {
        if (spot != null) {
            store.loadEpisodes(spot.entry.id)
            if (spot.playable == Playable.UNKNOWN && spot.action != HomeAction.NOTHING) store.probe(spot.entry.id)
        }
    }

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val rowTop = with(LocalDensity.current) { RowTop.roundToPx() }
    ToTop(nav, memory, scrolled = { list.firstVisibleItemIndex > 0 || list.firstVisibleItemScrollOffset > 0 }) { list.scrollToItem(0) }
    val placeholders = placeholdersAfterWait(home == null || home.pending.isNotEmpty())
    val sections = home?.sections.orEmpty().map { it to it.without(gone.toSet()) }.filter { it.second.isNotEmpty() }

    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            if (spot != null) {
                item(key = SPOTLIGHT, contentType = "hero") {
                    SpotlightHero(spot, (checks[spot.entry.id] ?: spot.playable) == Playable.NOT_FOUND, tables[spot.entry.id], memory, onOpen, onPlay, store, gone,
                        swapFade().onFocusChanged { if (it.hasFocus) scope.launch { list.animateScrollToItem(0) } })
                }
            } else if (placeholders) {
                item(key = "placeholder", contentType = "hero") { HeroPlaceholder(swapFade()) }
            }
            if (home?.offline == true) {
                item(key = "offline", contentType = "line") {
                    val since = home.offlineSince
                    if (since != null) {
                        Text(
                            Strings.homeOffline(context, AirTime.clock(java.time.Instant.ofEpochSecond(since))),
                            style = MaterialTheme.typography.bodyMedium,
                            color = ObiSilk.Haze,
                            modifier = swapFade().padding(start = Canvas.Keyline, top = if (spot == null) Canvas.SafeY else 0.dp),
                        )
                    } else {
                        OfflineLine(swapFade().padding(start = Canvas.Keyline, top = if (spot == null) Canvas.SafeY else 0.dp))
                    }
                }
            }
            var firstRow = spot == null
            for ((section, items) in sections) {
                val first = firstRow
                firstRow = false
                val key = "${section.kind.name}-${section.title}"
                item(key = key, contentType = "row") {
                    BrowseRow(section.title.text(context), modifier = swapFade().settlesAt(list, scope, key, rowTop)) {
                        items(items, key = { "$key-${it.entry.id}" }, contentType = { "card" }) { item ->
                            HomeCard(section.kind, item, memory, "$key/${item.entry.id}", first && item === items.first(), onOpen, onPlay, store, gone)
                        }
                    }
                }
            }
            if (home != null && home.pending.isNotEmpty() && placeholders) {
                item(key = "row-placeholder", contentType = "row") { RowPlaceholder(swapFade()) }
            }
            if (home?.failed == true) {
                item(key = "failed", contentType = "state") {
                    StateBlock(
                        Strings.homeFailed(context),
                        swapFade().padding(start = Canvas.Keyline, end = Canvas.SafeX),
                        action = Strings.homeTryAgain(context),
                        onAction = { store.loadHome(refresh = true) },
                        actionModifier = Modifier.firstFocus(memory, "retry", spot == null && sections.isEmpty()),
                    )
                }
            }
            home?.end?.let { end ->
                item(key = "end", contentType = "line") {
                    Text(
                        when (end) {
                            HomeEnd.EVERYTHING -> Strings.homeEnd(context)
                            HomeEnd.SIGN_IN -> Strings.homeEndSignedOut(context)
                            HomeEnd.NO_SOURCES -> Strings.homeEndNoSources(context)
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = ObiSilk.Haze,
                        modifier = swapFade().padding(start = Canvas.Keyline, end = Canvas.SafeX),
                    )
                }
            }
            if (home?.offline == true && home.offlineSince == null && spot == null && sections.isEmpty()) {
                item(key = "empty", contentType = "empty") {
                    Box(swapFade().fillMaxWidth().padding(top = 120.dp), contentAlignment = Alignment.Center) {
                        StateBlock(
                            Strings.homeOfflineEmptyTitle(context),
                            nagare = true,
                            detail = Strings.homeOfflineEmptyBody(context),
                            action = Strings.homeTryAgain(context),
                            onAction = { store.loadHome(refresh = true) },
                            actionModifier = Modifier.firstFocus(memory, "retry-offline"),
                        )
                    }
                }
            }
        }
    }
    EnterFocus(memory, entry = spot != null || sections.isNotEmpty())
}

/** The text under a card's title: the engine's line, else why it is here, else when it airs. */
private fun HomeItem.cardLine(context: android.content.Context): String? {
    line?.let { return it.text(context) }
    val reason = when (reason) {
        UpNextReason.NEW_SEASON -> Strings.homeReasonNewSeason(context)
        UpNextReason.PLANNING -> Strings.homeReasonPlanning(context)
        UpNextReason.RECOMMENDED -> Strings.homeReasonRecommended(context)
        UpNextReason.AIRING_TODAY -> Strings.upNextReasonAiringToday(context)
        null -> null
    }
    val airs = airsAt?.let { at ->
        val date = LocalDate.now().plusDays((day ?: 0u).toLong())
        val name = if (day == null || day == 0u) Strings.homeToday(context) else date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault())
        name + " " + AirTime.clock(java.time.Instant.ofEpochSecond(at)) // i18n-skip
    }
    return listOfNotNull(reason, airs).joinToString(" · ").ifEmpty { null } // i18n-skip
}

@Composable
private fun HomeCard(
    kind: SectionKind,
    item: HomeItem,
    memory: FocusMemory,
    memoryKey: String,
    first: Boolean,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
    store: AppStore,
    gone: MutableList<AnilistId>,
) {
    val context = LocalContext.current
    val entry = item.entry
    val line = item.cardLine(context)
    val modifier = Modifier
        .firstFocus(memory, memoryKey, first)
        .onFocusChanged { if (it.isFocused && item.playable == Playable.UNKNOWN && kind != SectionKind.CONTINUE) store.probe(entry.id) }
    val sheet: () -> ActionSheet = {
        val actions = mutableListOf<CardAction>()
        if (kind == SectionKind.CONTINUE) {
            item.episode?.let { episode ->
                actions += CardAction(Strings.homeMarkWatched(context, episode.toInt())) { store.setWatched(entry.id, listOf(episode.toString())) }
            }
            actions += CardAction(Strings.homeRemoveContinue(context)) { gone += entry.id; store.hideFromContinue(entry.id) }
        } else {
            actions += CardAction(Strings.tvAddToPlanning(context)) { store.setStatus(entry.id, LibrarySegment.PLANNING) }
            actions += CardAction(Strings.upNextNotInterested(context)) { gone += entry.id; store.dismissTitle(entry.id) }
        }
        actions += CardAction(Strings.tvOpenTitlePage(context)) { onOpen(entry.id) }
        ActionSheet(entry.title, actions)
    }
    if (kind == SectionKind.CONTINUE || kind == SectionKind.THIS_WEEK) {
        WideCard(
            entry,
            onClick = { onOpen(entry.id) },
            line = line,
            progress = item.progress?.toFloat(),
            newDot = item.new,
            sheet = sheet,
            modifier = modifier,
        )
    } else {
        PosterCard(entry, onClick = { onOpen(entry.id) }, line = line, sheet = sheet, modifier = modifier)
    }
}

@Composable
private fun SpotlightHero(
    spot: Spotlight,
    notFound: Boolean,
    rows: List<tv.anistream.engine.model.EpisodeRow>?,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
    store: AppStore,
    gone: MutableList<AnilistId>,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val entry = spot.entry
    val action = if (notFound && spot.action != HomeAction.NOTHING) HomeAction.ADD_TO_PLANNING else spot.action
    val line = if (notFound) Strings.homeNotFound(context) else spot.line?.text(context)
    HeroHeader(entry, rows, modifier) {
        Spacer(Modifier.height(4.dp))
        Text(spot.label.text(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1)
        if (line != null) Text(line, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1)
        Spacer(Modifier.height(16.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            // No source can play yet: the one way on is a plugin store, as on the title page.
            val addStore = LocalAddStore.current
            when {
                action == HomeAction.NOTHING -> Text(Strings.homeSpotlightPlanning(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.firstFocus(memory, SPOTLIGHT))
                action == HomeAction.ADD_TO_PLANNING -> AmberButton(
                    onClick = { store.setStatus(entry.id, LibrarySegment.PLANNING) },
                    icon = null,
                    modifier = Modifier.firstFocus(memory, SPOTLIGHT),
                ) { ButtonLabel(Strings.upNextAddPlanning(context)) }
                addStore != null -> AmberButton(onClick = addStore, icon = null, modifier = Modifier.firstFocus(memory, SPOTLIGHT)) {
                    ButtonLabel(stringResource(R.string.tv_add_store_to_play))
                }
                else -> AmberButton(
                    onClick = { onPlay(entry.id, spot.episode) },
                    modifier = Modifier.firstFocus(memory, SPOTLIGHT),
                ) {
                    ButtonLabel(
                        if (action == HomeAction.RESUME) Strings.homeResumeEpisode(context, spot.episode.toInt())
                        else Strings.homePlayEpisode(context, spot.episode.toInt()),
                    )
                }
            }
            Spacer(Modifier.width(16.dp))
            TonalButton(
                onClick = { onOpen(entry.id) },
                icon = TvIcons.Info,
                modifier = Modifier.remembered(memory, "hero/title"),
            ) { ButtonLabel(stringResource(R.string.tv_title_page)) }
        }
    }
}

@Composable
private fun HeroPlaceholder(modifier: Modifier) {
    Column(modifier.fillMaxWidth()) {
        Spacer(Modifier.height(212.dp))
        Column(Modifier.padding(start = Canvas.Keyline)) {
            TextBar(320.dp, height = 32.dp)
            Spacer(Modifier.height(12.dp))
            TextBar(260.dp)
        }
    }
}

@Composable
private fun RowPlaceholder(modifier: Modifier) {
    Column(modifier.padding(start = Canvas.Keyline)) {
        TextBar(140.dp, height = 16.dp)
        Spacer(Modifier.height(Canvas.HeaderGap))
        Row(horizontalArrangement = Arrangement.spacedBy(Canvas.CardGap)) {
            repeat(5) { CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight) }
        }
    }
}

private const val SPOTLIGHT = "hero"
