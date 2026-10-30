// The TV's trailer: Play trailer opens a full-screen player with YouTube's own controls. Nothing
// autoplays on the TV. Center plays and pauses through the player API, Back closes.

package tv.anistream.tv.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.delay
import tv.anistream.engine.model.EmbedStyle
import tv.anistream.engine.model.Trailer
import tv.anistream.engine.model.TrailerControl
import tv.anistream.i18n.R
import tv.anistream.store.TrailerPlayer
import tv.anistream.store.TrailerSignal

private const val REFUSED_MILLIS = 8_000L

/** YouTube's "this video can't be embedded" codes: the browser takes it instead. */
private val refusals = setOf(100, 101, 150)

@Composable
fun TrailerScreen(trailer: Trailer, onClose: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var playing by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    var attempt by remember { mutableStateOf(0) }
    val focus = remember { FocusRequester() }
    val player = remember(attempt) {
        failed = false
        playing = false
        TrailerPlayer(context, trailer, EmbedStyle.FULL) { signal ->
            when (signal) {
                TrailerSignal.Playing -> playing = true
                TrailerSignal.Ended -> { playing = false }
                is TrailerSignal.Failed -> {
                    if (signal.code in refusals) {
                        store.openTrailerInBrowser(trailer)
                        onClose()
                    } else {
                        failed = true
                    }
                }
                else -> {}
            }
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    LaunchedEffect(player) {
        delay(REFUSED_MILLIS)
        if (!playing) failed = true
    }
    LaunchedEffect(failed, attempt) { if (!failed) focus.requestFocus() }
    BackHandler(onBack = onClose)

    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnBackPress = false)) {
        Box(
            Modifier
                .fillMaxSize()
                .background(androidx.compose.ui.graphics.Color.Black)
                .onPreviewKeyEvent { event ->
                    when {
                        event.key == Key.Back && event.type == KeyEventType.KeyUp -> { onClose(); true }
                        event.key in setOf(Key.DirectionCenter, Key.Enter) && !failed -> {
                            if (event.type == KeyEventType.KeyUp) {
                                player.control(if (playing) TrailerControl.PAUSE else TrailerControl.PLAY)
                                playing = !playing
                            }
                            true
                        }
                        else -> false
                    }
                },
        ) {
            if (!failed) {
                AndroidView(
                    factory = { player.view.also { it.keepScreenOn = true } },
                    modifier = Modifier.fillMaxSize().focusRequester(focus).focusable(),
                )
            } else {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    StateBlock(
                        line = stringResource(R.string.trailer_failed),
                        action = stringResource(R.string.action_try_again),
                        onAction = { attempt++ },
                    )
                }
            }
        }
    }
}
