// Home: the spotlight, then the engine's sections as shelves, in the order it sends them. The
// engine decides which sections there are and what each holds; this draws them. A LazyColumn of
// LazyRow shelves, on phones and tablets alike.

package tv.anistream.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.design.ArtShape
import tv.anistream.design.CoverPlate
import tv.anistream.design.Motion
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.HomeEnd
import tv.anistream.engine.model.HomePart
import tv.anistream.engine.model.HomeView
import tv.anistream.engine.model.SectionKind
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.without

/** How long a pending part holds the activity line back: AniList's slow answers are 4 s. */
private const val ACTIVITY_AFTER_MILLIS = 4_000L

/** What a card or the spotlight can ask of Home: play, hide a title, leave Continue. */
internal class HomeHandlers(
    val actions: AppActions,
    /** Not interested: the title and whether it comes back when it airs. */
    val notInterested: (Entry, Boolean) -> Unit,
    /** Remove from Continue, with Undo. */
    val remove: (AnilistId) -> Unit,
)

@Composable
fun HomeScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val snackbars = LocalSnackbars.current
    val view by store.home.view.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    LaunchedEffect(Unit) { store.loadHome() }

    // One notice for each send that reports a part that did not load, with Try again; the ones
    // already counted when the screen opens were said before.
    val failures by store.home.failures.collectAsStateWithLifecycle()
    var counted by remember { mutableIntStateOf(failures) }
    val failedText = stringResource(R.string.home_failed)
    val tryAgain = stringResource(R.string.home_try_again)
    LaunchedEffect(failures) {
        if (failures > counted) {
            counted = failures
            snackbars.show(failedText, tryAgain) { store.loadHome(refresh = true) }
        }
    }

    // Titles taken off by Not interested or Remove from Continue, gone at once while their Undo
    // stands and until the engine's next send leaves them out.
    val gone = remember { mutableStateListOf<AnilistId>() }
    val handlers = HomeHandlers(actions, rememberDismiss(gone), rememberLeaveContinue(gone))

    val pending = view?.pending.orEmpty()
    val slow = pendingForLong(pending.isNotEmpty())
    val offlineSince = view?.offlineSince
    val line = when {
        view?.offline == true && offlineSince != null -> FrameLine.OfflineFrom(Strings.homeOffline(context, clock(offlineSince)))
        slow -> FrameLine.OfflineFrom(Strings.homeLoading(context))
        else -> null
    }

    ScreenFrame(
        title = stringResource(R.string.home_title),
        account = Account(user, actions.openSettings),
        line = line,
    ) { padding ->
        val top = padding.calculateTopPadding()
        Refreshable({ store.loadHome(refresh = true) }, top = top) {
            val home = view
            when {
                home == null -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top, bottom = 24.dp)) {
                    item(key = "loading") { AfterWait { HomePlaceholders(listOf(HomePart.SEASON, HomePart.SUGGESTIONS), spotlight = true) } }
                }
                // Offline with nothing kept: the existing state, with the way to what plays.
                home.offline && home.offlineSince == null && home.spotlight == null && home.sections.isEmpty() ->
                    OfflineHome(Modifier.padding(top = top))
                else -> HomeColumn(home, handlers, gone, top)
            }
        }
    }
}

/** The kept Home's time, as the clock shows it. */
private fun clock(unix: Long): String =
    DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(unix * 1000))

/** Whether something has been pending for four seconds: the activity line's cue. */
@Composable
private fun pendingForLong(pending: Boolean): Boolean {
    var past by remember { mutableStateOf(false) }
    LaunchedEffect(pending) {
        past = false
        if (pending) {
            delay(ACTIVITY_AFTER_MILLIS)
            past = true
        }
    }
    return pending && past
}

@Composable
private fun HomeColumn(home: HomeView, h: HomeHandlers, gone: List<AnilistId>, top: Dp) {
    val list = rememberLazyListState()
    val goneSet = gone.toSet()
    LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(top = top, bottom = 24.dp)) {
        // The spotlight changes in place: a crossfade on standard when its title or case moves, and
        // only then, so a sources check or a mark inside it never flashes.
        item(key = "spotlight", contentType = "spotlight") {
            val spot = home.spotlight?.takeUnless { it.entry.id in goneSet }
            Box(itemMotion()) {
                AnimatedContent(
                    targetState = spot,
                    contentKey = { it?.let { s -> s.entry.id to s.case } },
                    transitionSpec = { fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard()) },
                    label = "spotlight", // i18n-skip
                ) { shown ->
                    if (shown != null) SpotlightBlock(shown, h)
                    else if (home.pending.isNotEmpty()) AfterWait { SpotlightPlaceholder() }
                }
            }
        }
        for (section in home.sections) {
            val items = section.without(goneSet)
            if (items.isEmpty()) continue
            item(key = "${section.kind.name}-${section.title}", contentType = section.kind.name) {
                Box(itemMotion()) {
                    when (section.kind) {
                        SectionKind.CONTINUE -> ContinueShelf(section, items, h)
                        SectionKind.THIS_WEEK -> WeekShelf(section, items)
                        else -> PosterShelf(section, items, h)
                    }
                }
            }
        }
        // What is still on its way, held in place below what has arrived: nothing above it moves
        // when it lands.
        if (home.pending.isNotEmpty()) {
            item(key = "pending", contentType = "pending") {
                Box(itemMotion()) { AfterWait { HomePlaceholders(home.pending, spotlight = false) } }
            }
        }
        home.end?.let { end ->
            item(key = "end", contentType = "end") { Box(itemMotion()) { EndLine(end, h.actions) } }
        }
    }
}

/** The closing line: Seasonal and Calendar, Plugin stores, or Sign in to AniList, as the engine says. */
@Composable
private fun EndLine(end: HomeEnd, actions: AppActions) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val text = when (end) {
        HomeEnd.EVERYTHING -> Strings.homeEnd(context)
        HomeEnd.SIGN_IN -> Strings.homeEndSignedOut(context)
        HomeEnd.NO_SOURCES -> Strings.homeEndNoSources(context)
    }
    val links: List<Pair<String, () -> Unit>> = when (end) {
        HomeEnd.EVERYTHING -> listOf(
            Strings.homeSeasonal(context) to { navigator.go(Destination.Seasonal) },
            Strings.navCalendar(context) to { navigator.go(Destination.Calendar) },
        )
        HomeEnd.SIGN_IN -> emptyList()
        HomeEnd.NO_SOURCES -> listOf(Strings.navPluginStores(context) to { navigator.open(STORES_ROUTE) })
    }
    val (annotated, missing) = linked(text, links)
    Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 24.dp)) {
        HorizontalDivider(color = colors.outlineVariant)
        Spacer(Modifier.height(16.dp))
        Text(annotated, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        // A translation that words a link differently still gets its link, as a button.
        for ((word, go) in missing) TextButton(onClick = go) { Text(word) }
        if (end == HomeEnd.SIGN_IN) {
            OutlinedButton(onClick = actions.connectAniList, modifier = Modifier.padding(top = 12.dp)) {
                Text(Strings.homeSignIn(context))
            }
        }
    }
}

/** [text] with each of [links]' words made a link where the sentence has it, and the rest unmatched. */
@Composable
private fun linked(text: String, links: List<Pair<String, () -> Unit>>): Pair<AnnotatedString, List<Pair<String, () -> Unit>>> {
    val colors = MaterialTheme.colorScheme
    val found = links.mapNotNull { (word, go) -> text.indexOf(word).takeIf { it >= 0 }?.let { Triple(it, word, go) } }.sortedBy { it.first }
    val annotated = buildAnnotatedString {
        var at = 0
        for ((start, word, go) in found) {
            if (start < at) continue
            append(text.substring(at, start))
            withLink(
                LinkAnnotation.Clickable(
                    word,
                    TextLinkStyles(SpanStyle(color = colors.onSurface, textDecoration = TextDecoration.Underline)),
                ) { go() },
            ) { append(word) }
            at = start + word.length
        }
        append(text.substring(at))
    }
    return annotated to links.filter { l -> found.none { it.second == l.first } }
}

/** Offline with nothing kept: You're offline, and the way to what plays without the network. */
@Composable
private fun OfflineHome(modifier: Modifier) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    StateBlock(
        glyph = { Icon(Icons.Offline, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(32.dp)) },
        title = Strings.homeOfflineEmptyTitle(context),
        line = Strings.homeOfflineEmptyBody(context),
        modifier = modifier,
    ) {
        FilledTonalButton(onClick = { navigator.open(DOWNLOADS_ROUTE) }) { Text(Strings.homeOpenDownloads(context)) }
    }
}

/**
 * Not interested: the title leaves every shelf at once, the engine hides it, and a notice offers
 * Undo for 8 s. Undo restores it and the engine sends Home again with it.
 */
@Composable
private fun rememberDismiss(gone: MutableList<AnilistId>): (Entry, Boolean) -> Unit {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val undo = stringResource(R.string.action_undo)
    return remember(store, snackbars, context, undo) {
        { entry, untilAirs ->
            val id = entry.id
            gone += id
            store.dismissTitle(id)
            val words = if (untilAirs) Strings.upNextHiddenUntilAirs(context, entry.showTitle) else Strings.upNextHidden(context, entry.showTitle)
            snackbars.show(words, undo) {
                gone -= id
                store.restoreTitle(id)
            }
        }
    }
}

/**
 * Remove from Continue: the card goes at once and a notice offers Undo; only once the notice has
 * gone does the engine hide it, as there is nothing to undo there.
 */
@Composable
private fun rememberLeaveContinue(gone: MutableList<AnilistId>): (AnilistId) -> Unit {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val scope = rememberCoroutineScope()
    val hiddenText = stringResource(R.string.continue_hidden)
    val undoText = stringResource(R.string.action_undo)
    return remember(store, snackbars, hiddenText, undoText) {
        { id ->
            gone += id
            scope.launch {
                var undone = false
                try {
                    val result = withTimeoutOrNull(snackbarMillis(hiddenText, true)) {
                        snackbars.host.currentSnackbarData?.dismiss()
                        snackbars.host.showSnackbar(hiddenText, actionLabel = undoText, duration = SnackbarDuration.Indefinite)
                    }
                    if (result == null) snackbars.host.currentSnackbarData?.dismiss()
                    undone = result == SnackbarResult.ActionPerformed
                } finally {
                    if (!undone) store.hideFromContinue(id)
                    gone -= id
                }
            }
        }
    }
}

/** The spotlight's layout before it arrives: the art's plate and the lines of its text. */
@Composable
internal fun SpotlightPlaceholder() {
    Column(Modifier.padding(horizontal = 16.dp)) {
        CoverPlate(null, Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(ArtShape.banner))
        TextPlaceholder(0.6f, 28.dp, Modifier.padding(top = 8.dp))
        TextPlaceholder(0.4f, 20.dp)
        TextPlaceholder(1f, 56.dp, Modifier.padding(top = 12.dp))
    }
}

/** The shelves still on their way, as the layout they will take, held in place. */
@Composable
internal fun HomePlaceholders(parts: List<HomePart>, spotlight: Boolean) {
    Column {
        if (spotlight) SpotlightPlaceholder()
        repeat(parts.size.coerceAtLeast(1)) {
            TextPlaceholder(0.3f, 48.dp, Modifier.padding(start = 16.dp, top = 12.dp))
            androidx.compose.foundation.lazy.LazyRow(
                userScrollEnabled = false,
                contentPadding = PaddingValues(horizontal = 16.dp),
                horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
            ) {
                items(4) { Box(Modifier.width(posterWidth())) { PosterPlaceholder() } }
            }
        }
    }
}
