// The states every TV screen shares: the offline line, an empty answer with Nagare, a failure with
// its reason in plain words and Try again, the notice in the top right, and Remove animations.

package tv.anistream.tv.ui

import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.snap
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.design.Motion
import tv.anistream.design.Nagare
import tv.anistream.design.ObiSilk

/** What the offline line says: the TV keeps what it showed, and syncs once it is back. */
const val OFFLINE_LINE = "Offline. Changes sync once the network is back."

/** Whether the engine says the TV is offline, its one offline state. */
@Composable
fun isOffline(): Boolean = LocalAppStore.current.offline.collectAsStateWithLifecycle().value

/** The offline line: a haze dot and the words, quiet, never a coloured banner. */
@Composable
fun OfflineLine(modifier: Modifier = Modifier) {
    Row(
        modifier.semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(Modifier.size(6.dp).background(ObiSilk.Haze, CircleShape))
        Text(OFFLINE_LINE, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
}

/**
 * A screen's state where its content would be: Nagare (cell 6) above an empty answer, then the
 * line (headlineSmall), what it means in haze, and the one way forward as a tonal button.
 * Failures carry no Nagare.
 */
@Composable
fun StateBlock(
    line: String,
    modifier: Modifier = Modifier,
    nagare: Boolean = false,
    detail: String? = null,
    action: String? = null,
    onAction: () -> Unit = {},
    actionModifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        if (nagare) {
            Nagare(cell = 6)
            Spacer(Modifier.height(24.dp))
        }
        Text(line, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, textAlign = TextAlign.Center)
        if (detail != null) {
            Spacer(Modifier.height(8.dp))
            Text(
                detail,
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(max = 560.dp),
            )
        }
        if (action != null) {
            Spacer(Modifier.height(20.dp))
            TonalButton(onClick = onAction, modifier = actionModifier) { ButtonLabel(action) }
        }
    }
}

/** How long a notice stays: 4 s, plus 1 s for every 10 words past 12. */
fun noticeMillis(text: String): Long {
    val words = text.split(Regex("\\s+")).count { it.isNotBlank() }
    val past = (words - NOTICE_WORDS).coerceAtLeast(0)
    return NOTICE_BASE_MILLIS + 1_000L * ((past + 9) / 10)
}

private const val NOTICE_WORDS = 12
private const val NOTICE_BASE_MILLIS = 4_000L

/** One notice on screen, numbered so the same words twice still replace each other. */
data class ShownNotice(val id: Int, val text: String)

/**
 * The TV's one notice surface, top right: engine notices, the player's alerts and the remote's
 * lines all go through it. One at a time; a newer one replaces the shown one.
 */
@Stable
class TvNotices(private val scope: CoroutineScope) {
    var shown by mutableStateOf<ShownNotice?>(null)
        private set

    private var count by mutableIntStateOf(0)
    private var job: Job? = null

    fun say(text: String) {
        count += 1
        val notice = ShownNotice(count, text)
        shown = notice
        job?.cancel()
        job = scope.launch {
            delay(noticeMillis(text))
            if (shown == notice) shown = null
        }
    }
}

/** The root's notices, for screens that say something: "Signed in to AniList as Mika". */
val LocalNotices = staticCompositionLocalOf<TvNotices?> { null }

/**
 * The notice as the brief draws it: right edge at x 912, top at y 27, at most 360 wide, #2A2F45
 * with 12 corners, bodyMedium silk. It fades in and out, and a newer one crossfades. Never
 * focusable, and said by TalkBack as it comes.
 */
@Composable
fun NoticeView(shown: ShownNotice?, modifier: Modifier = Modifier) {
    var last by remember { mutableStateOf(shown) }
    if (shown != null) last = shown
    AnimatedVisibility(shown != null, modifier, enter = enterFade(), exit = exitFade()) {
        Crossfade(last, animationSpec = if (LocalReducedMotion.current) snap() else Motion.fadeIn(), label = "notice") { notice -> // i18n-skip
            Box(
                Modifier
                    .padding(top = Canvas.SafeY, end = Canvas.SafeX)
                    .widthIn(max = 360.dp)
                    .background(ObiSilk.RaisedHigh, RoundedCornerShape(12.dp))
                    .padding(horizontal = 16.dp, vertical = 12.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
            ) {
                Text(notice?.text.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            }
        }
    }
}

/** Remove animations is on: timed motion lands at once, and nothing turns. */
val LocalReducedMotion = staticCompositionLocalOf { false }

/** Whether the system's animator duration scale is 0, read once as the root composes. */
@Composable
fun rememberReducedMotion(): Boolean {
    val context = LocalContext.current
    return remember {
        runCatching { Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }
            .getOrDefault(1f) == 0f
    }
}

/** Fade in on the fade-in token, or at once with Remove animations. */
@Composable
fun enterFade(): EnterTransition = if (LocalReducedMotion.current) EnterTransition.None else fadeIn(Motion.fadeIn())

/** Fade out on the fade-out token, or at once with Remove animations. */
@Composable
fun exitFade(): ExitTransition = if (LocalReducedMotion.current) ExitTransition.None else fadeOut(Motion.fadeOut())
