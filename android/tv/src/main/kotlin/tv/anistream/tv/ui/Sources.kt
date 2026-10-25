// An episode's releases to choose from, in the 400-wide column on the right: from an episode's
// long press on the title page, and from the player when a source is slow to start.

package tv.anistream.tv.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.SourceCandidate

/** "1080p · 1.2 GiB · Dual audio · Automatic": what tells one release from another. */
fun releaseLine(candidate: SourceCandidate): String? = listOfNotNull(
    candidate.quality?.let { "${it}p" },
    candidate.size,
    candidate.seeders?.let { "$it seeders" },
    "Dub".takeIf { candidate.dubbed },
    "Dual audio".takeIf { candidate.dualAudio },
    "Automatic".takeIf { candidate.autoPick },
).joinToString(" · ").ifEmpty { null }

/**
 * The releases, the first focused once they come. While they are found, and when there are none,
 * the line saying so holds the focus, so the D-pad stays in the panel.
 */
@Composable
fun ReleaseList(choices: List<SourceCandidate>?, onPick: (SourceCandidate) -> Unit, modifier: Modifier = Modifier) {
    val first = remember { FocusRequester() }
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        when {
            choices == null -> item(key = "finding") { ReleaseNote("Finding releases", Modifier.focusRequester(first)) }
            choices.isEmpty() -> item(key = "none") { ReleaseNote("No releases found for this episode.", Modifier.focusRequester(first)) }
            else -> itemsIndexed(choices, key = { _, it -> "${it.providerId}/${it.id}" }) { index, candidate ->
                TvListRow(
                    candidate.title,
                    onClick = { onPick(candidate) },
                    supporting = releaseLine(candidate),
                    modifier = if (index == 0) Modifier.focusRequester(first) else Modifier,
                )
            }
        }
    }
    LaunchedEffect(choices == null, choices?.isEmpty()) { first.focusSoon() }
}

@Composable
private fun ReleaseNote(text: String, modifier: Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Haze,
        modifier = modifier.focusable().padding(16.dp),
    )
}

/** The player's Choose source: the releases in their own panel, which Back closes. */
@Composable
fun ReleasesPanel(choices: List<SourceCandidate>?, onPick: (SourceCandidate) -> Unit, onClose: () -> Unit, modifier: Modifier = Modifier) {
    BackHandler(onBack = onClose)
    Column(
        modifier
            .width(400.dp)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(start = 16.dp, end = PANEL_END, top = 27.dp, bottom = 27.dp),
    ) {
        Text("Choose a source", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 12.dp).semantics { heading() })
        ReleaseList(choices, onPick)
    }
}
