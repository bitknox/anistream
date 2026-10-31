// The poster pieces several screens share: a poster with its caption, its placeholder, and the
// quiet line a list leaves when it has nothing else to say.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import tv.anistream.design.ArtShape
import tv.anistream.design.ShowTitle
import tv.anistream.engine.model.Entry
import tv.anistream.i18n.R

/** The legal line, everywhere it appears. */
const val LEGAL_LINE = "anistream hosts no content."

/** A plain line of haze text where a screen has nothing else to say. */
@Composable
internal fun Quiet(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/**
 * Folding to one pane with a title picked shows that title's page (AT3); its page goes back to
 * the list and detail when the window widens again.
 */
@Composable
internal fun FoldFollowsSelection(wide: Boolean, state: QueueState) {
    val navigator = LocalNavigator.current
    var was by remember { mutableStateOf(wide) }
    LaunchedEffect(wide) {
        val id = state.selected?.toUInt()
        if (was && !wide && state.picked && id != null) {
            navigator.openedByFold = id
            navigator.openTitle(id, "fold")
        }
        was = wide
    }
}

/**
 * A poster with its title in two lines beneath, and a detail line: the first genre unless [detail]
 * says otherwise, with a water dot when [fresh]. Opens the title, flying from [from].
 */
@Composable
internal fun PosterCell(
    entry: Entry,
    modifier: Modifier = Modifier,
    from: String = "poster",
    detail: String? = entry.genreLine,
    fresh: Boolean = false,
    /** Said as the title opens, before the page goes up. */
    onOpen: (() -> Unit)? = null,
) {
    val navigator = LocalNavigator.current
    Column(
        modifier
            .shared(Shared.Page, entry.id, from)
            .clickable(onClickLabel = stringResource(R.string.title_open)) {
                onOpen?.invoke()
                navigator.openTitle(entry.id, from)
            },
    ) {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            CoverArt(entry, maxWidth, Modifier.size(maxWidth, maxWidth * 1.5f).shared(Shared.Cover, entry.id, from))
        }
        Text(
            entry.showTitle,
            style = ShowTitle.caption,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        detail?.let {
            Row(
                Modifier.padding(top = 2.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (fresh) Box(Modifier.size(6.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** A poster's place before its title arrives: the plate, and its caption's lines as bars. */
@Composable
internal fun PosterPlaceholder() {
    Column {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            tv.anistream.design.CoverPlate(null, Modifier.size(maxWidth, maxWidth * 1.5f).clip(ArtShape.cover(maxWidth)))
        }
        TextPlaceholder(0.8f, 20.dp, Modifier.padding(top = 8.dp))
        TextPlaceholder(0.5f, 16.dp, Modifier.padding(top = 2.dp))
    }
}
