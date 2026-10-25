// TVUpdate: the newer release, from Settings › About or its notice. The version, its size and
// what changed; then one place below that says where the update stands, the same height in every
// state so nothing moves: Update, the download's bar with Cancel, Android's installer, or why it
// stopped with Try again. The first press with "Install unknown apps" off shows the one screen
// that explains the switch and opens Android's page for it.

package tv.anistream.tv.screens

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
import tv.anistream.tv.ui.OfflineLine
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.focusSoon
import tv.anistream.tv.ui.isOffline
import tv.anistream.tv.ui.remembered

/** Where Android keeps the switch on Google TV, for a TV whose page does not open by itself. */
const val UNKNOWN_SOURCES_PATH = "Settings › Apps › Security & restrictions › Unknown sources"

/** What the update's place says, in words, for the state it is in. */
object UpdateWords {
    /** The line under the version: its size and what this TV runs now. */
    fun about(offer: UpdateOffer, installed: String?): String = listOfNotNull(
        offer.bytes.takeIf { it > 0 }?.let(Wording::bytes),
        installed?.let { "This TV has v${it.removePrefix("v")}" },
    ).joinToString(" · ")

    /** "Downloading · 34 % · 16 MB of 48 MB". */
    fun downloading(state: UpdateState.Downloading): String {
        val fraction = state.fraction ?: return "Downloading · ${Wording.bytes(state.done)}"
        return "Downloading · ${(fraction * 100).toInt()} % · ${Wording.bytes(state.done)} of ${Wording.bytes(state.total)}"
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
            Text("anistream is up to date", style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            installed?.let { Text("This TV has v${it.removePrefix("v")}", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze) }
            Spacer(Modifier.height(8.dp))
            TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/back")) { ButtonLabel("Back") }
            EnterFocus(memory, entry = "none")
            return@Column
        }
        Text("Update to anistream ${offer.name}", style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(UpdateWords.about(offer, installed), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        // What changed, as the release's notes begin: four lines at most, so the place below
        // always sits where it did.
        Text(
            offer.notes.ifBlank { "This release's notes are on its page." },
            style = MaterialTheme.typography.bodyLarge,
            color = ObiSilk.Silk,
            maxLines = 4,
            minLines = 4,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 720.dp),
        )
        Spacer(Modifier.height(8.dp))
        Where(current, offline, memory, onDone)
    }
}

/** The place under the notes: the same height whatever it says, so focus and text never jump. */
@Composable
private fun Where(state: UpdateState, offline: Boolean, memory: FocusMemory, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val updates = store.updates
    val installs = state.offer?.let(updates::installs) ?: false
    val primary = remember { FocusRequester() }
    // A new state has a new first button: it takes focus, where the last one went away.
    LaunchedEffect(state::class) { primary.focusSoon() }
    Column(Modifier.height(StateHeight).widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        when (state) {
            is UpdateState.Available, is UpdateState.None, is UpdateState.Allowing -> {
                StateLine(
                    when {
                        !installs -> "This build doesn't update itself. Install the release's TV app from its page."
                        offline -> null
                        else -> "Nothing installs until Android asks you to confirm."
                    },
                )
                if (offline) OfflineLine()
                Buttons {
                    AmberButton(
                        onClick = updates::press,
                        icon = null,
                        enabled = installs && !offline,
                        modifier = Modifier.focusRequester(primary).remembered(memory, "update/press"),
                    ) { ButtonLabel("Update") }
                    TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/later")) { ButtonLabel("Not now") }
                }
            }
            is UpdateState.Downloading -> {
                StateLine(UpdateWords.downloading(state))
                Bar(state.fraction)
                Buttons {
                    TonalButton(onClick = updates::cancel, modifier = Modifier.focusRequester(primary).remembered(memory, "update/cancel")) { ButtonLabel("Cancel") }
                }
            }
            is UpdateState.Verifying -> {
                StateLine("Checking the download")
                Bar(1f)
                Buttons {
                    TonalButton(onClick = updates::cancel, modifier = Modifier.focusRequester(primary).remembered(memory, "update/cancel")) { ButtonLabel("Cancel") }
                }
            }
            is UpdateState.Installing -> {
                StateLine("Android is asking to install the update. Confirm on its screen; your sign-ins, lists and settings stay.")
                Bar(1f)
                Buttons {
                    TonalButton(onClick = updates::cancel, modifier = Modifier.focusRequester(primary).remembered(memory, "update/cancel")) { ButtonLabel("Cancel") }
                }
            }
            is UpdateState.Done -> {
                StateLine("Installed. anistream opens on ${state.offer.name} next time.")
                Buttons {
                    TonalButton(onClick = onDone, modifier = Modifier.focusRequester(primary).remembered(memory, "update/done")) { ButtonLabel("Done") }
                }
            }
            is UpdateState.Failed -> {
                StateLine(state.failure.text)
                if (offline) OfflineLine()
                Buttons {
                    AmberButton(
                        onClick = updates::press,
                        icon = null,
                        enabled = !offline,
                        modifier = Modifier.focusRequester(primary).remembered(memory, "update/retry"),
                    ) { ButtonLabel("Try again") }
                    TonalButton(onClick = onDone, modifier = Modifier.remembered(memory, "update/later")) { ButtonLabel("Not now") }
                }
            }
        }
    }
}

/** The tallest state: two lines, the bar and the buttons. */
private val StateHeight = 220.dp

@Composable
private fun StateLine(text: String?) {
    Text(
        text.orEmpty(),
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Haze,
        maxLines = 2,
        minLines = 2,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun Buttons(content: @Composable () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) { content() }
}

/** How far the download is, in water on a rule-coloured track; full while it is checked. */
@Composable
private fun Bar(fraction: Float?) {
    val shown = fraction ?: 0f
    Box(
        Modifier
            .width(480.dp)
            .height(6.dp)
            .background(ObiSilk.Rule, CircleShape)
            .semantics { progressBarRangeInfo = ProgressBarRangeInfo(shown, 0f..1f) },
    ) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(shown).background(ObiSilk.Water, CircleShape))
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
        Text("Allow anistream to install its update", style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Column(Modifier.widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Step(1, "Choose Open settings. Android shows the apps that may install other apps.")
            Step(2, "Turn on anistream.")
            Step(3, "Press Back. The update downloads, and Android asks you to confirm it.")
        }
        Text(
            if (state.opened) {
                "Waiting for the switch. If Android's page didn't open, find it under $UNKNOWN_SOURCES_PATH."
            } else {
                "Android asks this once. If its page doesn't open, the switch is under $UNKNOWN_SOURCES_PATH."
            },
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Haze,
            minLines = 2,
            maxLines = 2,
            modifier = Modifier.widthIn(max = 720.dp),
        )
        Spacer(Modifier.height(8.dp))
        Buttons {
            AmberButton(onClick = onOpen, icon = null, modifier = Modifier.focusRequester(open).remembered(memory, "allow/open")) { ButtonLabel("Open settings") }
            TonalButton(onClick = onNotNow, modifier = Modifier.remembered(memory, "allow/later")) { ButtonLabel("Not now") }
        }
    }
    LaunchedEffect(Unit) { open.focusSoon() }
}

/** One step: its number in an amber circle, then what to do. */
@Composable
private fun Step(number: Int, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(Modifier.width(32.dp).height(32.dp).background(ObiSilk.AmberContainer, CircleShape), contentAlignment = Alignment.Center) {
            Text(number.toString(), style = MaterialTheme.typography.labelLarge, color = ObiSilk.OnAmberContainer)
        }
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
    }
}
