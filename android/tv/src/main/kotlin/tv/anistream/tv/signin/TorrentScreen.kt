// TVTorrents: turning torrents on, or changing what protects them, on the TV. The phone's sheet on
// the D-pad: Protect with as radio rows, no protection confirmed before it is picked, and the
// torrent index (with a SOCKS5 proxy's address when that is the choice) from the TV's own page on
// a phone, or typed with the system keyboard as the last resort. Edit proxy is the same form for a
// saved proxy, filled in, and saves only once the connection check passes through the new one.

package tv.anistream.tv.signin

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
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
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
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
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.engine.model.TorrentProxy
import tv.anistream.store.ProvidersModel
import tv.anistream.store.TorrentWords
import tv.anistream.tv.player.Spinner
import tv.anistream.tv.remote.ConfirmDialog
import tv.anistream.tv.remote.QrCode
import tv.anistream.tv.remote.localAddress
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvInfoRow
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.focusSoon

/** Why the screen is open: torrents turning on, their protection or index changing, or the saved proxy's details. */
enum class TorrentMode { TurnOn, Change, EditProxy }

/** The protections, in the phone's order. */
private val Protections = listOf(TorrentProtection.PROXY, TorrentProtection.DEVICE, TorrentProtection.NONE)

/**
 * TVTorrents, until Turn on (or Save) or Back, then [onDone]. [start] is the protection picked in
 * Settings, which the screen starts on; no protection is confirmed first.
 */
@Composable
fun TorrentScreen(mode: TorrentMode, start: TorrentProtection?, onDone: () -> Unit) {
    if (mode == TorrentMode.EditProxy) {
        EditProxyScreen(onDone)
        return
    }
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val torrent = view?.torrent
    val saved = torrent?.proxy
    val savedProtection = torrent?.protection?.takeIf { mode == TorrentMode.Change } ?: TorrentProtection.DEVICE
    var acknowledged by remember { mutableStateOf(torrent?.protection == TorrentProtection.NONE) }
    var protection by remember { mutableStateOf(start?.takeIf { it != TorrentProtection.NONE || acknowledged } ?: savedProtection) }
    var asking by remember { mutableStateOf(start == TorrentProtection.NONE && !acknowledged) }
    var index by remember { mutableStateOf(view?.sources?.index.orEmpty()) }
    var host by remember { mutableStateOf(saved?.host.orEmpty()) }
    var port by remember { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by remember { mutableStateOf(saved?.user.orEmpty()) }
    // Never kept with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    var typing by remember { mutableStateOf(false) }
    // Asked for again from the details once they are whole: Change opens a new page.
    var asked by remember { mutableIntStateOf(0) }
    val proxy = protection == TorrentProtection.PROXY
    val whole = TorrentWords.indexOk(index) && (!proxy || TorrentWords.proxyOk(host, port))
    val done by rememberUpdatedState(onDone)
    BackHandler { if (typing) typing = false else done() }

    val turnOn = {
        val trimmed = index.trim()
        if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, SettingValue.Str(trimmed))
        store.setTorrentProtection(
            protection,
            proxy = if (proxy) {
                TorrentProxy(
                    host = host.trim(),
                    port = port.trim().toInt().toUShort(),
                    user = user.trim().ifEmpty { null },
                    // Left empty, the one kept already stays.
                    password = password.takeIf { it.isNotEmpty() },
                )
            } else {
                null
            },
            onFailure = torrent?.onFailure ?: FailedCheck.PAUSE,
            acknowledged = protection == TorrentProtection.NONE && acknowledged,
        )
        password = ""
        if (mode == TorrentMode.TurnOn) store.setSwitch(SettingKey.TORRENTS, true)
        done()
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            if (mode == TorrentMode.TurnOn) "Turn on torrents" else "Protect torrent traffic",
            style = MaterialTheme.typography.titleLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.semantics { heading() },
        )
        Text(TorrentWords.WHY, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (typing) {
            TypeDetails(
                proxy = proxy,
                index = index,
                host = host,
                port = port,
                user = user,
                password = password,
                onIndex = { index = it },
                onHost = { host = it },
                onPort = { port = it.filter(Char::isDigit).take(5) },
                onUser = { user = it },
                onPassword = { password = it },
                onDone = { typing = false },
            )
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(40.dp)) {
                ProtectWith(protection) { picked ->
                    when {
                        picked == protection -> {}
                        picked == TorrentProtection.NONE && !acknowledged -> asking = true
                        else -> protection = picked
                    }
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    // Torrents' details: from the page while they are not whole, or once Change asks again.
                    val form = if (proxy) PageForm.TorrentProxy else PageForm.Torrent
                    if (!whole || asked > 0) {
                        DetailsPage(form, opened = asked) { answer ->
                            index = answer.index
                            if (proxy) {
                                host = answer.host
                                port = answer.port
                                user = answer.user
                                password = answer.password
                            }
                            asked = 0
                        }
                    } else {
                        Column {
                            TvInfoRow("Torrent index", supporting = index.trim())
                            if (proxy) TvInfoRow("Proxy", supporting = proxyLine(host, port, user))
                        }
                    }
                    Text(
                        TorrentWords.checkNote(torrent?.onFailure, protection),
                        style = MaterialTheme.typography.bodyMedium,
                        color = ObiSilk.Haze,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        AmberButton(onClick = turnOn, icon = null, enabled = whole) {
                            ButtonLabel(if (mode == TorrentMode.TurnOn) "Turn on" else "Save")
                        }
                        if (whole && asked == 0) {
                            TonalButton(onClick = { asked += 1 }) { ButtonLabel("Change") }
                        }
                        TonalButton(onClick = { typing = true }) { ButtonLabel("Type it") }
                        TonalButton(onClick = done) { ButtonLabel("Cancel") }
                    }
                }
            }
        }
    }
    if (asking) {
        ConfirmDialog(
            title = TorrentWords.NONE_TITLE,
            body = TorrentWords.NONE_BODY,
            action = "No protection",
            onConfirm = {
                asking = false
                acknowledged = true
                protection = TorrentProtection.NONE
            },
            onCancel = { asking = false },
        )
    }
}

/** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
internal fun proxyLine(host: String, port: String, user: String): String = TorrentWords.proxyLine(host, port, user)

/**
 * Edit proxy: the saved proxy's details, filled in but for the password, changed from the page or
 * typed. Save checks them first and only a passed check saves them; a failed one says why, and the
 * saved proxy stays.
 */
@Composable
private fun EditProxyScreen(onDone: () -> Unit) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val trial by store.providers.trial.collectAsStateWithLifecycle()
    val saved = view?.torrent?.proxy
    var host by remember { mutableStateOf(saved?.host.orEmpty()) }
    var port by remember { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by remember { mutableStateOf(saved?.user.orEmpty()) }
    // Never kept with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    var typing by remember { mutableStateOf(false) }
    var asked by remember { mutableIntStateOf(0) }
    val checking = trial == ProvidersModel.Trial.Checking
    val failed = (trial as? ProvidersModel.Trial.Said)?.view?.takeIf { it !is GuardView.Protected }
    val whole = TorrentWords.proxyOk(host, port)
    val done by rememberUpdatedState(onDone)
    val proxy = {
        TorrentProxy(
            host = host.trim(),
            port = port.trim().toInt().toUShort(),
            user = user.trim().ifEmpty { null },
            // Left empty, the one kept already stays.
            password = password.takeIf { it.isNotEmpty() },
        )
    }
    BackHandler { if (typing) typing = false else done() }
    DisposableEffect(Unit) { onDispose { store.providers.clearTrial() } }
    LaunchedEffect(trial) {
        val said = trial as? ProvidersModel.Trial.Said ?: return@LaunchedEffect
        if (said.view is GuardView.Protected) {
            store.setTorrentProtection(TorrentProtection.PROXY, proxy(), view?.torrent?.onFailure ?: FailedCheck.PAUSE, acknowledged = false)
            password = ""
            done()
        }
    }
    // New details are a new try: a failed check's reason no longer stands.
    val changed = { if (failed != null) store.providers.clearTrial() }
    val save = remember { FocusRequester() }
    LaunchedEffect(Unit) { save.focusSoon() }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text("Edit proxy", style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(TorrentWords.EDIT_NOTE, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (typing) {
            TypeDetails(
                proxy = true,
                withIndex = false,
                index = "",
                host = host,
                port = port,
                user = user,
                password = password,
                onIndex = {},
                onHost = { host = it; changed() },
                onPort = { port = it.filter(Char::isDigit).take(5); changed() },
                onUser = { user = it; changed() },
                onPassword = { password = it; changed() },
                onDone = { typing = false },
            )
        } else {
            Column(Modifier.widthIn(max = 880.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                if (asked > 0 || !whole) {
                    DetailsPage(PageForm.Proxy, opened = asked, filled = mapOf("host" to host, "port" to port, "user" to user)) { answer ->
                        host = answer.host
                        port = answer.port
                        user = answer.user
                        password = answer.password
                        changed()
                        asked = 0
                    }
                } else {
                    TvInfoRow("Proxy", supporting = proxyLine(host, port, user))
                }
                when {
                    checking -> CheckingLine()
                    failed != null -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                        Text(TorrentWords.editFailed(failed), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
                    }
                    else -> Text(TorrentWords.PASSWORD_KEPT, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    AmberButton(
                        // Kept enabled while the check runs, so focus stays on it; a press then waits.
                        onClick = { if (!checking) store.checkTorrentProtection(TorrentProtection.PROXY, proxy()) },
                        icon = null,
                        enabled = whole,
                        modifier = Modifier.focusRequester(save),
                    ) { ButtonLabel("Save") }
                    if (whole && asked == 0) {
                        TonalButton(onClick = { asked += 1 }) { ButtonLabel("Change") }
                    }
                    TonalButton(onClick = { typing = true }) { ButtonLabel("Type it") }
                    TonalButton(onClick = done) { ButtonLabel("Cancel") }
                }
            }
        }
    }
}

/** Checking the proxy, its spinner after a moment so a quick answer never flashes it. */
@Composable
private fun CheckingLine() {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        var spin by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) {
            delay(SPINNER_WAIT)
            spin = true
        }
        Box(Modifier.size(32.dp)) { if (spin) Spinner() }
        Text(TorrentWords.CHECKING_PROXY, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
    }
}

/** How long Checking the proxy waits before its spinner shows, as Checking the store does. */
private const val SPINNER_WAIT = 400L

/** Protect with: the three protections as radio rows, the chosen one in the amber container and focused first. */
@Composable
private fun ProtectWith(chosen: TorrentProtection, onPick: (TorrentProtection) -> Unit) {
    val first = remember { FocusRequester() }
    Column(Modifier.width(300.dp).selectableGroup(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text("Protect with", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, bottom = 4.dp))
        Protections.forEach { each ->
            TvListRow(
                TorrentWords.protection(each),
                onClick = { onPick(each) },
                supporting = TorrentWords.protectionNote(each, "TV"),
                supportingLines = 2,
                height = 88.dp,
                selected = each == chosen,
                modifier = if (each == chosen) Modifier.focusRequester(first) else Modifier,
            )
        }
    }
    LaunchedEffect(Unit) { first.focusSoon() }
}

/**
 * The TV's page for the details, behind a QR code: it takes one answer and closes, or closes
 * after its 10 minutes, "Code expired" then in the code's place. [opened] counts the pages asked
 * for; the form changes with the protection, and a new page opens with it.
 */
@Composable
private fun DetailsPage(form: PageForm, opened: Int, filled: Map<String, String> = emptyMap(), onAnswer: (PageAnswer.Torrent) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val answer by rememberUpdatedState(onAnswer)
    var address by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }
    var expired by remember { mutableStateOf(false) }
    var again by remember { mutableIntStateOf(0) }
    DisposableEffect(form, opened, again) {
        expired = false
        started = false
        address = null
        val page = SignInPage(tracker = "torrents", form = form, authorizeUrl = null, filled = filled) { sent ->
            if (sent is PageAnswer.Torrent) scope.launch { answer(sent) }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val local = localAddress(context)
            address = if (port != null && local != null) "http://$local:$port/${page.path}" else null
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
    Row(horizontalArrangement = Arrangement.spacedBy(20.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = address.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, size = QR, label = if (form == PageForm.Proxy) "QR code for the page that takes the proxy" else "QR code for the page that takes the torrent index")
        } else {
            // The code's place kept while the page starts, so nothing moves when it comes.
            Box(Modifier.size(QR).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text("Code expired", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                when (form) {
                    PageForm.TorrentProxy -> "Scan with your phone, then fill in the torrent index and the proxy on the page that opens."
                    PageForm.Proxy -> "Scan with your phone, then change the proxy on the page that opens."
                    else -> "Scan with your phone, then fill in the torrent index on the page that opens."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Silk,
            )
            Text(
                when {
                    expired -> "The page closed after 10 minutes."
                    shown != null -> shown
                    started -> "This TV isn't on a network a phone can reach."
                    else -> "Starting"
                },
                style = MaterialTheme.typography.bodySmall,
                color = ObiSilk.Haze,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
            Text("The page closes once it's sent, or in 10 minutes.", style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
            if (expired) {
                TonalButton(onClick = { again += 1 }) { ButtonLabel("Get a new code") }
            }
        }
    }
}

/** The details typed with the system keyboard: the index, then the proxy's when it is the choice. */
@Composable
private fun TypeDetails(
    proxy: Boolean,
    withIndex: Boolean = true,
    index: String,
    host: String,
    port: String,
    user: String,
    password: String,
    onIndex: (String) -> Unit,
    onHost: (String) -> Unit,
    onPort: (String) -> Unit,
    onUser: (String) -> Unit,
    onPassword: (String) -> Unit,
    onDone: () -> Unit,
) {
    val first = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        first.focusSoon()
        keyboard?.show()
    }
    Column(Modifier.widthIn(max = 640.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (withIndex) {
            Field(index, onIndex, "Torrent index", KeyboardType.Uri, focus = first)
            Text(TorrentWords.indexNote(index), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 24.dp))
        }
        if (proxy) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(host, onHost, "Proxy host", KeyboardType.Uri, Modifier.weight(2f), focus = first.takeIf { !withIndex })
                Field(port, onPort, "Port", KeyboardType.Number, Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(user, onUser, "User name, if asked", KeyboardType.Text, Modifier.weight(1f))
                Field(password, onPassword, "Password, if asked", KeyboardType.Password, Modifier.weight(1f), secret = true)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            TonalButton(onClick = onDone) { ButtonLabel("Done") }
        }
    }
}

/**
 * One typed field, Search's pill: the raised ground and its name in haze until something is
 * typed, silk with ground ink while focused.
 */
@Composable
private fun Field(
    text: String,
    onChange: (String) -> Unit,
    name: String,
    type: KeyboardType,
    modifier: Modifier = Modifier,
    secret: Boolean = false,
    focus: FocusRequester? = null,
) {
    var focused by remember { mutableStateOf(false) }
    val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
    Box(
        modifier.fillMaxWidth().height(56.dp).background(if (focused) ObiSilk.Silk else ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (text.isEmpty()) Text(name, style = MaterialTheme.typography.bodyLarge, color = if (focused) ObiSilk.Ground else ObiSilk.Haze, maxLines = 1)
        BasicTextField(
            value = text,
            onValueChange = onChange,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = ink),
            cursorBrush = SolidColor(if (focused) ObiSilk.Ground else ObiSilk.Amber),
            visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(keyboardType = type, imeAction = ImeAction.Next, autoCorrectEnabled = false),
            modifier = Modifier
                .fillMaxWidth()
                .let { if (focus != null) it.focusRequester(focus) else it }
                .onFocusChanged { focused = it.isFocused },
        )
    }
}

/** The QR code's side: smaller than sign-in's, as the protection rows sit beside it. */
private val QR = 140.dp
