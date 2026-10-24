// The column beside a tablet's pane: the effect of the row tapped last where there is one, the
// upscaling modes on one drawing or three titles named the chosen way, and otherwise what the pane
// is about (AndroidTabletSettings).

package tv.anistream.ui

import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.ShowTitle
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.SettingsCatalog

/** The column for [pane], about the row [focus] names when it has an effect to show. */
@Composable
internal fun SettingsAside(pane: SettingsPane, focus: String?) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    Column(
        Modifier
            .fillMaxWidth()
            .clip(MaterialTheme.shapes.large)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        when (pane) {
            SettingsPane.Playback -> if (focus == FOCUS_TITLES) TitleLanguagePreview() else UpscalingPreview()
            SettingsPane.Notifications -> Aside(
                "New episodes",
                "One notification when an episode of a show you follow comes out: the show and the episode's number, never its name. A show can turn its own off in its settings.",
            )
            SettingsPane.Downloads -> Aside(
                "Downloads",
                "Downloads stay in anistream's own space on this device, which other apps cannot read. Removing the app removes them.",
            )
            SettingsPane.Sources -> if (store.settings.isOn(SettingKey.TORRENTS, view)) {
                Aside("Connection check", checkNote(view?.torrent?.onFailure, view?.torrent?.protection))
            } else {
                Aside("Sources", "Sources are tried in order, top to bottom. A show can keep its own source, chosen from its title page.")
            }
            SettingsPane.Accounts -> Aside("Accounts", "anistream works without an account. Your watch history stays on this device when you sign out.")
            SettingsPane.About -> Aside("anistream", LEGAL_LINE)
        }
    }
}

@Composable
private fun AsideTitle(title: String) {
    Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
}

@Composable
private fun Aside(title: String, text: String) {
    AsideTitle(title)
    Text(text, style = MaterialTheme.typography.bodyMedium)
}

/**
 * The upscaling modes on one drawing, anistream's own: the left half as the 480p source, the
 * right half in the mode chosen, a silk line between them, and a chip naming each.
 */
@Composable
private fun UpscalingPreview() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val mode = SettingsCatalog.current(store.settings.row(SettingKey.UPSCALING, view))?.label ?: "natural · ArtCNN"
    AsideTitle("Upscaling")
    BoxWithConstraints(
        Modifier
            .fillMaxWidth()
            .aspectRatio(16f / 9f)
            .clip(RoundedCornerShape(8.dp))
            .semantics { contentDescription = "The same still, the left half as the 480p source, the right half with ${sentence(mode)}" },
    ) {
        val half = maxWidth / 2
        Drawing(mode, Modifier.fillMaxSize())
        // The same drawing at the same size, only its left half shown.
        Drawing("off", Modifier.fillMaxSize().drawWithContent { clipRect(right = size.width / 2) { this@drawWithContent.drawContent() } })
        Box(Modifier.offset(x = half - 1.dp).width(2.dp).fillMaxHeight().background(MaterialTheme.colorScheme.onSurface))
        Chip("480p source", Modifier.align(Alignment.BottomStart).padding(8.dp))
        Chip(sentence(mode), Modifier.align(Alignment.BottomEnd).padding(8.dp))
    }
    Text(upscalingNote(mode), style = MaterialTheme.typography.bodyMedium)
}

/** The drawing as [mode] shows it: softened for the source, a little more contrast for sharp. */
@Composable
private fun Drawing(mode: String, modifier: Modifier) {
    val painter = painterResource(tv.anistream.design.R.drawable.upscaling_preview)
    when (mode) {
        "off" -> Image(painter, null, modifier.blur(1.6.dp), contentScale = ContentScale.Crop, colorFilter = ColorFilter.colorMatrix(tone(0.85f, 0.92f)))
        "sharp · Anime4K" -> Image(painter, null, modifier, contentScale = ContentScale.Crop, colorFilter = ColorFilter.colorMatrix(tone(1.08f, 1.12f)))
        else -> Image(painter, null, modifier, contentScale = ContentScale.Crop)
    }
}

/** A colour matrix for [saturation] and [contrast], as the board's CSS filters set them. */
private fun tone(saturation: Float, contrast: Float): ColorMatrix {
    val matrix = ColorMatrix().apply { setToSaturation(saturation) }
    val offset = 128f * (1 - contrast)
    val scale = ColorMatrix(
        floatArrayOf(
            contrast, 0f, 0f, 0f, offset,
            0f, contrast, 0f, 0f, offset,
            0f, 0f, contrast, 0f, offset,
            0f, 0f, 0f, 1f, 0f,
        ),
    )
    matrix.timesAssign(scale)
    return matrix
}

/** A label over the drawing, as the board has it: labelSmall silk, 24 high, on the ground at 80 %. */
@Composable
private fun Chip(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = modifier
            .clip(RoundedCornerShape(4.dp))
            .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.8f))
            .heightIn(min = 24.dp)
            .wrapContentHeight(Alignment.CenterVertically)
            .padding(horizontal = 8.dp),
    )
}

/** What each upscaling mode does, under the drawing. */
internal fun upscalingNote(mode: String): String = when (mode) {
    "off" -> "The 480p source, scaled to fit without processing."
    "sharp · Anime4K" -> "Crisper lines and stronger edges, at some cost to texture."
    else -> "Faithful line work and texture, nothing restyled."
}

/** Three covers with their titles as the current choice names them, and what the choice does. */
@Composable
private fun TitleLanguagePreview() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val viewer by store.accounts.viewerTitleLanguage.collectAsStateWithLifecycle()
    val signedIn = anilistSignedIn()
    val choice = TitleChoice.of(SettingsCatalog.current(store.settings.row(SettingKey.TITLES, view))?.value)
    val examples = titleLanguageExamples()
    LaunchedEffect(Unit) { store.loadContinue() }
    AsideTitle("Title language")
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        for (entry in examples) {
            Column(Modifier.width(88.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                CoverArt(entry, 88.dp, Modifier.size(88.dp, 132.dp))
                Text(
                    TitleLanguageWords.example(entry, choice, signedIn, viewer),
                    style = ShowTitle.caption,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
    Text(TitleLanguageWords.note(choice, signedIn, viewer, examples), style = MaterialTheme.typography.bodyMedium)
}
