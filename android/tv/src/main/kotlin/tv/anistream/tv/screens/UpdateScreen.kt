// TVUpdate: the newer release, from Settings › About or its notice. The version, its size and
// what changed; then one place below that says where the update stands, its line, bar and buttons
// each in the same place in every state so nothing moves: Update, the download's bar with Cancel,
// Android's installer, or why it stopped with Try again. The first press with "Install unknown apps" off shows the one screen
// that explains the switch and opens Android's page for it.

package tv.anistream.tv.screens

import tv.anistream.store.percentText
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState
import tv.anistream.store.Wording
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.focusSoon
import tv.anistream.tv.ui.isOffline
import tv.anistream.tv.ui.remembered


/** What the update's place says, in words, for the state it is in. */
object UpdateWords {
    /** What changed, as the release's notes begin: their text without Markdown's marks or blank lines. */
    fun notes(raw: String): String = raw.lineSequence()
        .map { line ->
            line.trim()
                .replace(Regex("^#+\\s*"), "")
                .replace(Regex("^[-*+]\\s+"), "· ")
                .replace(Regex("\\*\\*|__|`"), "")
                .replace(Regex("\\[([^\\]]*)]\\([^)]*\\)"), "$1")
        }
        .filter { it.isNotEmpty() }
        .joinToString("\n")

    /** The line under the version: its size and what this TV runs now. */
    fun about(context: Context, offer: UpdateOffer, installed: String?): String = listOfNotNull(
        offer.bytes.takeIf { it > 0 }?.let { Wording.bytes(context, it) },
        installed?.let { Strings.tvUpdateInstalled(context, it.removePrefix("v")) },
    ).joinToString(" · ") // i18n-skip

    /** "Downloading · 34 % · 16 MB of 48 MB", or "Starting the download" before the first byte. */
    fun downloading(context: Context, state: UpdateState.Downloading): String {
        if (state.done == 0L) return Strings.updatesStartingDownload(context)
        val fraction = state.fraction ?: return "${Strings.downloadsCaptionRunningPlain(context)} · ${Wording.bytes(context, state.done)}" // i18n-skip
        return Strings.updatesDownloadingFull(context, percentText((fraction * 100).toInt()), Wording.bytes(context, state.done), Wording.bytes(context, state.total))
    }
}

@Composable
fun UpdateScreen(memory: FocusMemory, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val state by store.updates.state.collectAsStateWithLifecycle()
    val offline = isOffline()
    val context = LocalContext.current
    val installed = remember {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
    }
    BackHandler {
        if (state is UpdateState.Allowing) store.updates.notNow() else onDone()
    }
    val current = state
    if (current is UpdateState.Allowing) {
        AllowInstalls(current, memory, onOpen = store.updates::openInstallSettings, onNotNow = store.updates::notNow)
        return
    }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        val offer = current.offer
        if (offer == null) {
            Text(stringResource(R.string.updates_up_to_date), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            installed?.let { Text(stringResource(R.string.tv_update_installed, it.removePrefix("v")), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze) }
            Spacer(Modifier.height(8.dp))
            TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/back")) { ButtonLabel(stringResource(R.string.action_back)) }
            EnterFocus(memory, entry = "none")
            return@Column
        }
        Text(stringResource(R.string.updates_update_to, offer.name), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(UpdateWords.about(context, offer, installed), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        // What changed, as the release's notes begin: four lines at most, at most 60 characters
        // wide, so the place below always sits where it did.
        Text(
            UpdateWords.notes(offer.notes).ifBlank { stringResource(R.string.updates_notes_on_page) },
            style = MaterialTheme.typography.bodyLarge,
            color = ObiSilk.Silk,
            maxLines = 4,
            minLines = 4,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 560.dp),
        )
        Spacer(Modifier.height(8.dp))
        Where(current, offline, memory, onDone)
    }
}

/** The place under the notes: its line, bar and buttons in the same places whatever it says, so focus and text never jump. */
@Composable
private fun Where(state: UpdateState, offline: Boolean, memory: FocusMemory, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val updates = store.updates
    val installs = state.offer?.let(updates::installs) ?: false
    val primary = remember { FocusRequester() }
    // A new state has a new first button: it takes focus, where the last one went away.
    LaunchedEffect(state::class) { primary.focusSoon() }
    val cancel = @Composable {
        TonalButton(onClick = updates::cancel, modifier = Modifier.focusRequester(primary).remembered(memory, "update/cancel")) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
    Column(Modifier.widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        when (state) {
            is UpdateState.Available, is UpdateState.None, is UpdateState.Allowing -> {
                when {
                    !installs -> StateLine(stringResource(R.string.updates_no_self_update))
                    offline -> StateLine(stringResource(R.string.accounts_offline_changes_tv), Mark.Offline)
                    else -> StateLine(stringResource(R.string.updates_android_asks))
                }
                Bar(null)
                Buttons {
                    AmberButton(
                        onClick = updates::press,
                        icon = null,
                        enabled = installs && !offline,
                        modifier = Modifier.focusRequester(primary).remembered(memory, "update/press"),
                    ) { ButtonLabel(stringResource(R.string.action_update)) }
                    TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/later")) { ButtonLabel(stringResource(R.string.action_not_now)) }
                }
            }
            is UpdateState.Downloading -> {
                StateLine(UpdateWords.downloading(context, state))
                Bar(state.fraction ?: 0f)
                Buttons { cancel() }
            }
            is UpdateState.Verifying -> {
                StateLine(stringResource(R.string.updates_checking_download_short))
                Bar(1f)
                Buttons { cancel() }
            }
            is UpdateState.Installing -> {
                StateLine(stringResource(R.string.updates_confirm_on_android))
                Bar(1f)
                // Nothing here can stop Android's installer, so no Cancel; its place stays, a tonal
                // button high, so nothing above moves.
                Spacer(Modifier.height(48.dp))
            }
            is UpdateState.Done -> {
                StateLine(stringResource(R.string.updates_updated_tv, state.offer.name))
                Bar(null)
                Buttons {
                    TonalButton(onClick = onDone, modifier = Modifier.focusRequester(primary).remembered(memory, "update/done")) { ButtonLabel(stringResource(R.string.action_done)) }
                }
            }
            is UpdateState.Failed -> {
                if (offline) StateLine(stringResource(R.string.accounts_offline_changes_tv), Mark.Offline) else StateLine(state.failure.text(context), Mark.Error)
                Bar(null)
                Buttons {
                    AmberButton(
                        onClick = updates::press,
                        icon = null,
                        enabled = !offline,
                        modifier = Modifier.focusRequester(primary).remembered(memory, "update/retry"),
                    ) { ButtonLabel(stringResource(R.string.action_try_again)) }
                    TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/later")) { ButtonLabel(stringResource(R.string.action_not_now)) }
                }
            }
        }
    }
}

/** What a state line leads with: nothing, the offline line's haze dot, or an error's vermilion one. */
private enum class Mark { None, Offline, Error }

/** The state's line, two lines high whatever it says. An error's words are silk, since they say what to do. */
@Composable
private fun StateLine(text: String, mark: Mark = Mark.None) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        if (mark != Mark.None) {
            Box(Modifier.padding(top = 9.dp).size(8.dp).background(if (mark == Mark.Error) ObiSilk.Vermilion else ObiSilk.Haze, CircleShape))
        }
        Text(
            text,
            style = MaterialTheme.typography.bodyLarge,
            color = if (mark == Mark.Error) ObiSilk.Silk else ObiSilk.Haze,
            maxLines = 2,
            minLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun Buttons(content: @Composable () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) { content() }
}

/**
 * How far the download is, in amber on a rule-coloured track; full while it is checked. With no
 * [fraction] nothing is under way, and the bar's place stays empty, so the buttons never move.
 */
@Composable
private fun Bar(fraction: Float?) {
    val shown = fraction ?: 0f
    Box(
        Modifier
            .width(480.dp)
            .height(6.dp)
            .then(
                if (fraction == null) {
                    Modifier
                } else {
                    Modifier.background(ObiSilk.Rule, CircleShape).semantics { progressBarRangeInfo = ProgressBarRangeInfo(shown, 0f..1f) }
                },
            ),
    ) {
        if (fraction != null) Box(Modifier.fillMaxHeight().fillMaxWidth(shown).background(ObiSilk.Amber, CircleShape))
    }
}

/**
 * The one screen for Android's switch, the first time Update is pressed: what it is, where it is,
 * and the button that opens it. Back from Android's page, the download starts by itself once the
 * switch is on.
 */
@Composable
private fun AllowInstalls(state: UpdateState.Allowing, memory: FocusMemory, onOpen: () -> Unit, onNotNow: () -> Unit) {
    val open = remember { FocusRequester() }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(stringResource(R.string.updates_allow_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Column(Modifier.widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Step(1, stringResource(R.string.updates_allow_step_1))
            Step(2, stringResource(R.string.updates_allow_step_2))
            Step(3, stringResource(R.string.updates_allow_step_3))
        }
        Text(
            if (state.opened) {
                stringResource(R.string.updates_allow_waiting, stringResource(R.string.updates_unknown_sources_path))
            } else {
                stringResource(R.string.updates_allow_once, stringResource(R.string.updates_unknown_sources_path))
            },
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Silk,
            minLines = 2,
            maxLines = 2,
            modifier = Modifier.widthIn(max = 720.dp),
        )
        Spacer(Modifier.height(8.dp))
        Buttons {
            AmberButton(onClick = onOpen, icon = null, modifier = Modifier.focusRequester(open).remembered(memory, "allow/open")) { ButtonLabel(stringResource(R.string.action_open_settings)) }
            TonalButton(onClick = onNotNow, modifier = Modifier.remembered(memory, "allow/later")) { ButtonLabel(stringResource(R.string.action_not_now)) }
        }
    }
    LaunchedEffect(Unit) { open.focusSoon() }
}

/** One step: its number in haze, then what to do. The screen's one amber is its button. */
@Composable
private fun Step(number: Int, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(Modifier.width(32.dp).height(32.dp), contentAlignment = Alignment.Center) {
            Text(number.toString(), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Haze)
        }
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
    }
}
