// Play trailer (trailers board, "Play trailer window and sheet"): the trailer in a sheet with
// YouTube's own controls and sound on. The title's art stands in until the first frame; a trailer
// that does not start in 8 s says so with Try again, and one the player refuses opens the browser.

package tv.anistream.ui

import tv.anistream.engine.ffi.trailerEmbeddable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.delay
import tv.anistream.design.Motion
import tv.anistream.design.ShowTitle
import tv.anistream.engine.model.EmbedStyle
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Trailer
import tv.anistream.i18n.R
import tv.anistream.store.HeroArt
import tv.anistream.store.TrailerPlayer
import tv.anistream.store.TrailerSignal

/** YouTube's codes for a video that is gone or whose owner turned embedding off: the browser has it. */
private val REFUSED = setOf(100, 101, 150)

private const val NOTHING_PLAYS_MILLIS = 8_000L
private const val SPINNER_AFTER_MILLIS = 600L

/** Whether Play trailer opens the sheet: the app's player plays YouTube's trailers, the browser the rest. */
internal fun Entry.playsInSheet(): Boolean = trailer?.let { trailerEmbeddable(it) } == true

/** What the Trailer button does: the sheet where it can, else the browser. */
internal fun playTrailer(store: tv.anistream.store.AppStore, entry: Entry, openSheet: () -> Unit) {
    when {
        entry.playsInSheet() -> openSheet()
        entry.trailer != null -> store.openTrailerInBrowser(entry.trailer!!)
        else -> entry.trailerUrl?.let(store::openExternal)
    }
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun TrailerSheet(entry: Entry, trailer: Trailer, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var attempt by remember { mutableIntStateOf(0) }
    var failed by remember { mutableStateOf(false) }
    var playing by remember(attempt) { mutableStateOf(false) }
    var spinner by remember(attempt) { mutableStateOf(false) }
    val refuse = {
        store.openTrailerInBrowser(trailer)
        onDismiss()
    }
    LaunchedEffect(attempt) {
        delay(SPINNER_AFTER_MILLIS)
        if (!playing) spinner = true
        delay(NOTHING_PLAYS_MILLIS - SPINNER_AFTER_MILLIS)
        if (!playing) failed = true
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        scrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding()) {
            Box(
                Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(MaterialTheme.colorScheme.surface),
            ) {
                key(attempt) {
                    val player = remember {
                        TrailerPlayer(context, trailer, EmbedStyle.FULL) { signal ->
                            when (signal) {
                                TrailerSignal.Playing -> {
                                    playing = true
                                    failed = false
                                }
                                is TrailerSignal.Failed -> if (signal.code in REFUSED) refuse() else failed = true
                                else -> {}
                            }
                        }
                    }
                    DisposableEffect(player) { onDispose { player.release() } }
                    AndroidView(factory = { player.view }, modifier = Modifier.fillMaxSize())
                }
                // The title's art until the first frame, then gone over 200 ms.
                val art by animateFloatAsState(if (playing) 0f else 1f, Motion.fadeIn(), label = "sheet art") // i18n-skip
                if (art > 0f) {
                    val hero = HeroArt.of(entry, null)
                    Box(Modifier.fillMaxSize().alpha(art).background(MaterialTheme.colorScheme.surface)) {
                        if (hero != HeroArt.Compact) ArtworkImage(url = hero.url, plate = null, maxPixelSize = pixels(412.dp), modifier = Modifier.fillMaxSize())
                    }
                }
                if (spinner && !playing && !failed) CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp))
                if (failed && !playing) {
                    Column(
                        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Text(stringResource(R.string.trailer_failed), style = MaterialTheme.typography.titleMedium)
                        TextButton(onClick = {
                            failed = false
                            attempt += 1
                        }) { Text(stringResource(R.string.action_try_again)) }
                    }
                }
            }
            Column(Modifier.padding(horizontal = 16.dp).padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    entry.showTitle,
                    style = ShowTitle.page,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.semantics { heading() },
                )
                Text(stringResource(R.string.title_trailer), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            TextButton(onClick = onDismiss, modifier = Modifier.align(Alignment.End).padding(horizontal = 8.dp, vertical = 8.dp)) {
                Text(stringResource(R.string.action_close))
            }
        }
    }
}
