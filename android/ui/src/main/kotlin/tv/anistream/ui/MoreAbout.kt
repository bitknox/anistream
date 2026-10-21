// "More about this show": About, Rankings, Cast, Staff, Recommendations and Tags, as a modal
// bottom sheet on a phone (AndroidMoreAbout) and a supporting pane on a tablet
// (AndroidTabletMoreAbout). Every section with no data is left out.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ArtShape
import tv.anistream.design.Motion
import tv.anistream.design.Nagare
import tv.anistream.design.ShowTitle
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.CastRole
import tv.anistream.engine.model.Entry

/** The flat placeholder a related or recommended cover, or a character's art, waits on. */
private val PLATE = Color(0xFF1F2440)

/** The sheet itself on a phone: full height, its top 77 dp down, closed by its handle, the scrim or Back. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MoreAboutSheet(entry: Entry, onDismiss: () -> Unit, onOpenTitle: (AnilistId) -> Unit) {
    val state = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded))
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.padding(top = 77.dp - 24.dp),
    ) {
        MoreAboutContent(entry, about = true, onOpenTitle = onOpenTitle)
    }
}

/**
 * The sections, in order: About (on phones and in the tablet's pane), Rankings, Cast, Staff,
 * Recommendations and Tags. Placeholders while the page's fetch is out, Nagare when AniList has
 * nothing here, and a line with Try again when it did not answer.
 */
@Composable
fun MoreAboutContent(
    entry: Entry,
    about: Boolean,
    onOpenTitle: (AnilistId) -> Unit,
    modifier: Modifier = Modifier,
    /** Whether About heads itself with the title, as the phone's sheet does; the pane has its own header. */
    heading: Boolean = true,
) {
    val store = LocalAppStore.current
    val failed by store.titles.failed.collectAsStateWithLifecycle()
    val detail = entry.detail
    Column(
        modifier.verticalScroll(rememberScrollState()).padding(bottom = 48.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        if (about) About(entry, heading)
        // Placeholders give way to the sections, or to the failure, in one crossfade (T8).
        val phase = when {
            detail != null -> 1
            entry.id in failed -> 2
            else -> 0
        }
        androidx.compose.animation.Crossfade(phase, animationSpec = Motion.standard(), label = "about") { shown ->
        Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
        when (shown) {
            1 -> if (detail != null) {
                val nothing = detail.rankings.isEmpty() && detail.characters.isEmpty() && detail.staff.isEmpty() &&
                    detail.recommendations.isEmpty() && detail.tags.isEmpty()
                if (nothing) {
                    Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Nagare(cell = 4)
                        Text(
                            "AniList has no cast, staff or recommendations for this show yet.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                } else {
                    Rankings(entry)
                    Cast(entry)
                    Staff(entry)
                    Recommendations(entry, onOpenTitle)
                    Tags(entry)
                }
            }
            2 -> Column(Modifier.padding(horizontal = 16.dp)) {
                Text("AniList did not answer.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                TextButton(onClick = { store.loadDetail(entry.id) }, modifier = Modifier.padding(start = 0.dp)) { Text("Try again") }
            }
            else -> AboutPlaceholders()
        }
        }
        }
    }
}

@Composable
private fun SectionTitle(text: String, modifier: Modifier = Modifier) {
    Text(text, style = MaterialTheme.typography.titleMedium, modifier = modifier.semantics { heading() })
}

/** The full synopsis, and the facts in a grid: Studio, Based on, Season, the airing state, Also on. */
@Composable
private fun About(entry: Entry, heading: Boolean) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = if (heading) 16.dp else 24.dp)) {
        if (heading) {
            Text(entry.showTitle, style = ShowTitle.page, modifier = Modifier.semantics { heading() })
            entry.alternateTitle?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp)) }
        }
        val synopsis = synopsisWithoutSpoilers(entry.synopsis)
        if (synopsis.isNotEmpty()) {
            Text(
                synopsis,
                style = if (heading) MaterialTheme.typography.bodyLarge else MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(top = if (heading) 12.dp else 0.dp),
            )
        }
        val airing = entry.airingState()
        val facts = listOfNotNull(
            entry.studio?.let { "STUDIO" to it },
            entry.source?.label()?.let { "BASED ON" to it },
            entry.seasonWords()?.let { "SEASON" to it },
            airing?.let { it.label.uppercase() to it.value },
            entry.alsoOn()?.let { "ALSO ON" to it },
        )
        if (facts.isNotEmpty()) {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (row in facts.chunked(3)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        for ((label, value) in row) {
                            Column(Modifier.weight(1f)) {
                                Text(label, style = MaterialTheme.typography.labelSmall, color = colors.onSurfaceVariant)
                                Text(value, style = MaterialTheme.typography.bodyMedium)
                            }
                        }
                        repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }
    }
}

/** "#4 Highest rated, Summer 2026": the number in silk, the rest in haze, as plain lines. */
@Composable
private fun Rankings(entry: Entry) {
    val rankings = entry.detail?.rankings?.inOrder().orEmpty()
    if (rankings.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp)) {
        SectionTitle("Rankings", Modifier.padding(bottom = 4.dp))
        for (ranking in rankings) {
            Text(
                buildAnnotatedString {
                    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, fontFeatureSettings = "tnum")) { append("#${ranking.rank}") }
                    append(" ")
                    withStyle(SpanStyle(color = colors.onSurfaceVariant)) { append(ranking.words()) }
                },
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

/** Twelve characters in the engine's order, each with their role and first Japanese voice. */
@Composable
private fun Cast(entry: Entry) {
    val cast = entry.detail?.characters?.take(12).orEmpty()
    if (cast.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    Column {
        Row(Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp).fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            SectionTitle("Cast", Modifier.weight(1f))
            Text("Japanese voices", style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(cast) { member ->
                Column(Modifier.width(88.dp)) {
                    Box(Modifier.size(88.dp, 132.dp).clip(ArtShape.cover(88.dp)).background(PLATE)) {
                        member.imageUrl?.let { ArtworkImage(it, plate = PLATE, maxPixelSize = 300, modifier = Modifier.size(88.dp, 132.dp)) }
                    }
                    Text(member.name, style = MaterialTheme.typography.labelLarge, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                    Text(member.role.words(), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                    member.voiceActors.firstOrNull()?.let {
                        Text(it.name, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

private fun CastRole.words(): String = when (this) {
    CastRole.MAIN -> "Main"
    CastRole.SUPPORTING -> "Supporting"
    CastRole.BACKGROUND -> "Background"
}

/** Eight credits in two columns: the name, then the role as AniList writes it. */
@Composable
private fun Staff(entry: Entry) {
    val staff = entry.detail?.staff?.take(8).orEmpty()
    if (staff.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp)) {
        SectionTitle("Staff", Modifier.padding(bottom = 4.dp))
        for (pair in staff.chunked(2)) {
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                for (credit in pair) {
                    Column(Modifier.weight(1f).height(56.dp), verticalArrangement = Arrangement.Center) {
                        Text(credit.name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(credit.role, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

/** Eight recommendations as covers with their titles; one opens that title's page. */
@Composable
private fun Recommendations(entry: Entry, onOpenTitle: (AnilistId) -> Unit) {
    val recommendations = entry.detail?.recommendations?.take(8).orEmpty()
    if (recommendations.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    Column {
        SectionTitle("Recommendations", Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp))
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(recommendations, key = { it.id.toLong() }) { linked ->
                Column(Modifier.width(100.dp).clickable { onOpenTitle(linked.id) }) {
                    ArtworkImage(linked.coverUrl, plate = PLATE, maxPixelSize = 300, modifier = Modifier.size(100.dp, 150.dp).clip(ArtShape.cover(100.dp)))
                    Text(linked.title, style = ShowTitle.caption, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                    Text(linked.recommendationLine(), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1)
                }
            }
        }
    }
}

/**
 * Every tag without spoilers by rank, then the reveal: a plain text button that lists the spoiler
 * tags on their own line until the sheet closes.
 */
@Composable
private fun Tags(entry: Entry) {
    val tags = entry.detail?.tags.orEmpty()
    val plain = tags.plain()
    val spoilers = tags.spoilers()
    if (plain.isEmpty() && spoilers.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    var shown by remember(entry.id) { mutableStateOf(false) }
    Column(Modifier.padding(horizontal = 16.dp)) {
        SectionTitle("Tags", Modifier.padding(bottom = 4.dp))
        if (plain.isNotEmpty()) Text(plain.joinToString(" · ") { it.name }, style = MaterialTheme.typography.bodyMedium)
        if (spoilers.isNotEmpty()) {
            if (shown) {
                Text(
                    buildAnnotatedString {
                        spoilers.forEachIndexed { index, tag ->
                            if (index > 0) append(" · ")
                            append(tag.name)
                            append(" ")
                            withStyle(SpanStyle(color = colors.onSurfaceVariant, fontSize = MaterialTheme.typography.bodySmall.fontSize)) { append("Spoiler") }
                        }
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            val count = spoilers.size
            val label = when {
                shown && count == 1 -> "Hide spoiler tag"
                shown -> "Hide spoiler tags"
                count == 1 -> "Show 1 spoiler tag"
                else -> "Show $count spoiler tags"
            }
            TextButton(
                onClick = { shown = !shown },
                colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                modifier = Modifier.padding(top = 4.dp).semantics { stateDescription = if (shown) "Expanded" else "Collapsed" },
            ) { Text(label) }
        }
    }
}

/** While the page's fetch is out: two lines, four art plates, four rows and four covers. */
@Composable
private fun AboutPlaceholders() {
    val colors = MaterialTheme.colorScheme
    AfterWait {
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(24.dp)) {
            Column {
                TextPlaceholder(0.6f, 20.dp)
                TextPlaceholder(0.5f, 20.dp)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(4) { Box(Modifier.size(88.dp, 132.dp).clip(ArtShape.cover(88.dp)).background(colors.surfaceContainerHigh)) }
            }
            Column { repeat(4) { TextPlaceholder(0.7f, 28.dp) } }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(4) { Box(Modifier.size(100.dp, 150.dp).clip(ArtShape.cover(100.dp)).background(colors.surfaceContainerHigh)) }
            }
        }
    }
}
