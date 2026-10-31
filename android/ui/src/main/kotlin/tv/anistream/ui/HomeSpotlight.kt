// Home's spotlight: the one title at the top, with the one amber action. Art above the text on a
// phone, art on the right at 56% of the width on a wide window. Text never sits on the art.

package tv.anistream.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ArtShape
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.model.HomeAction
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.Playable
import tv.anistream.engine.model.Spotlight
import tv.anistream.engine.model.SpotlightCase
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.text

/** AniList's `~!spoiler!~` spans as bars that open on a tap, and the rest as it is. */
@Composable
private fun synopsisText(text: String, id: tv.anistream.engine.core.AnilistId): AnnotatedString {
    val colors = MaterialTheme.colorScheme
    var opened by rememberSaveable(id) { mutableStateOf(setOf<Int>()) }
    val flat = text.replace(Regex("[^\\S\\n]+"), " ").replace(Regex(" ?\\n ?"), "\n").replace(Regex("\\n{2,}"), "\n").trim()
    return buildAnnotatedString {
        var at = 0
        var index = 0
        for (m in Regex("~!(.*?)!~", RegexOption.DOT_MATCHES_ALL).findAll(flat)) {
            append(flat.substring(at, m.range.first))
            val key = index++
            if (key in opened) {
                append(m.groupValues[1])
            } else {
                // A bar the length of its words: nothing reflows when it opens.
                withLink(
                    LinkAnnotation.Clickable(
                        "spoiler-$key", // i18n-skip
                        TextLinkStyles(SpanStyle(color = Color.Transparent, background = colors.surfaceContainerHighest)),
                    ) { opened = opened + key },
                ) { append(m.groupValues[1]) }
            }
            at = m.range.last + 1
        }
        append(flat.substring(at))
    }
}

@Composable
internal fun SpotlightBlock(spot: Spotlight, h: HomeHandlers) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val entry = spot.entry
    val id = entry.id
    // The sources check: it decides between Play and Add to Planning, and only when asked.
    LaunchedEffect(id) { if (spot.playable == Playable.UNKNOWN && spot.action != HomeAction.NOTHING) store.probe(id) }
    val checks by store.home.playable.collectAsStateWithLifecycle()
    val notFound = (checks[id] ?: spot.playable) == Playable.NOT_FOUND
    var planned by rememberSaveable(id) { mutableStateOf(false) }
    val action = when {
        planned -> HomeAction.NOTHING
        notFound && spot.action != HomeAction.NOTHING -> HomeAction.ADD_TO_PLANNING
        else -> spot.action
    }
    val side = LocalConfiguration.current.screenWidthDp >= 700
    val line = when {
        notFound -> Strings.homeNotFound(context)
        else -> spot.line?.text(context) ?: spot.starts?.words()?.let { Strings.upNextStarts(context, it) }
    }
    val art: @Composable (Modifier) -> Unit = { m ->
        BannerArt(entry, m.aspectRatio(16f / 9f).shared(Shared.Art, id, "home-spotlight").shared(Shared.Player, id).clip(ArtShape.banner))
    }
    val text: @Composable (Modifier) -> Unit = { m ->
        Column(m, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(spot.label.text(context), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
            Text(entry.showTitle, style = ShowTitle.page, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (line != null) Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
            spot.resume?.fraction?.let { SilkMeter(it, Modifier.padding(vertical = 4.dp)) }
            if (entry.synopsis.isNotBlank()) {
                Text(
                    synopsisText(entry.synopsis, id),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = if (side) 5 else 3,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            SpotlightActions(spot, action, side, h, onPlanned = { planned = true }, Modifier.padding(top = 12.dp))
        }
    }
    val description = listOfNotNull(spot.label.text(context), entry.showTitle, line).joinToString(", ")
    if (side) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp).semantics(mergeDescendants = false) { contentDescription = description },
            horizontalArrangement = Arrangement.spacedBy(24.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            text(Modifier.weight(0.44f))
            art(Modifier.weight(0.56f))
        }
    } else {
        Column(Modifier.padding(horizontal = 16.dp).semantics(mergeDescendants = false) { contentDescription = description }) {
            art(Modifier.fillMaxWidth())
            text(Modifier.padding(top = 12.dp))
        }
    }
}

@Composable
private fun SpotlightActions(spot: Spotlight, action: HomeAction, side: Boolean, h: HomeHandlers, onPlanned: () -> Unit, modifier: Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val snackbars = LocalSnackbars.current
    val colors = MaterialTheme.colorScheme
    val entry = spot.entry
    val id = entry.id
    var menu by remember { mutableStateOf(false) }
    var sheet by rememberSaveable(id) { mutableStateOf(false) }
    val episode = spot.episode.toInt()
    val full = if (side) Modifier else Modifier.fillMaxWidth()
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        when (action) {
            HomeAction.RESUME, HomeAction.PLAY -> Button(
                onClick = { h.actions.play(id, spot.episode.toString()) },
                modifier = full.heightIn(min = 56.dp),
            ) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Text(
                    if (action == HomeAction.RESUME) Strings.homeResumeEpisode(context, episode) else Strings.homePlayEpisode(context, episode),
                    style = MaterialTheme.typography.titleMedium.tabular(),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            HomeAction.ADD_TO_PLANNING -> Button(
                onClick = {
                    onPlanned()
                    store.setStatus(id, LibrarySegment.PLANNING)
                },
                modifier = full.heightIn(min = 56.dp),
            ) {
                Icon(Icons.Plus, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(8.dp))
                Text(Strings.upNextAddPlanning(context), style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            HomeAction.NOTHING -> Row(Modifier.heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Check, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
                Text(Strings.homeSpotlightPlanning(context), style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            FilledTonalButton(onClick = { navigator.openTitle(id, "home-spotlight") }, modifier = Modifier.heightIn(min = 48.dp)) {
                Text(Strings.homeDetails(context))
            }
            if (entry.trailerUrl != null) {
                if (sheet) entry.trailer?.let { TrailerSheet(entry, it) { sheet = false } }
                OutlinedButton(onClick = { playTrailer(store, entry) { sheet = true } }, modifier = Modifier.heightIn(min = 48.dp)) {
                    Icon(Icons.Trailer, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.title_trailer))
                }
            }
            Box {
                IconButton(onClick = { menu = true }, modifier = Modifier.size(48.dp)) {
                    Icon(Icons.More, contentDescription = Strings.homeMore(context))
                }
                Menu(expanded = menu, onDismissRequest = { menu = false }) {
                    if (spot.case == SpotlightCase.CONTINUING) {
                        DropdownMenuItem(
                            text = { Text(Strings.homeMarkWatched(context, episode)) },
                            leadingIcon = { Icon(Icons.Check, null) },
                            onClick = {
                                menu = false
                                store.setWatched(id, listOf(spot.episode.toString()))
                                snackbars.show(Strings.homeMarkedWatched(context, episode), context.getString(R.string.action_undo)) {
                                    store.setWatched(id, listOf(spot.episode.toString()), watched = false)
                                }
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(Strings.homeRemoveContinue(context)) },
                            leadingIcon = { Icon(Icons.HideRow, null) },
                            onClick = { menu = false; h.remove(id) },
                        )
                    } else {
                        DropdownMenuItem(
                            text = { Text(Strings.upNextNotInterested(context)) },
                            leadingIcon = { Icon(Icons.HideRow, null) },
                            onClick = { menu = false; h.notInterested(entry, spot.starts != null) },
                        )
                    }
                }
            }
        }
    }
}
