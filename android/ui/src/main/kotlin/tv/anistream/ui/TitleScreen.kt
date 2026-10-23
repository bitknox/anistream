// A title's page on a phone, as AndroidTitle draws it: the banner, the poster and the title, the
// one amber action, the list and the trailer, the synopsis, the seasons and what is related, and
// the episodes, those past the next one with their stills hidden.

package tv.anistream.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.structuralEqualityPolicy
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.store.AppClock
import tv.anistream.store.HeroArt
import tv.anistream.store.LibraryModel
import tv.anistream.store.label

/** The episodes past the next one hide their stills, unless the show says otherwise (A2). */
private const val SPOILER_PROTECTION = true

/** What a page shows before its title has arrived. */
private fun placeholderEntry(id: AnilistId) = Entry(
    id = id, title = "", secondary = null, format = null, episodes = null, year = null, score = null,
    studio = null, synopsis = "", coverUrl = null, coverColor = null, bannerUrl = null, genres = emptyList(),
    availableOn = emptyList(), progress = null, airingIn = null, airingEpisode = null, nextEpisode = null,
    nextAiringIn = null, lastAired = null, resume = null, related = emptyList(), recommended = emptyList(),
    trailerUrl = null, listStatus = null, titleRomaji = null, titleEnglish = null, titleNative = null,
    season = null, startDate = null, endDate = null, source = null, adult = false, detail = null,
    status = null, synopsisSpans = emptyList(),
)

/**
 * What a page shows of a season whose title has not arrived: the card its family carries, so its
 * name and cover are there at once, and nothing that would have to be taken back.
 */
private fun cardEntry(id: AnilistId, card: LinkedTitle?): Entry {
    val blank = placeholderEntry(id)
    card ?: return blank
    return blank.copy(
        title = card.title, titleRomaji = card.titleRomaji, titleEnglish = card.titleEnglish,
        titleNative = card.titleNative, format = card.format, coverUrl = card.coverUrl,
        coverColor = card.coverColor, season = card.season, year = card.year, listStatus = card.listStatus,
    )
}

/**
 * The franchise's seasons as the last of its pages that had them said. Every season has the same
 * ones, so a season whose own page has not come yet keeps the picker, numbered as it was, and its
 * card.
 */
private class KnownSeasons {
    private var cards: List<LinkedTitle>? = null

    fun familyOf(entry: Entry): TitleFamily {
        entry.detail?.seasons?.takeIf { it.isNotEmpty() }?.let { cards = it }
        return TitleFamily.of(entry, cards)
    }
}

/**
 * The page a season change fades into when the season picked has no page laid out yet, as after a
 * jump along a long picker: the season's name and cover from its card, with T8's placeholders in
 * place of the rest. It is composed and laid out beneath ahead of time like the seasons' own pages,
 * so the tap changes only what is drawn; the season's own page is composed beneath it once the
 * change has settled, and fades in over it when laid out.
 */
private val STAND_IN: AnilistId = 0u

/** The season the stand-in shows, and whether it crossfades to it: only while it is on screen. */
@androidx.compose.runtime.Immutable
private data class StandIn(val season: AnilistId, val crossfade: Boolean)

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun TitleScreen(id: AnilistId, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    // Every title and every title's episodes, read through the few this page shows: another
    // title's update elsewhere in the app recomposes nothing here.
    val titlesState = store.titles.entries.collectAsStateWithLifecycle()
    val episodesState = store.titles.episodes.collectAsStateWithLifecycle()
    // The season on screen. A pick in the season picker changes it in place, as the Mac's page
    // does: the page stays where it is and its content hands over in one crossfade (T7), rather
    // than a page of its own pushed at the top. The page's bounds keep the title it was opened as,
    // so it still closes into the cover or banner it grew from.
    var picked by rememberSaveable(id) { mutableLongStateOf(id.toLong()) }
    val shown: AnilistId = picked.toUInt()
    val known = remember(id) { KnownSeasons() }
    val own = rememberTitle(titlesState, shown)
    val family = known.familyOf(own ?: placeholderEntry(shown))
    val entry = own ?: cardEntry(shown, family.card(shown))
    // Each title's page asked for once while this screen is up, whoever asks: the page itself, or
    // the seasons either side of it ahead of a pick.
    val asked = remember(id) { mutableSetOf<AnilistId>() }
    val ask: (AnilistId) -> Unit = remember(id) {
        { season ->
            if (season !in store.titles.detailed && (asked.add(season) || season in store.titles.failed.value)) store.loadDetail(season)
        }
    }
    LaunchedEffect(shown) {
        ask(shown)
        store.loadEpisodes(shown)
    }
    // The season the pickers mark: the one picked last, ahead of the page while it changes.
    val marked = remember(id) { mutableLongStateOf(picked) }
    val inset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val wide = wideWindow()
    PrefetchSeasons(family, marked, wide, ask)
    // A page a fold opened goes back to the list and detail once the window is wide again (AT3).
    LaunchedEffect(wide) {
        if (wide && navigator.openedByFold == id) {
            navigator.openedByFold = null
            navigator.back()
        }
    }
    // Left any other way, by Back or for another page, the fold's page is the viewer's own.
    DisposableEffect(id) { onDispose { if (navigator.openedByFold == id) navigator.openedByFold = null } }
    if (wide) {
        val familyIds = family.seasons.map { it.id } + shown
        val episodes by remember(familyIds) {
            derivedStateOf(structuralEqualityPolicy()) { episodesState.value.filterKeys { it in familyIds } }
        }
        TabletTitle(id, entry, family, episodes, actions) {
            picked = it.toLong()
            marked.longValue = it.toLong()
        }
        return
    }

    // Each season near the one on screen has a page of its own: the one on screen on top, the
    // seasons either side of it, and the stand-in for any other. Each page beneath is composed
    // ahead in the frames' idle time, a row at a time, by its list's prefetch, and once the page
    // comes to rest it is laid out beneath, unseen, its picker where the one on screen has its
    // own. A pick then changes only what is drawn: the picked page fades in over the other on
    // standard while the pickers morph, and once it has, the season becomes the page's own: its
    // top bar, its sheets and what flies into a player or back to where the page was opened from.
    // As the season on screen moves along the picker, the pages follow it, one a frame once it
    // is at rest; a long franchise never has more than these few.
    var aboutOpen by rememberSaveable(id) { mutableStateOf(false) }
    // The page drawn on top, fading in, and the one beneath it at full strength: the season on
    // screen's own, or the stand-in for it. The same at rest.
    val top = remember(id) { mutableLongStateOf(picked) }
    val base = remember(id) { mutableLongStateOf(picked) }
    val standIn = remember(id) { mutableStateOf(StandIn(shown, crossfade = false)) }
    val fade = remember(id) { androidx.compose.runtime.mutableFloatStateOf(1f) }
    val shift = remember(id) { androidx.compose.runtime.mutableFloatStateOf(0f) }
    val lists = remember(id) { androidx.compose.runtime.mutableStateMapOf<AnilistId, LazyListState>() }
    // Each season laid out beneath, by where its picker stands.
    val ready = remember(id) { androidx.compose.runtime.mutableStateMapOf<AnilistId, Int>() }
    // The pages composed long enough ago for their lists' prefetch to have composed their rows in
    // the frames' idle time, so laying one out beneath composes nothing more.
    val grown = remember(id) { androidx.compose.runtime.mutableStateSetOf<AnilistId>() }
    // The pages composed, the one on screen first: see the effect below.
    val composed = remember(id) { androidx.compose.runtime.mutableStateListOf(shown) }
    // The season the pages beneath are composed around: the one on screen, once its change is done.
    val around = remember(id) { mutableLongStateOf(picked) }
    val scope = rememberCoroutineScope()
    var fading by remember(id) { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    // The season a finger is on in the picker, until it lifts and, for a pick, the fade is done.
    val pressed = remember(id) { mutableStateOf<AnilistId?>(null) }
    // The stand-in's list, made ahead of its page, so a pick before the page was composed can
    // line it up all the same.
    val standInList = remember(id) { LazyListState(prefetchStrategy = AheadPrefetch()) }
    val families = rememberUpdatedState(family)
    val geometry = rememberSeasonGeometry(family.seasons.size, LocalConfiguration.current.screenWidthDp.dp - 32.dp)
    val viewport = with(LocalDensity.current) { LocalConfiguration.current.screenWidthDp.dp.roundToPx() }
    // One scroll for every page's picker, so the picker fading in stands where the one it
    // replaces does; the season on screen in the middle as the page opens.
    val pickerScroll = rememberSaveable(id, saver = androidx.compose.foundation.ScrollState.Saver) {
        androidx.compose.foundation.ScrollState(geometry.centred(family.seasons.indexOfFirst { it.id == shown }, viewport))
    }
    FollowChosen(pickerScroll, geometry, family, marked, viewport)
    // What a page shows of [season], its title or its card: the stand-in's, the season it stands for.
    val pageEntry: (AnilistId) -> Entry = { season ->
        val of = if (season == STAND_IN) standIn.value.season else season
        titlesState.value[of]?.takeIf { season != STAND_IN } ?: cardEntry(of, families.value.card(of))
    }
    // Line [season]'s page up with the one on screen, its picker [at] this one's place.
    val lineUp: (AnilistId, Int) -> Unit = lineUp@{ season, at ->
        val list = lists[season] ?: return@lineUp
        list.requestScrollToItem(seasonsIndex(pageEntry(season)), -at)
        ready[season] = at
    }
    val standInShown = { top.longValue == STAND_IN.toLong() || base.longValue == STAND_IN.toLong() }
    // The pages beneath follow the one on screen each time it comes to rest with its picker in
    // view, after the page's own opening has settled.
    LaunchedEffect(id) {
        kotlinx.coroutines.delay(LINE_UP_AFTER)
        snapshotFlow {
            val onTop = top.longValue.toUInt()
            val at = lists[onTop]?.takeIf { !it.isScrollInProgress && fade.floatValue >= 1f }
                ?.layoutInfo?.visibleItemsInfo?.firstOrNull { it.key == SEASONS }?.offset
            // Where each page beneath has its picker now: a title's details arriving move it.
            at?.let { it to lists.filterKeys { season -> season != onTop && season in grown }.mapValues { (season, list) -> pickerAt(list).takeIf { season in ready } } }
        }.collect { found ->
            val (at, beneath) = found ?: return@collect
            for ((season, now) in beneath) if (now != at) lineUp(season, at)
        }
    }
    // The pages composed follow the season on screen: it, and once the page's own opening has
    // settled, the stand-in and the seasons either side of it. One is added or let go a frame,
    // the season on screen's own first, and only while nothing moves, so none of it lands in a
    // frame that draws motion.
    var opened by remember(id) { mutableStateOf(false) }
    LaunchedEffect(id) {
        kotlinx.coroutines.delay(LINE_UP_AFTER)
        opened = true
    }
    LaunchedEffect(id) {
        snapshotFlow {
            val seasons = families.value
            val near = around.longValue.toUInt()
            val want = buildList {
                add(near)
                if (opened && seasons.seasons.isNotEmpty()) add(STAND_IN)
                if (opened) addAll(seasons.neighbours(near))
            }
            val still = fade.floatValue >= 1f && lists[top.longValue.toUInt()]?.isScrollInProgress != true
            want.takeIf { still }
        }.collectLatest { want ->
            want ?: return@collectLatest
            for (season in want) {
                if (season !in composed) {
                    androidx.compose.runtime.withFrameNanos { }
                    composed.add(season)
                }
            }
            val kept = setOf(top.longValue.toUInt(), base.longValue.toUInt(), picked.toUInt())
            for (season in composed.toList()) {
                if (season !in want && season !in kept) {
                    androidx.compose.runtime.withFrameNanos { }
                    composed.remove(season)
                }
            }
        }
    }
    LaunchedEffect(picked) {
        kotlinx.coroutines.delay(AROUND_AFTER)
        around.longValue = picked
    }
    // The fade on standard that brings [page] in over the one beneath, then makes the season
    // marked the page's own.
    val fadeIn: (AnilistId, Int?) -> Unit = { page, at ->
        val from = top.longValue.toUInt()
        val there = lists[page]?.let(::pickerAt)
        // Near the top of the page the two pickers may stand apart, the content above them of
        // different heights: the pages then move together on the fade's spring, from the old
        // picker's place to the new one's.
        shift.floatValue = if (at != null && there != null) (at - there).toFloat() else 0f
        fade.floatValue = 0f
        top.longValue = page.toLong()
        fading = scope.launch {
            androidx.compose.animation.core.animate(0f, 1f, animationSpec = Motion.standard()) { value, _ -> fade.floatValue = value }
            aboutOpen = false
            // The page left stays laid out beneath, lined up where it was.
            if (at != null) ready[from] = at
            base.longValue = top.longValue
            picked = marked.longValue
            pressed.value = null
        }
    }
    val pick: (AnilistId) -> Unit = pick@{ next ->
        if (next.toLong() == marked.longValue) return@pick
        val onTop = top.longValue.toUInt()
        val laidOut = ready[next] != null && lists[next] != null
        if (!laidOut && onTop == STAND_IN) {
            // The stand-in is on screen, or fading in: it stands for this season instead, its
            // head crossfading, and the change goes on as it was.
            standIn.value = StandIn(next, crossfade = true)
            marked.longValue = next.toLong()
            if (fading?.isActive != true) {
                fading = scope.launch {
                    kotlinx.coroutines.delay(STANDARD_MILLIS)
                    picked = marked.longValue
                    pressed.value = null
                }
            }
            return@pick
        }
        fading?.cancel()
        marked.longValue = next.toLong()
        val page = if (laidOut) next else STAND_IN
        if (page == STAND_IN) {
            standIn.value = StandIn(next, crossfade = standInShown())
            // Picked before the stand-in was composed, straight after the page opened: composed
            // now, its list lined up ahead of its first frame.
            if (STAND_IN !in composed) {
                composed.add(STAND_IN)
                lists[STAND_IN] = standInList
            }
        }
        if (page.toLong() == base.longValue) {
            // Back to the page under the fade: it is there already.
            fade.floatValue = 1f
            top.longValue = base.longValue
            picked = next.toLong()
            pressed.value = null
            return@pick
        }
        val at = lists[onTop]?.let(::pickerAt)
        // Not laid out yet, as when picked straight after the page opened: lined up now.
        if (ready[page] == null && at != null) lineUp(page, at)
        fadeIn(page, at)
    }
    // The stand-in on screen hands over to the season's own page once that is laid out beneath,
    // in one more crossfade: T8's placeholders to the rows, or to its own placeholders, which
    // look the same.
    LaunchedEffect(id) {
        snapshotFlow {
            val season = picked.toUInt()
            val standing = lists[STAND_IN]
            val onScreen = base.longValue == STAND_IN.toLong() && top.longValue == base.longValue && fade.floatValue >= 1f
            val list = lists[season]
            if (!onScreen || standing == null || list == null || standing.isScrollInProgress || ready[season] == null) return@snapshotFlow null
            // Laid out beneath with its picker in view, wherever the content above it let it stand.
            season.takeIf { pickerAt(standing) != null && pickerAt(list) != null }
        }.collect { season ->
            if (season != null && season.toLong() == marked.longValue) fadeIn(season, lists[STAND_IN]?.let(::pickerAt))
        }
    }
    val press: (AnilistId) -> Unit = { season ->
        if (season.toLong() != marked.longValue) {
            pressed.value = season
            // A season with no page laid out: the stand-in takes its card now, unseen, ready for
            // the tap, and its title is asked for.
            if ((ready[season] == null || lists[season] == null) && !standInShown()) standIn.value = StandIn(season, crossfade = false)
            if (store.titles[season] == null) ask(season)
        }
    }
    // The finger lifted: unless that was a pick, the season it was on is let go.
    val lift: () -> Unit = {
        scope.launch {
            androidx.compose.runtime.withFrameNanos { }
            if (pressed.value != null && pressed.value?.toLong() != marked.longValue) pressed.value = null
        }
    }
    // What every page reads of the pick, through state: a pick recomposes the pickers alone and
    // leaves the pages and their lists as they are.
    val onScreenFamily = rememberUpdatedState(family)
    val picks = rememberUpdatedState(pick)
    val onPick = remember(id) { { next: AnilistId -> picks.value(next) } }
    val presses = rememberUpdatedState(press)
    val lifts = rememberUpdatedState(lift)
    val onPress = remember(id) { { season: AnilistId -> presses.value(season) } }
    val onLift = remember(id) { { lifts.value() } }
    val onAbout = remember(id) { { aboutOpen = true } }
    val onScreenPage = base.longValue.toUInt()
    Box(Modifier.fillMaxSize().shared(Shared.Page, id).background(MaterialTheme.colorScheme.surface)) {
        for (season in composed) {
            key(season) {
                val onScreen = season == onScreenPage
                val list = if (season == STAND_IN) standInList else rememberLazyListState(prefetchStrategy = remember { AheadPrefetch() })
                DisposableEffect(list) {
                    lists[season] = list
                    onDispose {
                        lists.remove(season)
                        ready.remove(season)
                        grown.remove(season)
                    }
                }
                LaunchedEffect(list) {
                    kotlinx.coroutines.delay(COMPOSE_AHEAD)
                    grown += season
                }
                PageSlot(
                    season = season,
                    onScreen = onScreen,
                    list = list,
                    standIn = standIn,
                    family = onScreenFamily,
                    titles = titlesState,
                    episodes = episodesState,
                    top = top,
                    base = base,
                    fade = fade,
                    shift = shift,
                    ready = ready,
                    pressed = pressed,
                    marked = marked,
                    geometry = geometry,
                    pickerScroll = pickerScroll,
                    inset = inset,
                    actions = actions,
                    onPick = onPick,
                    onPress = onPress,
                    onLift = onLift,
                    onAbout = onAbout,
                )
            }
        }
        TitleTopBar(entry, onBack = navigator::back, modifier = Modifier.padding(top = inset).zIndex(3f))
    }
    if (aboutOpen) {
        MoreAboutSheet(entry, onDismiss = { aboutOpen = false }) { other ->
            aboutOpen = false
            navigator.openTitle(other, "recommendation")
        }
    }
}

/** [id]'s title from [titles], read alone: another title changing leaves this one's readers be. */
@Composable
private fun rememberTitle(titles: androidx.compose.runtime.State<Map<AnilistId, Entry>>, id: AnilistId): Entry? {
    val title by remember(titles, id) { derivedStateOf { titles.value[id] } }
    return title
}

/**
 * [season]'s page in the title screen's stack, or the stand-in's: drawn on top, beneath, or laid
 * out unseen, as [top] and [base] say, read as it is placed and drawn, so a pick recomposes none
 * of the pages. Its own scope, so what one page reads recomposes that page alone.
 */
@Composable
private fun PageSlot(
    season: AnilistId,
    onScreen: Boolean,
    list: LazyListState,
    standIn: androidx.compose.runtime.State<StandIn>,
    family: androidx.compose.runtime.State<TitleFamily>,
    titles: androidx.compose.runtime.State<Map<AnilistId, Entry>>,
    episodes: androidx.compose.runtime.State<Map<AnilistId, List<EpisodeRow>>>,
    top: androidx.compose.runtime.LongState,
    base: androidx.compose.runtime.LongState,
    fade: androidx.compose.runtime.FloatState,
    shift: androidx.compose.runtime.FloatState,
    ready: Map<AnilistId, Int>,
    pressed: androidx.compose.runtime.State<AnilistId?>,
    marked: androidx.compose.runtime.LongState,
    geometry: SeasonGeometry,
    pickerScroll: androidx.compose.foundation.ScrollState,
    inset: Dp,
    actions: AppActions,
    onPick: (AnilistId) -> Unit,
    onPress: (AnilistId) -> Unit,
    onLift: () -> Unit,
    onAbout: () -> Unit,
) {
    // Composed beneath, not on screen: its placeholders are drawn from the start, so a change
    // into it shows them inside the crossfade.
    val ahead = remember { !onScreen }
    val title = if (season == STAND_IN) null else rememberTitle(titles, season)
    val shows = if (season == STAND_IN) standIn.value.season else season
    val card = family.value.card(shows)
    val page = title ?: remember(shows, card) { cardEntry(shows, card) }
    HeldRows(season, episodes, top, base, pressed) { rows ->
        SeasonPage(
            entry = page,
            episodes = if (season == STAND_IN) null else rows,
            loaded = title != null,
            ahead = ahead,
            crossfadeHead = if (season == STAND_IN) ({ standIn.value.crossfade }) else null,
            hasSeasons = family.value.seasons.isNotEmpty(),
            family = family,
            chosen = marked,
            titles = titles,
            geometry = geometry,
            pickerScroll = pickerScroll,
            // Only the page on screen flies into a page or a player.
            shared = onScreen && season != STAND_IN,
            list = list,
            inset = inset,
            actions = actions,
            onPick = onPick,
            onPress = onPress,
            onLift = onLift,
            onAbout = onAbout,
            modifier = Modifier
                .fillMaxWidth()
                .then(if (onScreen || season in ready) Modifier.fillMaxHeight() else Modifier.height(0.dp))
                // The picked page over the one it replaces over the rest, which are laid out but
                // not drawn.
                .layout { measurable, constraints ->
                    val placeable = measurable.measure(constraints)
                    layout(placeable.width, placeable.height) {
                        val z = when (season.toLong()) {
                            top.longValue -> 2f
                            base.longValue -> 1f
                            else -> 0f
                        }
                        placeable.place(0, 0, zIndex = z)
                    }
                }
                .graphicsLayer {
                    val moving = top.longValue != base.longValue
                    when (season.toLong()) {
                        top.longValue -> {
                            alpha = fade.floatValue
                            translationY = if (moving) shift.floatValue * (1f - fade.floatValue) else 0f
                        }
                        base.longValue -> {
                            alpha = 1f
                            translationY = if (moving) -shift.floatValue * fade.floatValue else 0f
                        }
                        else -> {
                            alpha = 0f
                            translationY = 0f
                        }
                    }
                }
                .then(if (onScreen) Modifier else Modifier.clearAndSetSemantics {}),
        )
    }
}

/** Where [list]'s season picker stands in its viewport, when it is in view. */
private fun pickerAt(list: LazyListState): Int? = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == SEASONS }?.offset

/** How long a title page waits after it opens before laying its other seasons out beneath. */
private const val LINE_UP_AFTER = 500L

/** How long a page composed beneath is given to compose its rows ahead before it is laid out. */
private const val COMPOSE_AHEAD = 300L

/** How long after a season change settles the pages beneath move on around it. */
private const val AROUND_AFTER = 120L

/** About how long [Motion.standard] takes to settle, for a change that has no fade of its own to wait on. */
private const val STANDARD_MILLIS = 300L

/**
 * [content] with [season]'s episodes as they are, except while a finger is on its season in the
 * picker or its page fades in over another: episodes arriving then wait for the pick to be done
 * and come in as T8 has them, rather than composing as the page changes. Its own scope, so a press
 * or a pick recomposes this and not the page under it.
 */
@Composable
private fun HeldRows(
    season: AnilistId,
    allEpisodes: androidx.compose.runtime.State<Map<AnilistId, List<EpisodeRow>>>,
    top: androidx.compose.runtime.LongState,
    base: androidx.compose.runtime.LongState,
    pressed: androidx.compose.runtime.State<AnilistId?>,
    content: @Composable (List<EpisodeRow>?) -> Unit,
) {
    val episodes by remember(season) { derivedStateOf { allEpisodes.value[season] } }
    var held by remember(season) { mutableStateOf(episodes) }
    val fadingIn = top.longValue == season.toLong() && base.longValue != season.toLong()
    if (!fadingIn && pressed.value != season) held = episodes
    content(held)
}

/** The season picker's key in a page's list. */
private const val SEASONS = "seasons"

/** Where the season picker stands in [entry]'s page: after the head, the actions, the synopsis if any, the facts and More about. */
private fun seasonsIndex(entry: Entry): Int = if (synopsisWithoutSpoilers(entry.synopsis).isNotEmpty()) 5 else 4

/**
 * A page's prefetch: the lazy list's own while it scrolls, and, while the page waits off screen,
 * every item from its head to a screenful of rows past the picker, composed and measured in the
 * frames' idle time, so the page is whole the moment a pick shows it.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
private class AheadPrefetch : androidx.compose.foundation.lazy.LazyListPrefetchStrategy {
    private val default = androidx.compose.foundation.lazy.LazyListPrefetchStrategy()
    private val ahead = mutableMapOf<Int, androidx.compose.foundation.lazy.layout.LazyLayoutPrefetchState.PrefetchHandle>()

    /** How many items the page had when they were composed ahead: rows arriving make them others. */
    private var items = -1

    override fun androidx.compose.foundation.lazy.LazyListPrefetchScope.onScroll(delta: Float, layoutInfo: androidx.compose.foundation.lazy.LazyListLayoutInfo) {
        with(default) { onScroll(delta, layoutInfo) }
    }

    override fun androidx.compose.foundation.lazy.LazyListPrefetchScope.onVisibleItemsUpdated(layoutInfo: androidx.compose.foundation.lazy.LazyListLayoutInfo) {
        with(default) { onVisibleItemsUpdated(layoutInfo) }
        if (layoutInfo.viewportSize.height > 0 || layoutInfo.totalItemsCount != items) {
            // On screen, what was composed ahead is in use or no longer needed; and once the
            // page's items change, as its episodes arrive, the ones at those places are others.
            ahead.values.forEach { it.cancel() }
            ahead.clear()
            items = layoutInfo.totalItemsCount
            if (layoutInfo.viewportSize.height > 0) return
        }
        for (index in 0 until minOf(layoutInfo.totalItemsCount, AHEAD_ITEMS)) {
            if (index !in ahead) ahead[index] = schedulePrefetch(index)
        }
    }

    override fun androidx.compose.foundation.lazy.layout.NestedPrefetchScope.onNestedPrefetch(firstVisibleItemIndex: Int) {
        with(default) { onNestedPrefetch(firstVisibleItemIndex) }
    }

    private companion object {
        /** The head, the actions, the synopsis, the facts, More about, the picker, Related and a screenful of rows. */
        const val AHEAD_ITEMS = 16
    }
}

/**
 * One season's page: its head, actions, synopsis and facts, the picker, what is related, and its
 * episodes. [chosen] is the season the pickers mark, which the picker of every page reads, so a
 * pick morphs the same picker on both pages as one fades over the other. A season whose title has
 * not arrived ([loaded] false) shows its card's name and cover, and T8's placeholders for the rest.
 */
@Composable
private fun SeasonPage(
    entry: Entry,
    episodes: List<EpisodeRow>?,
    loaded: Boolean,
    /** Composed beneath, off screen: its placeholders show from the start, with no wait. */
    ahead: Boolean,
    /** Whether a new season in the head crossfades in, as on the stand-in while it is on screen. */
    crossfadeHead: (() -> Boolean)?,
    hasSeasons: Boolean,
    family: androidx.compose.runtime.State<TitleFamily>,
    chosen: androidx.compose.runtime.LongState,
    titles: androidx.compose.runtime.State<Map<AnilistId, Entry>>,
    geometry: SeasonGeometry,
    pickerScroll: androidx.compose.foundation.ScrollState,
    shared: Boolean,
    list: LazyListState,
    inset: Dp,
    actions: AppActions,
    onPick: (AnilistId) -> Unit,
    onPress: (AnilistId) -> Unit,
    onLift: () -> Unit,
    onAbout: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val store = LocalAppStore.current
    val marks = remember(entry.id) { UserMarks() }
    val unfolded by store.episodeFolds.unfolded.collectAsStateWithLifecycle()
    val fold = rememberEpisodeFold(entry.id, list)
    androidx.compose.runtime.CompositionLocalProvider(LocalUserMarks provides marks) {
        // Every item moves as one when another comes or goes, as the title's details arrive: the
        // synopsis, the facts and Related fade in above the episodes as they make room, and the
        // placeholders of a season that had not arrived crossfade with what replaces them (T8).
        LazyColumn(state = list, modifier = modifier.background(MaterialTheme.colorScheme.surface), contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "head", contentType = "head") {
                if (crossfadeHead == null) {
                    TitleHead(entry, episodes, list, inset, shared)
                } else {
                    androidx.compose.animation.AnimatedContent(
                        entry,
                        transitionSpec = {
                            if (crossfadeHead()) fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard())
                            else androidx.compose.animation.EnterTransition.None togetherWith androidx.compose.animation.ExitTransition.None
                        },
                        contentAlignment = Alignment.TopStart,
                        label = "stand-in",
                        contentKey = { it.id },
                    ) { TitleHead(it, episodes, list, inset, shared) }
                }
            }
            if (loaded) {
                item(key = "actions", contentType = "actions") { Box(itemMotion()) { TitleActions(entry, actions) } }
                val synopsis = synopsisWithoutSpoilers(entry.synopsis)
                if (synopsis.isNotEmpty()) item(key = "synopsis", contentType = "synopsis") { Box(itemMotion()) { Synopsis(synopsis) } }
                item(key = "facts", contentType = "facts") { Box(itemMotion()) { FactsLines(entry) } }
                item(key = "more-about", contentType = "more-about") { Box(itemMotion()) { MoreAboutRow(onAbout) } }
            } else {
                // In the layout of what replaces them, so the picker stands where it will.
                item(key = "actions-placeholder", contentType = "actions-placeholder") { Box(itemMotion()) { ActionsPlaceholder() } }
                item(key = "facts-placeholder", contentType = "facts-placeholder") { Box(itemMotion()) { FactsPlaceholder() } }
                item(key = "more-about-placeholder", contentType = "more-about-placeholder") { Box(itemMotion()) { MoreAboutPlaceholder() } }
            }
            if (hasSeasons) item(key = SEASONS, contentType = SEASONS) {
                val picked = chosen.longValue.toUInt()
                val pickedEntry by remember(picked) { derivedStateOf { titles.value[picked] } }
                Seasons(picked, pickedEntry, family.value, geometry, pickerScroll, onPick, itemMotion(), onPress = onPress, onLift = onLift)
            }
            val related = entry.relatedCards()
            if (related.isNotEmpty()) item(key = "related", contentType = "related") { Box(itemMotion()) { RelatedRail(related) } }
            episodeRows(entry, episodes, actions, unfolded = entry.id in unfolded, onFold = fold, wait = !ahead)
        }
    }
}

/** The actions' places while a season's title is on its way: the main button's, then the list's and the trailer's. */
@Composable
private fun ActionsPlaceholder() {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 16.dp)) {
        Box(Modifier.fillMaxWidth().height(56.dp).clip(CircleShape).background(colors.surfaceContainer))
        Row(Modifier.padding(top = 4.dp).height(48.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f).height(40.dp).clip(CircleShape).background(colors.surfaceContainer))
            Box(Modifier.weight(1f).height(40.dp).border(1.dp, colors.outlineVariant, CircleShape))
        }
    }
}

/** The facts' two lines while a season's title is on its way. */
@Composable
private fun FactsPlaceholder() {
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 4.dp)) {
        TextPlaceholder(0.62f, 20.dp)
        TextPlaceholder(0.4f, 20.dp)
    }
}

/** More about this show's row while a season's title is on its way: its place, its words as a bar. */
@Composable
private fun MoreAboutPlaceholder() {
    Box(Modifier.padding(top = 4.dp).fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 16.dp), contentAlignment = Alignment.CenterStart) {
        TextPlaceholder(0.42f, 20.dp)
    }
}

/**
 * [content] for the season on screen, [entry]. A pick in the season picker hands the old season's
 * over to the new one's in one crossfade on standard, the height following on standard, as the
 * Mac's page changes in place (T7). Art the new season has in the cache is there at once inside the
 * fade; art still on its way fades in over its plate as it lands (T8).
 */
@Composable
private fun SeasonFade(entry: Entry, modifier: Modifier = Modifier, content: @Composable (Entry) -> Unit) {
    androidx.compose.animation.AnimatedContent(
        entry,
        modifier = modifier,
        transitionSpec = {
            (fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard()))
                .using(androidx.compose.animation.SizeTransform(clip = false) { _, _ -> Motion.standard() })
        },
        contentAlignment = Alignment.TopStart,
        label = "season",
        contentKey = { it.id },
    ) { content(it) }
}

/**
 * The titles and art of the seasons either side of the one [around] marks, asked for ahead of a
 * pick, so a pick in the season picker hands over to a season that is already here: its words at
 * once, its art from the cache. Only a title the app has not seen is asked of AniList, as the
 * Mac's picker does, and only these two: as the season on screen moves along the picker, the ones
 * next to it are asked for in turn, never the whole franchise at once.
 */
@Composable
private fun PrefetchSeasons(family: TitleFamily, around: androidx.compose.runtime.State<Long>, wide: Boolean, ask: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val titles = store.titles.entries.collectAsStateWithLifecycle()
    val near = family.neighbours(around.value.toUInt())
    LaunchedEffect(near) {
        // Past the frame of the pick that moved them.
        androidx.compose.runtime.withFrameNanos { }
        for (other in near) if (store.titles[other] == null) ask(other)
    }
    val banner = if (wide) 1600 else pixels(412.dp)
    val cover = if (wide) 500 else 300
    // Read through these seasons alone: another title changing asks for nothing again.
    val art by remember(near, family, banner, cover) {
        derivedStateOf(structuralEqualityPolicy()) {
            near.flatMap { other ->
                val title = titles.value[other]
                if (title != null) listOfNotNull(title.bannerUrl?.to(banner), title.coverUrl?.to(cover))
                else listOfNotNull(family.card(other)?.coverUrl?.to(cover))
            }
        }
    }
    LaunchedEffect(art) {
        for ((url, size) in art) store.artwork.request(url, size)
    }
}

/** The banner, the poster, the title and its other forms and metadata: the page's head. */
@Composable
private fun TitleHead(entry: Entry, episodes: List<EpisodeRow>?, list: LazyListState, inset: Dp, shared: Boolean) {
    val art = HeroArt.of(entry, episodes)
    Box {
        if (art != HeroArt.Compact) {
            ArtworkImage(
                url = art.url,
                plate = null,
                maxPixelSize = pixels(412.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(inset + 156.dp)
                    // The banner scrolls at half the page's speed (T1's parallax), read as the
                    // layer draws, so a scroll never recomposes the head.
                    .graphicsLayer {
                        translationY = (if (list.firstVisibleItemIndex == 0) list.firstVisibleItemScrollOffset else 0) * 0.5f
                    }
                    // The player grows out of the banner and closes back into it (T2, T3); only
                    // the page on screen takes part, not one waiting off screen.
                    .then(if (shared) Modifier.shared(Shared.Player, entry.id).shared(Shared.Art, entry.id) else Modifier),
            )
        }
        TitleHeader(entry, shared, Modifier.padding(top = inset + if (art == HeroArt.Compact) 64.dp else 112.dp))
    }
}

/** The facts on one line, then the top three tags on another, each cut at the end. */
@Composable
private fun FactsLines(entry: Entry) {
    val facts = entry.factsLine()
    val tags = entry.tagsLine()
    if (facts == null && tags == null) return
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 4.dp)) {
        for (line in listOfNotNull(facts, tags)) {
            Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** "More about this show", 48 high with a chevron, which opens the sheet. */
@Composable
private fun MoreAboutRow(onOpen: () -> Unit) {
    Row(
        Modifier
            .padding(top = 4.dp)
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(onClickLabel = "Open the sheet", onClick = onOpen)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("More about this show", style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
        Icon(Icons.Chevron, contentDescription = null, modifier = Modifier.size(20.dp))
    }
}

/** Every other relation as a rail of cover cards: the cover, the title, and what it is. */
@Composable
private fun RelatedRail(related: List<tv.anistream.engine.model.LinkedTitle>) {
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(top = 8.dp)) {
        Text("Related", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp).semantics { heading() })
        androidx.compose.foundation.lazy.LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            modifier = Modifier.padding(top = 4.dp),
        ) {
            items(related, key = { it.id.toLong() }) { linked ->
                Row(
                    itemMotion().clickable(onClickLabel = "Open title") { navigator.openTitle(linked.id, "related") },
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    ArtworkImage(linked.coverUrl, plate = linked.plate(), maxPixelSize = 120, modifier = Modifier.size(40.dp, 60.dp).clip(ArtShape.cover(40.dp)))
                    Column {
                        Text(linked.title, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 220.dp))
                        Text(linked.cardLine(), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1)
                    }
                }
            }
        }
    }
}

/** The poster, the title, the other title and the metadata line, standing on the banner's foot. */
@Composable
private fun TitleHeader(entry: Entry, shared: Boolean, modifier: Modifier = Modifier) {
    Row(
        modifier.padding(horizontal = 16.dp).heightIn(min = 120.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        ArtworkImage(
            url = entry.coverUrl,
            plate = null,
            maxPixelSize = 300,
            modifier = Modifier
                .size(80.dp, 120.dp)
                .then(if (shared) Modifier.shared(Shared.Cover, entry.id) else Modifier)
                .clip(ArtShape.cover(80.dp))
                .border(1.dp, ObiSilk.Silk.copy(alpha = 0.1f), ArtShape.cover(80.dp)),
        )
        Column(Modifier.weight(1f)) {
            Text(
                entry.showTitle,
                style = ShowTitle.page,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.semantics { heading() },
            )
            OtherForms(entry, Modifier.padding(top = 2.dp))
            if (entry.capsLine.isNotEmpty()) {
                Text(
                    entry.capsLine,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
        }
    }
}

/**
 * The title's other forms under it, in haze: both when they fit on one line, else only the first,
 * cut at the end.
 */
@Composable
private fun OtherForms(entry: Entry, modifier: Modifier = Modifier) {
    val forms = entry.otherForms()
    if (forms.isEmpty()) return
    val style = MaterialTheme.typography.bodyMedium
    val measurer = rememberTextMeasurer()
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val width = constraints.maxWidth
        val both = forms.joinToString(" \u00b7 ")
        val fits = remember(both, width) {
            !measurer.measure(both, style, maxLines = 1, constraints = Constraints(maxWidth = width)).hasVisualOverflow
        }
        Text(
            if (fits) both else forms.first(),
            style = style,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Back and More, each a 40 circle on surfaceContainer in a 48 target, over the banner. */
@Composable
private fun TitleTopBar(entry: Entry, onBack: () -> Unit, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    var more by remember { mutableStateOf(false) }
    var showSettings by rememberSaveable(entry.id) { mutableStateOf(false) }
    if (showSettings) ShowSettingsSheet(entry) { showSettings = false }
    Row(
        modifier.fillMaxWidth().height(64.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CircleIconButton(Icons.Back, "Back", onBack)
        Spacer(Modifier.weight(1f))
        Box {
            CircleIconButton(Icons.More, "More", { more = true })
            Menu(expanded = more, onDismissRequest = { more = false }) {
                DropdownMenuItem(
                    text = { Text("Settings for this show") },
                    leadingIcon = { Icon(Icons.Settings, null) },
                    onClick = {
                        more = false
                        showSettings = true
                    },
                )
                DropdownMenuItem(
                    text = { Text("Hide from Continue") },
                    leadingIcon = { Icon(Icons.HideRow, null) },
                    onClick = {
                        more = false
                        store.hideFromContinue(entry.id)
                    },
                )
                DropdownMenuItem(
                    text = { Text("Open on AniList") },
                    leadingIcon = { Icon(Icons.External, null) },
                    onClick = {
                        more = false
                        store.openExternal("https://anilist.co/anime/${entry.id}")
                    },
                )
            }
        }
    }
}

@Composable
private fun CircleIconButton(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    IconButton(onClick = onClick, modifier = Modifier.semantics { contentDescription = label }) {
        Box(
            Modifier.size(40.dp).background(MaterialTheme.colorScheme.surfaceContainer, CircleShape),
            contentAlignment = Alignment.Center,
        ) { Icon(icon, contentDescription = null) }
    }
}

/** The one amber action, then the list it is on and its trailer. */
@Composable
private fun TitleActions(entry: Entry, actions: AppActions) {
    val store = LocalAppStore.current
    val action = TitleAction.of(entry)
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 16.dp)) {
        if (action.playable) {
            Button(
                onClick = { actions.play(entry.id, action.episode.toString()) },
                modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                // What it plays crossfades as an episode is marked (T9).
                Crossfade(action.label, animationSpec = Motion.standard(), label = "action") {
                    Text(it, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
                }
            }
        } else {
            // Caught up: when the next one airs, in the action's place, and no amber on the page.
            CaughtUp(action, Modifier.heightIn(min = 56.dp))
        }
        Row(Modifier.padding(top = 4.dp).heightIn(min = 48.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            var open by remember { mutableStateOf(false) }
            Box(Modifier.weight(1f)) {
                FilledTonalButton(onClick = { open = true }, modifier = Modifier.fillMaxWidth()) {
                    // After a pick the label crossfades on quick, and the button keeps its size (T7).
                    androidx.compose.animation.AnimatedContent(
                        entry.listStatus?.label?.replaceFirstChar { it.uppercase() } ?: "Add to list",
                        transitionSpec = { fadeIn(Motion.quick()) togetherWith fadeOut(Motion.quick()) },
                        label = "list",
                    ) { Text(it) }
                    Spacer(Modifier.width(8.dp))
                    Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
                }
                Menu(expanded = open, onDismissRequest = { open = false }) {
                    for (segment in LibraryModel.order) {
                        DropdownMenuItem(
                            text = { Text(segment.label.replaceFirstChar { it.uppercase() }) },
                            leadingIcon = {
                                Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                    if (entry.listStatus == segment) Icon(Icons.Check, contentDescription = "Current")
                                }
                            },
                            onClick = {
                                open = false
                                store.setStatus(entry.id, segment)
                            },
                        )
                    }
                }
            }
            entry.trailerUrl?.let { trailer ->
                OutlinedButton(onClick = { store.openExternal(trailer) }, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Trailer, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Trailer")
                }
            }
        }
    }
}

/** "Episode 10 airs Thursday 16:30": a haze line with a water dot. */
@Composable
internal fun CaughtUp(action: TitleAction, modifier: Modifier = Modifier) {
    val context = androidx.compose.ui.platform.LocalContext.current
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(6.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
        Text(action.caughtUp(context), style = MaterialTheme.typography.bodyLarge.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * The synopsis cuts worked out, kept past the page that asked for them: a fold, an unfold or a
 * turn back to a width the synopsis was set at before takes the cut it had (AT3) instead of laying
 * the text out again. The most recent few, on the main thread.
 */
internal object SynopsisCuts {
    private data class Key(val text: String, val width: Int, val style: androidx.compose.ui.text.TextStyle, val density: Float, val fontScale: Float)

    /** A cut, or that the whole text fits. */
    private class Cut(val text: String?)

    private val cuts = object : LinkedHashMap<Key, Cut>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Cut>?) = size > 48
    }

    fun get(text: String, width: Int, style: androidx.compose.ui.text.TextStyle, density: androidx.compose.ui.unit.Density, cut: () -> String?): String? =
        cuts.getOrPut(Key(text, width, style, density.density, density.fontScale)) { Cut(cut()) }.text
}

/** What a cut synopsis ends with to be measured: the ellipsis and More, set a little wider. */
private const val SYNOPSIS_ENDING = "\u2026 More "

/**
 * The longest start of [text] that leaves room on its two lines at [width] for "… More", with the
 * ellipsis; `null` when the whole text fits. The answer a search over every length would give: it
 * is estimated from one layout of the whole text, where the ending would start on its second line,
 * then confirmed against the next length, widening the step only when the estimate is off. A few
 * layouts in all, where the search took one for every halving of the text's length.
 */
internal fun synopsisCut(text: String, width: Int, style: androidx.compose.ui.text.TextStyle, measurer: androidx.compose.ui.text.TextMeasurer): String? {
    fun layout(candidate: String) = measurer.measure(candidate, style, maxLines = 2, constraints = Constraints(maxWidth = width))
    fun fits(length: Int) = !layout(text.take(length).trimEnd() + SYNOPSIS_ENDING).hasVisualOverflow
    val whole = layout(text)
    if (!whole.hasVisualOverflow) return null
    val ending = measurer.measure(SYNOPSIS_ENDING, style).size.width
    val line = whole.lineCount - 1
    val estimate = whole.getOffsetForPosition(
        androidx.compose.ui.geometry.Offset(
            (width - ending).coerceAtLeast(0).toFloat(),
            (whole.getLineTop(line) + whole.getLineBottom(line)) / 2,
        ),
    ).coerceIn(0, text.length)
    // Bound the longest length that fits between one that does and one that does not, stepping
    // out from the estimate, then close in between them.
    var low: Int
    var high: Int
    if (fits(estimate)) {
        low = estimate
        var step = 1
        high = minOf(text.length, low + step)
        while (high > low && fits(high)) {
            low = high
            step *= 2
            high = minOf(text.length, low + step)
        }
        if (high == low) return text.take(low).trimEnd() + "\u2026"
    } else {
        high = estimate
        var step = 1
        low = maxOf(0, high - step)
        while (low > 0 && !fits(low)) {
            high = low
            step *= 2
            low = maxOf(0, high - step)
        }
    }
    // low fits (or is 0), high does not.
    while (high - low > 1) {
        val mid = (low + high) / 2
        if (fits(mid)) low = mid else high = mid
    }
    return text.take(low).trimEnd() + "\u2026"
}

/**
 * The synopsis in two lines, ending "… More" where there is more; a tap on it expands it in place,
 * and it stays open.
 */
@Composable
private fun Synopsis(text: String) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium
    val measurer = rememberTextMeasurer()
    BoxWithConstraints(
        Modifier
            .padding(horizontal = 16.dp)
            .padding(top = 4.dp)
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(enabled = !expanded) { expanded = true }
            .padding(vertical = 4.dp),
    ) {
        val width = constraints.maxWidth
        val density = LocalDensity.current
        val cut = remember(text, width, style, density) {
            SynopsisCuts.get(text, width, style, density) { synopsisCut(text, width, style, measurer) }
        }
        Text(
            buildAnnotatedString {
                if (expanded || cut == null) {
                    append(text)
                } else {
                    append(cut)
                    append(" ")
                    withStyle(SpanStyle(color = colors.onSurface, fontWeight = FontWeight.Medium)) { append("More") }
                }
            },
            style = style,
            color = colors.onSurfaceVariant,
        )
    }
}

/**
 * How a season picker of [count] seasons lays out. Side by side across the room it has when they
 * fit, the season chosen [CHOSEN] wide against 1 for the rest (T7), [unit] px each; else as a row
 * that scrolls sideways, each season as wide as its name and the chosen one's check and count
 * need, [pad] px at either end and the season chosen kept in view.
 */
@androidx.compose.runtime.Immutable
internal class SeasonGeometry(
    val count: Int,
    val unit: Float,
    val gap: Float,
    val pad: Float,
    val scrolls: Boolean,
    /** Whether the chosen season has room for its count beside its name and check. */
    val countFits: Boolean,
) {
    /** The row's width when it scrolls, its ends' padding with it. */
    val width: Float get() = 2 * pad + unit * (count - 1 + CHOSEN) + gap * (count - 1).coerceAtLeast(0)

    /** The scroll that puts the season at [index], chosen, in the middle of a [viewport] px wide, as far as the row's ends allow. */
    fun centred(index: Int, viewport: Int): Int {
        if (!scrolls || index < 0) return 0
        val middle = pad + index * (unit + gap) + unit * CHOSEN / 2
        return (middle - viewport / 2f).coerceIn(0f, (width - viewport).coerceAtLeast(0f)).roundToInt()
    }

    override fun equals(other: Any?): Boolean = other is SeasonGeometry && other.count == count && other.unit == unit &&
        other.gap == gap && other.pad == pad && other.scrolls == scrolls && other.countFits == countFits

    override fun hashCode(): Int = java.util.Objects.hash(count, unit, gap, pad, scrolls, countFits)

    companion object {
        /** The chosen season's width against the others'. */
        const val CHOSEN = 1.4f
    }
}

/**
 * The picker's layout for [count] seasons in [available] across, from its labels as set in
 * labelLarge at the current font scale: side by side while every name fits with room around it
 * and the chosen one's check beside its name, else a row that scrolls.
 */
@Composable
private fun rememberSeasonGeometry(count: Int, available: Dp): SeasonGeometry {
    val density = LocalDensity.current
    val measurer = rememberTextMeasurer()
    val style = MaterialTheme.typography.labelLarge
    return remember(count, available, density, style) {
        with(density) {
            fun width(text: String, of: androidx.compose.ui.text.TextStyle = style) =
                measurer.measure(text, of, maxLines = 1, softWrap = false).size.width.toFloat()
            val label = (1..count).maxOfOrNull { width("Season $it") } ?: 0f
            val count2 = width("00/00", style.tabular().copy(fontWeight = FontWeight.Normal))
            val check = (18.dp + 6.dp).toPx()
            val gap = 2.dp.toPx()
            // Side by side, a name needs 8 dp clear either side; a row that scrolls gives it 16.
            val tight = 2 * 8.dp.toPx()
            val roomy = 2 * 16.dp.toPx()
            val fit = if (count > 0) (available.toPx() - gap * (count - 1)) / (count - 1 + SeasonGeometry.CHOSEN) else 0f
            if (count > 0 && fit >= label + tight && fit * SeasonGeometry.CHOSEN >= check + label + tight) {
                val counted = check + label + 6.dp.toPx() + count2 + tight
                SeasonGeometry(count, fit, gap, pad = 0f, scrolls = false, countFits = fit * SeasonGeometry.CHOSEN >= counted)
            } else {
                val counted = check + label + 6.dp.toPx() + count2 + roomy
                val unit = maxOf(label + roomy, counted / SeasonGeometry.CHOSEN)
                SeasonGeometry(count, unit, gap, pad = 16.dp.toPx(), scrolls = true, countFits = true)
            }
        }
    }
}

/**
 * Keep the season [marked] in the middle of a picker that scrolls, as far as its ends allow: where
 * it stands as the picker first lays out, and on standard as a pick moves it, as a scrollable tab
 * row does. [viewport] is how wide the picker is on screen.
 */
@Composable
private fun FollowChosen(scroll: androidx.compose.foundation.ScrollState, geometry: SeasonGeometry, family: TitleFamily, marked: androidx.compose.runtime.State<Long>, viewport: Int) {
    val index = family.seasons.indexOfFirst { it.id.toLong() == marked.value }
    var placed by remember(geometry, viewport) { mutableStateOf(false) }
    LaunchedEffect(geometry, index, viewport) {
        if (!geometry.scrolls || index < 0) return@LaunchedEffect
        val target = geometry.centred(index, viewport)
        if (!placed) {
            scroll.scrollTo(target)
            placed = true
        } else if (scroll.value != target) {
            scroll.animateScrollTo(target, Motion.standard())
        }
    }
}

/**
 * The seasons as a connected group: the one chosen selected with its count, the others a tap
 * away. A pick changes the page in place (T7): the selection morphs as [ConnectedButton]'s does,
 * its width on the same spring as its corners, and the count comes and goes with the check. When
 * they do not fit across, the group scrolls sideways under [scroll], the season at the edge cut
 * to say so.
 */
@Composable
private fun Seasons(
    chosenId: AnilistId,
    entry: Entry?,
    family: TitleFamily,
    geometry: SeasonGeometry,
    scroll: androidx.compose.foundation.ScrollState,
    onPick: (AnilistId) -> Unit,
    modifier: Modifier = Modifier,
    inset: Boolean = true,
    onPress: (AnilistId) -> Unit = {},
    onLift: () -> Unit = {},
) {
    val store = LocalAppStore.current
    val motion = MaterialTheme.motionScheme
    val seasons = family.seasons
    // How far each season is chosen, on the shape's spring: its width follows, read as the group
    // is laid out, so the morph recomposes nothing.
    val chosen = seasons.map { season ->
        key(season.id) { androidx.compose.animation.core.animateFloatAsState(if (season.id == chosenId) 1f else 0f, motion.defaultSpatialSpec(), label = "width") }
    }
    val row = if (geometry.scrolls) {
        Modifier.fillMaxWidth().horizontalScroll(scroll).padding(horizontal = with(LocalDensity.current) { geometry.pad.toDp() })
    } else {
        Modifier.padding(horizontal = if (inset) 16.dp else 0.dp).fillMaxWidth()
    }
    androidx.compose.ui.layout.Layout(
        modifier = modifier.then(row).heightIn(min = 48.dp),
        content = {
            seasons.forEachIndexed { index, season ->
                key(season.id) {
                    val isChosen = season.id == chosenId
                    ConnectedButtonContent(
                        chosen = isChosen,
                        first = index == 0,
                        last = index == seasons.lastIndex,
                        onClick = { onPick(season.id) },
                        // The season's episodes are asked for as the finger lands, ahead of the pick.
                        onPress = {
                            store.loadEpisodes(season.id)
                            onPress(season.id)
                        },
                        onRelease = onLift,
                        enabled = !isChosen,
                    ) { color ->
                        androidx.compose.foundation.text.BasicText(
                            "Season ${index + 1}",
                            style = MaterialTheme.typography.labelLarge,
                            color = color,
                            maxLines = 1,
                            softWrap = false,
                        )
                        val done = entry?.progress?.done?.toInt()
                        val total = entry?.episodes?.takeIf { it > 0u }
                        val count = if (isChosen && geometry.countFits && done != null && total != null) done to total else null
                        // The count as it last read, while it leaves with the selection.
                        var last by remember { mutableStateOf(count) }
                        if (count != null) last = count
                        AnimatedVisibility(count != null, enter = markIn, exit = markOut) {
                            last?.let { (watched, of) ->
                                Row(Modifier.padding(start = 6.dp)) {
                                    val style = MaterialTheme.typography.labelLarge.tabular().copy(fontWeight = FontWeight.Normal)
                                    // Only the changed digit moves as an episode is marked (T9).
                                    val countColor = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.8f)
                                    RollingCount(watched, style, countColor)
                                    Text("/$of", style = style, color = countColor)
                                }
                            }
                        }
                    }
                }
            }
        },
    ) { measurables, constraints ->
        val weights = FloatArray(measurables.size) { 1f + (SeasonGeometry.CHOSEN - 1f) * (chosen.getOrNull(it)?.value ?: 0f) }
        val gap = geometry.gap
        // Side by side, the seasons share the width as their weights say, whatever the springs
        // stand at; scrolling, each is its own width.
        val scale = if (geometry.scrolls) geometry.unit else (constraints.maxWidth - gap * (measurables.size - 1)).coerceAtLeast(0f) / weights.sum().coerceAtLeast(1f)
        var x = 0f
        val placed = measurables.mapIndexed { i, measurable ->
            val left = x.roundToInt()
            x += weights[i] * scale
            val width = (x.roundToInt() - left).coerceAtLeast(0)
            x += gap
            left to measurable.measure(Constraints(minWidth = width, maxWidth = width, minHeight = 0, maxHeight = constraints.maxHeight))
        }
        val height = maxOf(placed.maxOfOrNull { it.second.height } ?: 0, constraints.minHeight)
        val width = if (geometry.scrolls) (x - gap).roundToInt().coerceAtLeast(0) else constraints.maxWidth
        layout(width, height) {
            for ((left, placeable) in placed) placeable.place(left, (height - placeable.height) / 2)
        }
    }
}

/**
 * The episodes, a row each, from the last one watched, the watched ones before it folded under
 * one row; plates in their place while the table loads.
 */
private fun LazyListScope.episodeRows(
    entry: Entry,
    episodes: List<EpisodeRow>?,
    actions: AppActions,
    unfolded: Boolean,
    onFold: (anchor: String) -> Unit,
    /** Whether the placeholders wait 600 ms first (T8): not on a page composed ahead, off screen. */
    wait: Boolean = true,
) {
    if (episodes == null) {
        // Crossfading with the rows as they arrive (T8), as every item comes and goes.
        items(4, key = { "placeholder-$it" }, contentType = { "placeholder" }) {
            Box(itemMotion()) { if (wait) AfterWait { EpisodePlaceholder() } else EpisodePlaceholder() }
        }
        return
    }
    val next = entry.progress?.next ?: 1u
    val listed = entry.listedEpisodes(episodes)
    val folded = entry.foldedEpisodes(listed)
    if (folded.isNotEmpty()) {
        val anchor = episodeKey(entry.id, listed[folded.size])
        item(key = watchedFold(entry.id), contentType = "fold") { WatchedFoldRow(folded, unfolded, itemMotion().padding(horizontal = 16.dp)) { onFold(anchor) } }
    }
    val shown = if (folded.isNotEmpty() && !unfolded) listed.drop(folded.size) else listed
    items(shown, key = { episodeKey(entry.id, it) }, contentType = { "episode" }) { row ->
        Box(itemMotion()) { EpisodeRowView(entry, row, next, actions) }
    }
}

/**
 * An episode row's key in the page's list: [id]'s own, so a pick in the season picker fades one
 * season's rows out as the other's fade in (T7).
 */
internal fun episodeKey(id: AnilistId, row: EpisodeRow): String = "episode-$id-${row.number}"

/** The fold row's key in [id]'s page. */
internal fun watchedFold(id: AnilistId): String = "watched-fold-$id"

/**
 * Fold or unfold [id]'s watched episodes, keeping the row named [anchor], the first shown while
 * folded, where it stands on screen: the list grows or shrinks above it, and nothing jumps.
 */
@Composable
private fun rememberEpisodeFold(id: AnilistId, list: androidx.compose.foundation.lazy.LazyListState): (String) -> Unit {
    val store = LocalAppStore.current
    val scope = rememberCoroutineScope()
    return remember(id, list) {
        { anchor ->
            val before = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == anchor }?.offset
            val count = list.layoutInfo.totalItemsCount
            store.episodeFolds.toggle(id)
            if (before != null) {
                scope.launch {
                    androidx.compose.runtime.snapshotFlow { list.layoutInfo }.first { it.totalItemsCount != count }
                    val after = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == anchor }?.offset
                    if (after != null) list.scrollBy((after - before).toFloat())
                }
            }
        }
    }
}

/** [rememberEpisodeFold] for the tablet's two columns of episodes. */
@Composable
private fun rememberEpisodeFold(id: AnilistId, grid: androidx.compose.foundation.lazy.grid.LazyGridState): (String) -> Unit {
    val store = LocalAppStore.current
    val scope = rememberCoroutineScope()
    return remember(id, grid) {
        { anchor ->
            val before = grid.layoutInfo.visibleItemsInfo.firstOrNull { it.key == anchor }?.offset?.y
            val count = grid.layoutInfo.totalItemsCount
            store.episodeFolds.toggle(id)
            if (before != null) {
                scope.launch {
                    androidx.compose.runtime.snapshotFlow { grid.layoutInfo }.first { it.totalItemsCount != count }
                    val after = grid.layoutInfo.visibleItemsInfo.firstOrNull { it.key == anchor }?.offset?.y
                    if (after != null) grid.scrollBy((after - before).toFloat())
                }
            }
        }
    }
}

/**
 * "Episodes 1–5 watched" over the folded episodes, or "Hide watched episodes" once they show: a
 * quiet one-line row with a chevron, not a button.
 */
@Composable
private fun WatchedFoldRow(folded: List<EpisodeRow>, unfolded: Boolean, modifier: Modifier = Modifier, onToggle: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val turn by androidx.compose.animation.core.animateFloatAsState(if (unfolded) 180f else 0f, Motion.quick(), label = "fold")
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(onClickLabel = if (unfolded) "Hide watched episodes" else "Show watched episodes", onClick = onToggle)
            .semantics { stateDescription = if (unfolded) "Expanded" else "Collapsed" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            if (unfolded) "Hide watched episodes" else watchedRangeWords(folded),
            style = MaterialTheme.typography.bodyLarge,
            color = colors.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        Icon(Icons.Down, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.graphicsLayer { rotationZ = turn })
    }
}

@Composable
private fun EpisodePlaceholder() {
    Row(
        Modifier.fillMaxWidth().height(72.dp).padding(start = 16.dp, end = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(96.dp, 54.dp).clip(ArtShape.cover(96.dp)).background(MaterialTheme.colorScheme.surfaceContainer))
        Column(Modifier.weight(1f)) {
            TextPlaceholder(0.7f, 24.dp)
            TextPlaceholder(0.35f, 20.dp)
        }
    }
}

/** Seconds from now, as the rows compare a broadcast against it. */
private fun nowSeconds() = AppClock.now().epochSecond

@Composable
private fun EpisodeRowView(entry: Entry, row: EpisodeRow, next: UInt, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val number = row.number.toUIntOrNull() ?: 0u
    val current = number == next
    val airsAt = row.airsAt
    val unaired = airsAt != null && airsAt > nowSeconds()
    val hidden = SPOILER_PROTECTION && number > next && !row.completed && !unaired
    var menu by remember { mutableStateOf(false) }
    Box {
        EpisodeLine(
            entry = entry,
            row = row,
            current = current,
            unaired = unaired,
            hidden = hidden,
            onPlay = { if (!unaired) actions.play(entry.id, row.number) },
            onMenu = { menu = true },
        )
        EpisodeMenu(entry, row, expanded = menu, onDismiss = { menu = false }, actions = actions)
    }
}

/**
 * A title's page on a tablet, as AndroidTabletTitle draws it: the banner across the content, the
 * poster beside the title and its actions, the details to the right, the seasons and what is
 * related in one row, and the episodes in two columns.
 */
@Composable
private fun TabletTitle(
    /** The title the page was opened as, whose bounds it grew from and closes into. */
    id: AnilistId,
    entry: Entry,
    family: TitleFamily,
    allEpisodes: Map<AnilistId, List<EpisodeRow>>,
    actions: AppActions,
    onPick: (AnilistId) -> Unit,
) {
    val navigator = LocalNavigator.current
    val marks = remember(entry.id) { UserMarks() }
    val inset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val episodes = allEpisodes[entry.id]
    val next = entry.progress?.next ?: 1u
    val colors = MaterialTheme.colorScheme
    var aboutOpen by rememberSaveable(entry.id) { mutableStateOf(false) }
    val store = LocalAppStore.current
    val unfolded by store.episodeFolds.unfolded.collectAsStateWithLifecycle()
    val grid = androidx.compose.foundation.lazy.grid.rememberLazyGridState()
    val fold = rememberEpisodeFold(entry.id, grid)
    // Back, the gesture or the key, closes the pane before it leaves the page.
    androidx.activity.compose.BackHandler(enabled = aboutOpen) { aboutOpen = false }
    androidx.compose.runtime.CompositionLocalProvider(LocalUserMarks provides marks) {
        Row(Modifier.fillMaxSize().shared(Shared.Page, id).background(colors.surface)) {
        Box(Modifier.weight(1f).fillMaxHeight()) {
            androidx.compose.foundation.lazy.grid.LazyVerticalGrid(
                state = grid,
                columns = androidx.compose.foundation.lazy.grid.GridCells.Fixed(2),
                contentPadding = PaddingValues(start = 8.dp, end = 24.dp, bottom = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                fullWidth("head") {
                  SeasonFade(entry) { entry ->
                    val art = HeroArt.of(entry, allEpisodes[entry.id])
                    Box {
                        if (art != HeroArt.Compact) {
                            ArtworkImage(
                                art.url,
                                plate = null,
                                maxPixelSize = 1600,
                                modifier = Modifier.fillMaxWidth().height(inset + 208.dp).shared(Shared.Art, entry.id),
                            )
                        }
                        // The cover overlaps the banner's foot, the title block sits level with
                        // the cover's foot, and the details column starts below the banner, so it
                        // grows down the page and never up over the art.
                        val below = if (art == HeroArt.Compact) 0.dp else 208.dp - 135.dp + 24.dp
                        Row(
                            Modifier.padding(top = inset + if (art == HeroArt.Compact) 64.dp else 135.dp).padding(start = 16.dp),
                            horizontalArrangement = Arrangement.spacedBy(24.dp),
                            verticalAlignment = Alignment.Top,
                        ) {
                          Row(
                            Modifier.weight(1f),
                            horizontalArrangement = Arrangement.spacedBy(24.dp),
                            verticalAlignment = Alignment.Bottom,
                          ) {
                            ArtworkImage(
                                entry.coverUrl,
                                plate = null,
                                maxPixelSize = 500,
                                modifier = Modifier
                                    .size(150.dp, 225.dp)
                                    .shared(Shared.Cover, entry.id)
                                    .clip(ArtShape.cover(150.dp))
                                    .border(1.dp, ObiSilk.Silk.copy(alpha = 0.1f), ArtShape.cover(150.dp)),
                            )
                            Column(Modifier.weight(1f).padding(bottom = 0.dp)) {
                                Text(entry.showTitle, style = ShowTitle.page, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { heading() })
                                entry.alternateTitle?.let {
                                    Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                }
                                if (entry.capsLine.isNotEmpty()) {
                                    Text(entry.capsLine, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
                                }
                                TabletActions(entry, actions, Modifier.padding(top = 4.dp))
                            }
                          }
                            if (!aboutOpen) TitleFacts(entry, Modifier.padding(top = below).width(376.dp)) { aboutOpen = true }
                        }
                    }
                  }
                }
                if (family.seasons.isNotEmpty() || family.related.isNotEmpty()) {
                    fullWidth("family") {
                        if (family.seasons.isNotEmpty()) TabletSeasons(entry, family, onPick) else Spacer(Modifier.height(24.dp))
                    }
                }
                if (entry.relatedCards().isNotEmpty()) {
                    fullWidth("related") { SeasonFade(entry, itemMotion()) { RelatedRail(it.relatedCards()) } }
                }
                fullWidth("gap") { Spacer(Modifier.height(16.dp)) }
                if (episodes == null) {
                    items(4) { AfterWait { EpisodePlaceholder() } }
                } else {
                    val listed = entry.listedEpisodes(episodes)
                    // Folded in pairs, so the last watched keeps its column.
                    val folded = entry.foldedEpisodes(listed, columns = 2)
                    if (folded.isNotEmpty()) {
                        val anchor = episodeKey(entry.id, listed[folded.size])
                        fullWidth(watchedFold(entry.id)) {
                            WatchedFoldRow(folded, entry.id in unfolded, itemMotion().padding(start = 16.dp)) { fold(anchor) }
                        }
                    }
                    val shown = if (folded.isNotEmpty() && entry.id !in unfolded) listed.drop(folded.size) else listed
                    items(shown.size, key = { episodeKey(entry.id, shown[it]) }) { index ->
                        val row = shown[index]
                        Box(itemMotion().clip(MaterialTheme.shapes.large)) { EpisodeRowView(entry, row, next, actions) }
                    }
                }
            }
            TitleTopBar(entry, onBack = navigator::back, modifier = Modifier.padding(top = inset))
        }
        if (aboutOpen) {
            AboutPane(entry, onClose = { aboutOpen = false }) { other ->
                aboutOpen = false
                navigator.openTitle(other, "recommendation")
            }
        }
        }
    }
}

/**
 * The tablet's season picker, 520 wide as AndroidTabletTitle has it while the seasons fit, else
 * across the page, scrolling sideways with the season on screen kept in the middle.
 */
@Composable
private fun TabletSeasons(entry: Entry, family: TitleFamily, onPick: (AnilistId) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth().padding(top = 24.dp)) {
        val geometry = rememberSeasonGeometry(family.seasons.size, minOf(520.dp, maxWidth - 16.dp))
        val viewport = constraints.maxWidth
        val chosen = rememberUpdatedState(entry.id.toLong())
        val scroll = rememberSaveable(saver = androidx.compose.foundation.ScrollState.Saver) {
            androidx.compose.foundation.ScrollState(geometry.centred(family.seasons.indexOfFirst { it.id == entry.id }, viewport))
        }
        FollowChosen(scroll, geometry, family, chosen, viewport)
        if (geometry.scrolls) {
            Seasons(entry.id, entry, family, geometry, scroll, onPick)
        } else {
            Box(Modifier.padding(start = 16.dp).width(520.dp)) { Seasons(entry.id, entry, family, geometry, scroll, onPick, inset = false) }
        }
    }
}

/** The supporting pane, 384 wide in the details column's place: About and the sheet's sections. */
@Composable
private fun AboutPane(entry: Entry, onClose: () -> Unit, onOpenTitle: (AnilistId) -> Unit) {
    val inset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    Column(
        Modifier
            .padding(top = inset, end = 24.dp, bottom = 24.dp)
            .width(384.dp)
            .fillMaxHeight()
            .clip(MaterialTheme.shapes.large)
            .background(MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Row(Modifier.heightIn(min = 64.dp).padding(start = 24.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("More about this show", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f).semantics { heading() })
            IconButton(onClick = onClose, modifier = Modifier.semantics { contentDescription = "Close" }) {
                Icon(Icons.Clear, contentDescription = null)
            }
        }
        MoreAboutContent(entry, about = true, heading = false, onOpenTitle = onOpenTitle)
    }
}

private fun androidx.compose.foundation.lazy.grid.LazyGridScope.fullWidth(
    key: String,
    content: @Composable androidx.compose.foundation.lazy.grid.LazyGridItemScope.() -> Unit,
) {
    item(key = key, span = { androidx.compose.foundation.lazy.grid.GridItemSpan(maxLineSpan) }) { content() }
}

/** The tablet's actions in one row: the amber action sized to its words, the list, the trailer. */
@Composable
private fun TabletActions(entry: Entry, actions: AppActions, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val action = TitleAction.of(entry)
    var open by remember { mutableStateOf(false) }
    Row(modifier.heightIn(min = 56.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (action.playable) {
            Button(onClick = { actions.play(entry.id, action.episode.toString()) }, modifier = Modifier.heightIn(min = 56.dp), contentPadding = PaddingValues(horizontal = 24.dp)) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Crossfade(action.label, animationSpec = Motion.standard(), label = "action") {
                    Text(it, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
                }
            }
        } else {
            CaughtUp(action, Modifier.padding(horizontal = 8.dp))
        }
        Box {
            FilledTonalButton(onClick = { open = true }, contentPadding = PaddingValues(start = 16.dp, end = 12.dp)) {
                androidx.compose.animation.AnimatedContent(
                    entry.listStatus?.label?.replaceFirstChar { it.uppercase() } ?: "Add to list",
                    transitionSpec = { fadeIn(Motion.quick()) togetherWith fadeOut(Motion.quick()) },
                    label = "list",
                ) { Text(it) }
                Spacer(Modifier.width(8.dp))
                Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
            }
            Menu(expanded = open, onDismissRequest = { open = false }) {
                for (segment in LibraryModel.order) {
                    DropdownMenuItem(
                        text = { Text(segment.label.replaceFirstChar { it.uppercase() }) },
                        leadingIcon = {
                            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                if (entry.listStatus == segment) Icon(Icons.Check, contentDescription = "Current")
                            }
                        },
                        onClick = {
                            open = false
                            store.setStatus(entry.id, segment)
                        },
                    )
                }
            }
        }
        entry.trailerUrl?.let { trailer ->
            OutlinedButton(onClick = { store.openExternal(trailer) }, contentPadding = PaddingValues(start = 16.dp, end = 24.dp)) {
                Icon(Icons.Trailer, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("Trailer")
            }
        }
    }
}

/**
 * The details column beside the title: the synopsis, the facts (Studio, Based on, the airing
 * state, Also on), the top three tags, and "More about this show", which opens the pane.
 */
@Composable
private fun TitleFacts(entry: Entry, modifier: Modifier = Modifier, onMore: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Column(modifier) {
        val synopsis = synopsisWithoutSpoilers(entry.synopsis)
        if (synopsis.isNotEmpty()) {
            Text(synopsis, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 4, overflow = TextOverflow.Ellipsis)
        }
        val airing = entry.airingState()
        val facts = listOfNotNull(
            entry.studio?.let { "STUDIO" to it },
            entry.source?.label()?.let { "BASED ON" to it },
            airing?.let { it.label.uppercase() to it.value },
            entry.alsoOn()?.let { "ALSO ON" to it },
        )
        if (facts.isNotEmpty()) {
            Column(Modifier.padding(top = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (row in facts.chunked(3)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        for ((label, value) in row) {
                            Column(Modifier.weight(1f)) {
                                Text(label, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant)
                                Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                        repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }
        entry.tagsLine()?.let {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 12.dp))
        }
        FilledTonalButton(onClick = onMore, modifier = Modifier.padding(top = 12.dp)) { Text("More about this show") }
    }
}
