// A title's page on a phone, as AndroidTitle draws it: the banner, the poster and the title, the
// one amber action, the list and the trailer, the synopsis, the seasons and what is related, and
// the episodes, those past the next one with their stills hidden.

package tv.anistream.ui

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.store.AirTime
import tv.anistream.store.HeroArt
import tv.anistream.store.LibraryModel
import tv.anistream.store.label
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow

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

@Composable
fun TitleScreen(id: AnilistId, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val allEpisodes by store.titles.episodes.collectAsStateWithLifecycle()
    val entry = titles[id] ?: placeholderEntry(id)
    val episodes = allEpisodes[id]
    LaunchedEffect(id) {
        if (id !in store.titles.detailed) store.loadDetail(id)
        store.loadEpisodes(id)
    }
    val art = HeroArt.of(entry, episodes)
    val list = rememberLazyListState()
    val inset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    if (wideWindow()) {
        TabletTitle(entry, episodes, art, titles, actions)
        return
    }

    val marks = remember(id) { UserMarks() }
    androidx.compose.runtime.CompositionLocalProvider(LocalUserMarks provides marks) {
    Box(Modifier.fillMaxSize().shared(Shared.Page, id).background(MaterialTheme.colorScheme.surface)) {
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "head") {
                val density = LocalDensity.current
                Box {
                    if (art != HeroArt.Compact) {
                        // The banner scrolls at half the page's speed (T1's parallax).
                        val scrolled = if (list.firstVisibleItemIndex == 0) list.firstVisibleItemScrollOffset else 0
                        ArtworkImage(
                            url = art.url,
                            plate = null,
                            maxPixelSize = with(density) { 412.dp.roundToPx() },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(inset + 156.dp)
                                .graphicsLayer { translationY = scrolled * 0.5f }
                                .shared(Shared.Art, id),
                        )
                    }
                    TitleHeader(entry, Modifier.padding(top = inset + if (art == HeroArt.Compact) 64.dp else 112.dp))
                }
            }
            item(key = "actions") { TitleActions(entry, actions) }
            if (entry.synopsis.isNotEmpty()) item(key = "synopsis") { Synopsis(entry.synopsis) }
            val family = TitleFamily.of(entry)
            if (family.seasons.isNotEmpty()) item(key = "seasons") { Seasons(entry, family) }
            if (family.related.isNotEmpty()) item(key = "related") { Related(family, titles) }
            episodeRows(entry, episodes, actions)
        }
        TitleTopBar(entry, onBack = navigator::back, modifier = Modifier.padding(top = inset))
    }
    }
}

/** The poster, the title, the other title and the metadata line, standing on the banner's foot. */
@Composable
private fun TitleHeader(entry: Entry, modifier: Modifier = Modifier) {
    Row(
        modifier.padding(horizontal = 16.dp).height(120.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        ArtworkImage(
            url = entry.coverUrl,
            plate = null,
            maxPixelSize = 300,
            modifier = Modifier
                .size(80.dp, 120.dp)
                .shared(Shared.Cover, entry.id)
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
            entry.alternateTitle?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            if (entry.metadataLine.isNotEmpty()) {
                Text(
                    entry.metadataLine,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
        }
    }
}

/** Back and More, each a 40 circle on surfaceContainer in a 48 target, over the banner. */
@Composable
private fun TitleTopBar(entry: Entry, onBack: () -> Unit, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    var more by remember { mutableStateOf(false) }
    Row(
        modifier.fillMaxWidth().height(64.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CircleIconButton(Icons.Back, "Back", onBack)
        Spacer(Modifier.weight(1f))
        Box {
            CircleIconButton(Icons.More, "More", { more = true })
            DropdownMenu(expanded = more, onDismissRequest = { more = false }) {
                // The show's own settings sheet comes with A2.
                DropdownMenuItem(
                    text = { Text("Settings for this show") },
                    leadingIcon = { Icon(Icons.Settings, null) },
                    onClick = { more = false },
                    enabled = false,
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
                modifier = Modifier.fillMaxWidth().height(56.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                // What it plays crossfades as an episode is marked (T9).
                Crossfade(action.label, animationSpec = Motion.standard(), label = "action") {
                    Text(it, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
                }
            }
        } else {
            // Still to air: when, in the button's place, never amber.
            Box(
                Modifier.fillMaxWidth().height(56.dp).border(1.dp, colors.outline, RoundedCornerShape(28.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Text(action.label, style = MaterialTheme.typography.titleMedium.tabular(), color = colors.onSurfaceVariant)
            }
        }
        Row(Modifier.padding(top = 4.dp).height(48.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            var open by remember { mutableStateOf(false) }
            Box(Modifier.weight(1f)) {
                FilledTonalButton(onClick = { open = true }, modifier = Modifier.fillMaxWidth()) {
                    Text(entry.listStatus?.label?.replaceFirstChar { it.uppercase() } ?: "Add to list")
                    Spacer(Modifier.width(8.dp))
                    Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
                }
                DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
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

/**
 * The synopsis in two lines, ending "… More" where there is more, which shows the rest; a tap on
 * the whole text toggles it.
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
            .clickable { expanded = !expanded }
            .padding(vertical = 4.dp),
    ) {
        val width = constraints.maxWidth
        val cut = remember(text, width, style) {
            fun fits(candidate: String) =
                !measurer.measure(candidate, style, maxLines = 2, constraints = Constraints(maxWidth = width)).hasVisualOverflow
            if (fits(text)) {
                null
            } else {
                // The longest start that leaves room for the ending, More set a little wider.
                var low = 0
                var high = text.length
                while (low < high) {
                    val mid = (low + high + 1) / 2
                    if (fits(text.take(mid).trimEnd() + "\u2026 More ")) low = mid else high = mid - 1
                }
                text.take(low).trimEnd() + "\u2026"
            }
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

/** The seasons as a connected group: this one selected with its count, the others a tap away. */
@Composable
private fun Seasons(entry: Entry, family: TitleFamily, inset: Boolean = true) {
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.padding(horizontal = if (inset) 16.dp else 0.dp).fillMaxWidth().height(48.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        family.seasons.forEachIndexed { index, season ->
            val first = index == 0
            val last = index == family.seasons.lastIndex
            val shape = when {
                season.isThis -> RoundedCornerShape(20.dp)
                first -> RoundedCornerShape(topStart = 20.dp, bottomStart = 20.dp, topEnd = 8.dp, bottomEnd = 8.dp)
                last -> RoundedCornerShape(topStart = 8.dp, bottomStart = 8.dp, topEnd = 20.dp, bottomEnd = 20.dp)
                else -> RoundedCornerShape(8.dp)
            }
            Row(
                Modifier
                    .weight(if (season.isThis) 1.4f else 1f)
                    .height(40.dp)
                    .clip(shape)
                    .background(if (season.isThis) colors.primaryContainer else colors.surfaceContainer)
                    .clickable(enabled = !season.isThis) { navigator.openTitle(season.id, "season-picker") },
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val content = if (season.isThis) colors.onPrimaryContainer else colors.onSurface
                if (season.isThis) {
                    Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                }
                Text("Season ${index + 1}", style = MaterialTheme.typography.labelLarge, color = content, maxLines = 1)
                val progress = entry.progress
                val total = entry.episodes?.takeIf { it > 0u }
                if (season.isThis && progress != null && total != null) {
                    Spacer(Modifier.width(6.dp))
                    val count = MaterialTheme.typography.labelLarge.tabular().copy(fontWeight = FontWeight.Normal)
                    // Only the changed digit moves as an episode is marked (T9).
                    RollingCount(progress.done.toInt(), count, content.copy(alpha = 0.8f))
                    Text("/$total", style = count, color = content.copy(alpha = 0.8f))
                }
            }
        }
    }
}

/** Films, OVAs and specials, each an outlined chip with where it stands. */
@Composable
private fun Related(family: TitleFamily, titles: Map<AnilistId, Entry>, inset: Boolean = true) {
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.padding(horizontal = if (inset) 16.dp else 0.dp).height(48.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Related", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(end = 4.dp))
        for (related in family.related.take(3)) {
            val known = titles[related.id]
            val state = when {
                known?.progress == null -> related.relation.replaceFirstChar { it.uppercase() }
                known.episodes != null && known.progress!!.done >= known.episodes!! -> "Watched"
                known.progress!!.done == 0u -> "Not started"
                else -> "In progress"
            }
            Row(
                Modifier
                    .height(32.dp)
                    .clip(MaterialTheme.shapes.small)
                    .border(1.dp, colors.outline, MaterialTheme.shapes.small)
                    .clickable { navigator.openTitle(related.id, "related") }
                    .padding(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(formatLabel(related.format) ?: related.title, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                Text(state, style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Normal), color = colors.onSurfaceVariant, maxLines = 1)
            }
        }
    }
}

/** The episodes, a row each; plates in their place while the table loads. */
private fun LazyListScope.episodeRows(entry: Entry, episodes: List<EpisodeRow>?, actions: AppActions) {
    if (episodes == null) {
        items(4, key = { "placeholder-$it" }) { AfterWait { EpisodePlaceholder() } }
        return
    }
    val next = entry.progress?.next ?: 1u
    items(episodes, key = { "episode-${it.number}" }) { row ->
        EpisodeRowView(entry, row, next, actions)
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
private fun nowSeconds() = System.currentTimeMillis() / 1000

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
    entry: Entry,
    episodes: List<EpisodeRow>?,
    art: HeroArt,
    titles: Map<AnilistId, Entry>,
    actions: AppActions,
) {
    val navigator = LocalNavigator.current
    val marks = remember(entry.id) { UserMarks() }
    val inset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val next = entry.progress?.next ?: 1u
    val family = TitleFamily.of(entry)
    val colors = MaterialTheme.colorScheme
    androidx.compose.runtime.CompositionLocalProvider(LocalUserMarks provides marks) {
        Box(Modifier.fillMaxSize().shared(Shared.Page, entry.id).background(colors.surface)) {
            androidx.compose.foundation.lazy.grid.LazyVerticalGrid(
                columns = androidx.compose.foundation.lazy.grid.GridCells.Fixed(2),
                contentPadding = PaddingValues(start = 8.dp, end = 24.dp, bottom = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                fullWidth("head") {
                    Box {
                        if (art != HeroArt.Compact) {
                            ArtworkImage(
                                art.url,
                                plate = null,
                                maxPixelSize = 1600,
                                modifier = Modifier.fillMaxWidth().height(inset + 208.dp).shared(Shared.Art, entry.id),
                            )
                        }
                        Row(
                            Modifier.padding(top = inset + if (art == HeroArt.Compact) 64.dp else 135.dp).padding(start = 16.dp),
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
                                    Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
                                }
                                if (entry.metadataLine.isNotEmpty()) {
                                    Text(entry.metadataLine, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
                                }
                                TabletActions(entry, actions, Modifier.padding(top = 4.dp))
                            }
                            TitleFacts(entry, Modifier.width(376.dp))
                        }
                    }
                }
                if (family.seasons.isNotEmpty() || family.related.isNotEmpty()) {
                    fullWidth("family") {
                        Row(Modifier.padding(start = 16.dp, top = 24.dp), horizontalArrangement = Arrangement.spacedBy(32.dp), verticalAlignment = Alignment.CenterVertically) {
                            if (family.seasons.isNotEmpty()) Box(Modifier.width(520.dp)) { Seasons(entry, family, inset = false) }
                            if (family.related.isNotEmpty()) Related(family, titles, inset = false)
                        }
                    }
                }
                fullWidth("gap") { Spacer(Modifier.height(16.dp)) }
                if (episodes == null) {
                    items(4) { AfterWait { EpisodePlaceholder() } }
                } else {
                    items(episodes.size, key = { "episode-${episodes[it].number}" }) { index ->
                        val row = episodes[index]
                        Box(Modifier.clip(MaterialTheme.shapes.large)) { EpisodeRowView(entry, row, next, actions) }
                    }
                }
            }
            TitleTopBar(entry, onBack = navigator::back, modifier = Modifier.padding(top = inset))
        }
    }
}

private fun androidx.compose.foundation.lazy.grid.LazyGridScope.fullWidth(key: String, content: @Composable () -> Unit) {
    item(key = key, span = { androidx.compose.foundation.lazy.grid.GridItemSpan(maxLineSpan) }) { content() }
}

/** The tablet's actions in one row: the amber action sized to its words, the list, the trailer. */
@Composable
private fun TabletActions(entry: Entry, actions: AppActions, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val action = TitleAction.of(entry)
    var open by remember { mutableStateOf(false) }
    Row(modifier.height(56.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (action.playable) {
            Button(onClick = { actions.play(entry.id, action.episode.toString()) }, modifier = Modifier.height(56.dp), contentPadding = PaddingValues(horizontal = 24.dp)) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Crossfade(action.label, animationSpec = Motion.standard(), label = "action") {
                    Text(it, style = MaterialTheme.typography.titleMedium.tabular(), maxLines = 1)
                }
            }
        } else {
            Text(action.label, style = MaterialTheme.typography.titleMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 8.dp))
        }
        Box {
            FilledTonalButton(onClick = { open = true }, contentPadding = PaddingValues(start = 16.dp, end = 12.dp)) {
                Text(entry.listStatus?.label?.replaceFirstChar { it.uppercase() } ?: "Add to list")
                Spacer(Modifier.width(8.dp))
                Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
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

/** The synopsis, and the studio, the next episode and where else it streams, beside the title. */
@Composable
private fun TitleFacts(entry: Entry, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(modifier) {
        if (entry.synopsis.isNotEmpty()) {
            Text(entry.synopsis, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 4, overflow = TextOverflow.Ellipsis)
        }
        val facts = listOfNotNull(
            entry.studio?.let { "STUDIO" to it },
            entry.nextAiringIn?.let { "NEXT EPISODE" to AirTime.words(java.time.Instant.now().plusSeconds(it)) },
            entry.availableOn.takeIf { it.isNotEmpty() }?.let { "ALSO ON" to it.joinToString(", ") },
        )
        if (facts.isNotEmpty()) {
            Row(Modifier.padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                for ((label, value) in facts) {
                    Column(Modifier.weight(1f)) {
                        Text(label, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant)
                        Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}
