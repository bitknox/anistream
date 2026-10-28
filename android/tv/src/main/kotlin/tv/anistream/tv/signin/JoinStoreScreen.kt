// TVJoinStore: adding a plugin store on the TV itself, no paired phone needed. The TV's own page
// takes an invite link pasted on any phone, from a QR code; "Type the link" opens the system
// keyboard as the last resort. Either way the TV shows the store before joining, with the amber
// Join and the legal line under it.

package tv.anistream.tv.signin

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.InvitedStore
import tv.anistream.store.InviteLinks
import tv.anistream.store.StoresModel
import tv.anistream.store.runByLine
import tv.anistream.tv.remote.QrCode
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.LocalNotices
import tv.anistream.tv.player.Spinner
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.entryFocus
import tv.anistream.tv.ui.focusSoon
import tv.anistream.store.plainReason

/** The line under every Join. */
const val JOIN_LEGAL_LINE = "anistream hosts no content."

/**
 * TVJoinStore, until a store is joined ([onDone] with true) or Back ([onDone] with false). On
 * first launch [cancelLabel] is "Skip for now".
 */
@Composable
fun JoinStoreScreen(onDone: (joined: Boolean) -> Unit, cancelLabel: String = "Cancel", title: String = "Add a plugin store", line: String? = null) {
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val preview by store.stores.preview.collectAsStateWithLifecycle()
    val joined by store.stores.joined.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val done by rememberUpdatedState(onDone)
    LaunchedEffect(Unit) { store.stores.clearJoined() }
    LaunchedEffect(joined) {
        val id = joined ?: return@LaunchedEffect
        val name = stores.firstOrNull { it.id == id }?.name
        notices?.say(if (name != null) "Joined $name" else "Joined the plugin store")
        store.stores.clearJoined()
        done(true)
    }
    DisposableEffect(Unit) { onDispose { store.stores.clearPreview() } }
    var typing by remember { mutableStateOf(false) }
    var link by remember { mutableStateOf("") }
    BackHandler {
        when {
            preview != null -> store.stores.clearPreview()
            typing -> typing = false
            else -> done(false)
        }
    }
    val check: (String) -> Unit = { text ->
        val invite = InviteLinks.invite(text) ?: text.trim()
        link = invite
        typing = false
        store.previewStore(invite)
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        if (line != null) Text(line, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        when (val shown = preview) {
            null -> if (typing) {
                TypeLink(link, onCheck = check, onCancel = { typing = false })
            } else {
                Ways(onAnswer = check, onType = { typing = true }, onCancel = { done(false) }, cancelLabel = cancelLabel)
            }
            is StoresModel.Preview.Checking -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                // The spinner comes after a moment, so a quick answer never flashes it.
                var spin by remember { mutableStateOf(false) }
                LaunchedEffect(Unit) {
                    delay(SPINNER_WAIT)
                    spin = true
                }
                Box(Modifier.size(32.dp)) { if (spin) Spinner() }
                Text("Checking the store", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
            is StoresModel.Preview.Failed -> Failed(shown.reason, onAgain = { store.stores.clearPreview() }, onCancel = { done(false) }, cancelLabel = cancelLabel)
            is StoresModel.Preview.Store -> Preview(
                shown.store,
                onJoin = { store.joinStore(link) },
                onCancel = { store.stores.clearPreview() },
            )
        }
    }
}

/**
 * The ways to hand the TV a link: the TV's page from a QR code, or typing it. The page closes
 * after its 10 minutes; the QR code goes with it, "Code expired" in its place, and Get a new code
 * opens another page.
 */
@Composable
private fun Ways(onAnswer: (String) -> Unit, onType: () -> Unit, onCancel: () -> Unit, cancelLabel: String) {
    val context = LocalContext.current
    val pageHost = LocalPageHost.current
    val scope = rememberCoroutineScope()
    val answer by rememberUpdatedState(onAnswer)
    var address by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }
    // Each page in turn: a new one once the last has expired and Get a new code is pressed.
    var opened by remember { mutableIntStateOf(0) }
    var expired by remember { mutableStateOf(false) }
    DisposableEffect(opened) {
        expired = false
        val page = SignInPage(tracker = "a plugin store", form = PageForm.Link, authorizeUrl = null) { pasted ->
            if (pasted is PageAnswer.Link) scope.launch { answer(pasted.text) }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val local = pageHost(context)
            address = if (port != null && local != null) page.address(local, port) else null
            started = true
            if (port != null) {
                delay(SignInPage.LIFETIME_NANOS / 1_000_000)
                expired = true
                page.stop()
            }
        }
        onDispose {
            job.cancel()
            page.stop()
        }
    }
    val type = remember { FocusRequester() }
    val again = remember { FocusRequester() }
    LaunchedEffect(expired) { if (expired) again.focusSoon() }
    Text(
        "Scan the QR code with your phone, then paste the store's invite link on the page that opens.",
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Silk,
        modifier = Modifier.widthIn(max = 640.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = address.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, label = "QR code for the page that takes the link") // i18n-skip
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text("Code expired", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 520.dp)) {
            Text("Or open this address", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            Text(
                when {
                    expired -> "The page closed after 10 minutes."
                    shown != null -> shown
                    started -> "This TV isn't on a network a phone can reach."
                    else -> "Starting"
                },
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Silk,
            )
            Text("The page closes with this screen, or in 10 minutes.", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        }
    }
    Row(Modifier.entryFocus(type), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (expired) {
            TonalButton(onClick = { opened += 1 }, modifier = Modifier.focusRequester(again)) { ButtonLabel("Get a new code") }
        }
        TonalButton(onClick = onType, modifier = Modifier.focusRequester(type)) { ButtonLabel("Type the link") }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/** The link typed with the system keyboard, then checked. */
@Composable
private fun TypeLink(start: String, onCheck: (String) -> Unit, onCancel: () -> Unit) {
    var text by remember { mutableStateOf(start) }
    val field = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        field.focusSoon()
        keyboard?.show()
    }
    Box(
        Modifier.widthIn(max = 640.dp).fillMaxWidth().height(56.dp).background(ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (text.isEmpty()) Text("Invite link", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        BasicTextField(
            value = text,
            onValueChange = { text = it },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
            cursorBrush = SolidColor(ObiSilk.Amber),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onGo = { if (text.isNotBlank()) onCheck(text) }),
            modifier = Modifier.fillMaxWidth().focusRequester(field),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = { onCheck(text) }, enabled = text.isNotBlank()) { ButtonLabel("Check the link") }
        TonalButton(onClick = onCancel) { ButtonLabel("Back") }
    }
}

/** The link did not lead to a store: an error's vermilion dot, why in plain words, another try, and Cancel. */
@Composable
private fun Failed(reason: String, onAgain: () -> Unit, onCancel: () -> Unit, cancelLabel: String) {
    val again = remember { FocusRequester() }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
        Text("Couldn't open this link.", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
    }
    Text(plainReason(LocalContext.current, reason), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    Row(Modifier.entryFocus(again), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = onAgain, modifier = Modifier.focusRequester(again)) { ButtonLabel("Try another link") }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/**
 * The store before joining: its name, who runs it, its key, its plugins with the hosts each
 * contacts, and whether anyone with the link can join. The amber Join, focused, with the legal
 * line under it.
 */
@Composable
private fun Preview(invited: InvitedStore, onJoin: () -> Unit, onCancel: () -> Unit) {
    val store = LocalAppStore.current
    val working by store.stores.working.collectAsStateWithLifecycle()
    val joining = store.stores.isWorking(working, StoresModel.Work.Join)
    val join = remember { FocusRequester() }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(invited.name, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
        Text(invited.runByLine, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        Text("Key ${invited.fingerprint.chunked(4).take(4).joinToString(" ")}", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        if (invited.open) Text("Anyone with the link can join.", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
    if (invited.plugins.isNotEmpty()) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.widthIn(max = 720.dp)) {
            invited.plugins.take(PREVIEW_PLUGINS).forEach { plugin ->
                Row {
                    Text(plugin.name, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.width(200.dp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(plugin.hosts.joinToString(", "), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (invited.plugins.size > PREVIEW_PLUGINS) {
                Text("and ${invited.plugins.size - PREVIEW_PLUGINS} more", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            }
        }
    }
    Column(Modifier.entryFocus(join)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (invited.alreadyJoined) {
                TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(join)) { ButtonLabel("Already joined") }
            } else {
                AmberButton(onClick = onJoin, icon = null, enabled = !joining, modifier = Modifier.focusRequester(join)) { ButtonLabel(if (joining) "Joining" else "Join") }
                TonalButton(onClick = onCancel) { ButtonLabel("Cancel") }
            }
        }
        Spacer(Modifier.height(12.dp))
        Text(JOIN_LEGAL_LINE, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
    }
}

/** How long Checking the store waits before its spinner shows. */
private const val SPINNER_WAIT = 400L

/** A store with many plugins shows the first few; the store's page in Settings has the rest. */
private const val PREVIEW_PLUGINS = 5
