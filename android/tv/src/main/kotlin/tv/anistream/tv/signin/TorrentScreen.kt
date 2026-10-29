// TVTorrents: turning torrents on, or changing what protects them, on the TV. The phone's sheet on
// the D-pad: Protect with as radio rows, no protection confirmed before it is picked, and the
// torrent index (with a SOCKS5 proxy's address when that is the choice) from the TV's own page on
// a phone, or typed with the system keyboard as the last resort. A new or changed proxy is saved
// only once the connection check passes through it, and a failed check saves nothing. The torrent
// index has a screen of its own, and Edit proxy is the proxy's form, filled in.

package tv.anistream.tv.signin

import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.R
import android.content.Context
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
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
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
import tv.anistream.engine.model.ProxyView
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
import tv.anistream.tv.ui.LocalNotices
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.TvInfoRow
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.focusSoon

/**
 * Why the screen is open: torrents turning on, their protection changing, the index changing, or
 * the saved proxy's details.
 */
enum class TorrentMode { TurnOn, Change, Index, EditProxy }

/** The protections, in the phone's order. */
private val Protections = listOf(TorrentProtection.PROXY, TorrentProtection.DEVICE, TorrentProtection.NONE)

/** Where the QR page's address is reached on this network: this TV's own address, or none. */
val LocalPageHost = staticCompositionLocalOf<(Context) -> String?> { ::localAddress }

/**
 * TVTorrents, until Turn on (or Save) or Back, then [onDone]. [start] is the protection picked in
 * Settings, which the screen starts on; no protection is confirmed first.
 */
@Composable
fun TorrentScreen(mode: TorrentMode, start: TorrentProtection?, onDone: () -> Unit) {
    when (mode) {
        TorrentMode.EditProxy -> EditProxyScreen(onDone)
        TorrentMode.Index -> IndexScreen(onDone)
        TorrentMode.TurnOn, TorrentMode.Change -> ProtectScreen(mode, start, onDone)
    }
}

/** Turn on torrents, or Protect torrent traffic: the protection, the index, and a proxy's details. */
@Composable
private fun ProtectScreen(mode: TorrentMode, start: TorrentProtection?, onDone: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val notices = LocalNotices.current
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
    val form = rememberForm()
    val proxy = protection == TorrentProtection.PROXY
    val whole = TorrentWords.indexOk(index) && (!proxy || TorrentWords.proxyOk(host, port))
    val details = if (proxy) proxyOf(host, port, user, password) else null
    // A new or changed proxy is checked before anything is saved; the same saved one is not again.
    val checks = proxy && (mode == TorrentMode.TurnOn || torrent?.protection != TorrentProtection.PROXY || !sameAs(saved, host, port, user) || password.isNotEmpty())
    val done by rememberUpdatedState(onDone)
    BackHandler { if (form.typing) form.stopTyping() else done() }

    val save = { checked: TorrentProxy? ->
        val trimmed = index.trim()
        if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, SettingValue.Str(trimmed))
        store.setTorrentProtection(
            protection,
            proxy = checked,
            onFailure = torrent?.onFailure ?: FailedCheck.PAUSE,
            acknowledged = protection == TorrentProtection.NONE && acknowledged,
        )
        password = ""
        if (mode == TorrentMode.TurnOn) store.setSwitch(SettingKey.TORRENTS, true)
        notices?.say(context.getString(if (mode == TorrentMode.TurnOn) TorrentWords.SAID_ON else TorrentWords.SAID_PROTECTION))
        done()
    }
    val check = proxyCheck(details.takeIf { checks }, onPassed = save)
    val press = {
        when {
            // Kept enabled while the check runs, so focus stays on it; a press then waits.
            check.checking -> {}
            checks && details != null -> store.checkTorrentProtection(TorrentProtection.PROXY, details)
            else -> save(details)
        }
    }
    LaunchedEffect(Unit) { form.first.focusSoon() }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            stringResource(if (mode == TorrentMode.TurnOn) R.string.torrents_sheet_turn_on_title else R.string.torrents_sheet_protect_title),
            style = MaterialTheme.typography.titleLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.semantics { heading() },
        )
        Text(stringResource(TorrentWords.WHY), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (form.typing) {
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
                onDone = form::stopTyping,
            )
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(32.dp)) {
                ProtectWith(protection, form.first) { picked ->
                    when {
                        picked == protection || check.checking -> {}
                        picked == TorrentProtection.NONE && !acknowledged -> asking = true
                        else -> protection = picked
                    }
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    // Torrents' details: from the page while they are not whole, or once a phone is asked again.
                    if (!whole || form.asked > 0) {
                        DetailsPage(if (proxy) PageForm.TorrentProxy else PageForm.Torrent, opened = form.asked) { answer ->
                            index = answer.index
                            if (proxy) {
                                host = answer.host
                                port = answer.port
                                user = answer.user
                                password = answer.password
                            }
                            form.answered()
                        }
                    } else {
                        Column {
                            TvInfoRow(stringResource(R.string.torrents_index), supporting = index.trim())
                            if (proxy) TvInfoRow(stringResource(R.string.torrents_protect_proxy), supporting = proxyLine(context, host, port, user))
                        }
                    }
                    Status(
                        PROTECT_STATUS,
                        checking = check.checking,
                        failed = check.failed?.let {
                            TorrentWords.proxyFailed(context, it, if (mode == TorrentMode.TurnOn) TorrentWords.Kept.TurningOn else TorrentWords.Kept.Protection)
                        },
                        otherwise = TorrentWords.checkNote(context, torrent?.onFailure, protection),
                    )
                    FormButtons(form, primary = stringResource(if (mode == TorrentMode.TurnOn) R.string.torrents_sheet_turn_on_button else R.string.action_save), whole = whole, checking = check.checking, onPrimary = press, onCancel = done)
                }
            }
        }
    }
    if (asking) {
        ConfirmDialog(
            title = stringResource(TorrentWords.NONE_TITLE),
            body = stringResource(TorrentWords.NONE_BODY),
            action = stringResource(R.string.torrents_protect_none),
            onConfirm = {
                asking = false
                acknowledged = true
                protection = TorrentProtection.NONE
            },
            onCancel = { asking = false },
        )
    }
}

/** The torrent index alone, from the Torrent index row: from a phone, or typed. */
@Composable
private fun IndexScreen(onDone: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    var index by remember { mutableStateOf(view?.sources?.index.orEmpty()) }
    val form = rememberForm()
    val whole = TorrentWords.indexOk(index)
    val done by rememberUpdatedState(onDone)
    BackHandler { if (form.typing) form.stopTyping() else done() }
    LaunchedEffect(Unit) { form.first.focusSoon() }
    val save = {
        val trimmed = index.trim()
        if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, SettingValue.Str(trimmed))
        notices?.say(context.getString(TorrentWords.SAID_INDEX))
        done()
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(stringResource(R.string.torrents_index), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(TorrentWords.indexAbout(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (form.typing) {
            TypeDetails(
                proxy = false,
                index = index,
                host = "",
                port = "",
                user = "",
                password = "",
                onIndex = { index = it },
                onHost = {},
                onPort = {},
                onUser = {},
                onPassword = {},
                onDone = form::stopTyping,
            )
        } else {
            Column(Modifier.widthIn(max = 880.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                if (!whole || form.asked > 0) {
                    DetailsPage(PageForm.Torrent, opened = form.asked) { answer ->
                        index = answer.index
                        form.answered()
                    }
                } else {
                    TvInfoRow(stringResource(R.string.torrents_index), supporting = index.trim())
                }
                FormButtons(form, primary = stringResource(R.string.action_save), whole = whole, checking = false, onPrimary = save, onCancel = done, primaryFirst = true)
            }
        }
    }
}

/**
 * Edit proxy: the saved proxy's details, filled in but for the password, changed from the page or
 * typed. Save checks them first and only a passed check saves them; a failed one says why, and the
 * saved proxy stays.
 */
@Composable
private fun EditProxyScreen(onDone: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved = view?.torrent?.proxy
    var host by remember { mutableStateOf(saved?.host.orEmpty()) }
    var port by remember { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by remember { mutableStateOf(saved?.user.orEmpty()) }
    // Never kept with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    val form = rememberForm()
    val whole = TorrentWords.proxyOk(host, port)
    val details = proxyOf(host, port, user, password)
    val done by rememberUpdatedState(onDone)
    val check = proxyCheck(details) { checked ->
        store.setTorrentProtection(TorrentProtection.PROXY, checked, view?.torrent?.onFailure ?: FailedCheck.PAUSE, acknowledged = false)
        password = ""
        notices?.say(context.getString(TorrentWords.SAID_PROXY))
        done()
    }
    BackHandler { if (form.typing) form.stopTyping() else done() }
    LaunchedEffect(Unit) { form.first.focusSoon() }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(stringResource(R.string.torrents_sheet_edit_proxy_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(stringResource(TorrentWords.EDIT_NOTE), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (form.typing) {
            TypeDetails(
                proxy = true,
                withIndex = false,
                index = "",
                host = host,
                port = port,
                user = user,
                password = password,
                onIndex = {},
                onHost = { host = it },
                onPort = { port = it.filter(Char::isDigit).take(5) },
                onUser = { user = it },
                onPassword = { password = it },
                onDone = form::stopTyping,
            )
        } else {
            Column(Modifier.widthIn(max = 880.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                if (form.asked > 0 || !whole) {
                    DetailsPage(PageForm.Proxy, opened = form.asked, filled = mapOf("host" to host, "port" to port, "user" to user)) { answer ->
                        host = answer.host
                        port = answer.port
                        user = answer.user
                        password = answer.password
                        form.answered()
                    }
                } else {
                    TvInfoRow(stringResource(R.string.torrents_protect_proxy), supporting = proxyLine(context, host, port, user))
                }
                Status(EDIT_STATUS, checking = check.checking, failed = check.failed?.let { TorrentWords.editFailed(context, it) }, otherwise = stringResource(TorrentWords.PASSWORD_KEPT))
                FormButtons(
                    form,
                    primary = stringResource(R.string.action_save),
                    whole = whole,
                    checking = check.checking,
                    onPrimary = { if (!check.checking && details != null) store.checkTorrentProtection(TorrentProtection.PROXY, details) },
                    onCancel = done,
                    primaryFirst = true,
                )
            }
        }
    }
}

/**
 * A torrents form's moving parts: the page asked for again ([asked]), the details being typed,
 * and where focus goes as they come and go. [first] is the screen's first focus; [phone] and
 * [type] are the buttons that open the page and the typing.
 */
private class FormState {
    var asked by mutableIntStateOf(0)
        private set
    var typing by mutableStateOf(false)
        private set

    /** Bumped as the typing closes, so Type it takes focus back. */
    var typed by mutableIntStateOf(0)
        private set
    val first = FocusRequester()
    val type = FocusRequester()

    fun askPhone() {
        asked += 1
    }

    fun answered() {
        asked = 0
    }

    fun startTyping() {
        typing = true
    }

    fun stopTyping() {
        typing = false
        typed += 1
    }
}

@Composable
private fun rememberForm(): FormState = remember { FormState() }

/**
 * The form's buttons, all one height: [primary] in amber, "Use a phone" while the details are
 * whole and no page is open, "Type it" and "Cancel". While a check runs the others are locked,
 * and the amber one keeps focus. Focus comes back to Type it as the typing or a new page takes the
 * place of the button pressed. [primaryFirst] focuses the amber one as the screen opens.
 */
@Composable
private fun FormButtons(
    form: FormState,
    primary: String,
    whole: Boolean,
    checking: Boolean,
    onPrimary: () -> Unit,
    onCancel: () -> Unit,
    primaryFirst: Boolean = false,
) {
    // The page replaces Use a phone, and Type it moves into its place: focus goes with it.
    LaunchedEffect(form.asked) { if (form.asked > 0) form.type.focusSoon() }
    LaunchedEffect(form.typed) { if (form.typed > 0) form.type.focusSoon() }
    // Four buttons fit beside Protect with only this close together.
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        AmberButton(onClick = onPrimary, icon = null, enabled = whole, modifier = if (primaryFirst) Modifier.focusRequester(form.first) else Modifier) {
            ButtonLabel(primary)
        }
        if (whole && form.asked == 0) {
            TonalButton(onClick = form::askPhone, enabled = !checking, height = BUTTON_HEIGHT) { ButtonLabel(stringResource(R.string.tv_use_phone)) }
        }
        TonalButton(onClick = form::startTyping, enabled = !checking, height = BUTTON_HEIGHT, modifier = Modifier.focusRequester(form.type)) { ButtonLabel(stringResource(R.string.tv_type_it)) }
        TonalButton(onClick = onCancel, height = BUTTON_HEIGHT) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
}

/** The amber action's height, which the row's tonal buttons take too. */
private val BUTTON_HEIGHT = 56.dp

/** What a proxy's check before saving says on screen: under way, or why it failed. */
private class ProxyCheck(val checking: Boolean, val failed: GuardView?)

/**
 * The check of [asked], the details the form holds now: a verdict on any other details, ones
 * since changed or a check from before, is set aside. A pass hands [onPassed] exactly what was
 * checked, the password as given included.
 */
@Composable
private fun proxyCheck(asked: TorrentProxy?, onPassed: (TorrentProxy) -> Unit): ProxyCheck {
    val store = LocalAppStore.current
    val trial by store.providers.trial.collectAsStateWithLifecycle()
    val mine = trial?.takeIf { asked != null && it.isFor(TorrentProtection.PROXY, asked) }
    val passed by rememberUpdatedState(onPassed)
    DisposableEffect(Unit) { onDispose { store.providers.clearTrial() } }
    LaunchedEffect(mine) {
        val said = mine as? ProvidersModel.Trial.Said ?: return@LaunchedEffect
        val checked = said.proxy
        if (said.view is GuardView.Protected && checked != null) {
            store.providers.clearTrial()
            passed(checked)
        }
    }
    return ProxyCheck(
        checking = mine is ProvidersModel.Trial.Checking,
        failed = (mine as? ProvidersModel.Trial.Said)?.view?.takeIf { it !is GuardView.Protected },
    )
}

/** The proxy the fields make, once its host and port are whole. A password left empty keeps the one saved. */
private fun proxyOf(host: String, port: String, user: String, password: String): TorrentProxy? {
    if (!TorrentWords.proxyOk(host, port)) return null
    return TorrentProxy(
        host = host.trim(),
        port = port.trim().toInt().toUShort(),
        user = user.trim().ifEmpty { null },
        password = password.takeIf { it.isNotEmpty() },
    )
}

/** Whether the fields still say the [saved] proxy. */
private fun sameAs(saved: ProxyView?, host: String, port: String, user: String): Boolean =
    saved != null && saved.host == host.trim() && saved.port.toString() == port.trim() && saved.user.orEmpty() == user.trim()

/** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
internal fun proxyLine(context: Context, host: String, port: String, user: String): String = TorrentWords.proxyLine(context, host, port, user)

/**
 * One place under the details for what the check says, [height] high whatever is in it so the
 * buttons never move: Checking the proxy while it runs, why it failed, or [otherwise]. TalkBack
 * says each change as it comes.
 */
@Composable
private fun Status(height: Dp, checking: Boolean, failed: String?, otherwise: String?) {
    Box(
        Modifier
            .fillMaxWidth()
            .height(height)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            checking -> CheckingLine()
            failed != null -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                Text(failed, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            otherwise != null -> Text(otherwise, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** Protect with's status: the check's three-line note, or what the check says in its place. */
private val PROTECT_STATUS = 64.dp

/** Edit proxy's status: the password's one line, or what the check says in its place. */
private val EDIT_STATUS = 52.dp

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
        Text(stringResource(TorrentWords.CHECKING_PROXY), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
    }
}

/** How long Checking the proxy waits before its spinner shows, as Checking the store does. */
private const val SPINNER_WAIT = 400L

/**
 * Protect with: the three protections as radio rows, the chosen one in the amber container. [first]
 * marks the chosen one, which the screen focuses as it opens.
 */
@Composable
private fun ProtectWith(chosen: TorrentProtection, first: FocusRequester, onPick: (TorrentProtection) -> Unit) {
    val context = LocalContext.current
    Column(Modifier.width(300.dp).selectableGroup(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(stringResource(R.string.torrents_protect_with), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, bottom = 4.dp))
        Protections.forEach { each ->
            TvListRow(
                TorrentWords.protection(context, each),
                onClick = { onPick(each) },
                supporting = TorrentWords.protectionNote(context, each),
                supportingLines = 2,
                height = 88.dp,
                selected = each == chosen,
                modifier = if (each == chosen) Modifier.focusRequester(first) else Modifier,
            )
        }
    }
}

/**
 * The TV's page for the details, behind a QR code: it takes one answer and closes, or closes
 * after its 10 minutes, "Code expired" then in the code's place. [opened] counts the pages asked
 * for; the form changes with the protection, and a new page opens with it.
 */
@Composable
private fun DetailsPage(form: PageForm, opened: Int, filled: Map<String, String> = emptyMap(), onAnswer: (PageAnswer.Torrent) -> Unit) {
    val context = LocalContext.current
    val host = LocalPageHost.current
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
        val page = SignInPage(context, tracker = "torrents", form = form, authorizeUrl = null, filled = filled) { sent ->
            if (sent is PageAnswer.Torrent) scope.launch { answer(sent) }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val local = host(context)
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
    Row(horizontalArrangement = Arrangement.spacedBy(20.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = address.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, size = QR, label = stringResource(if (form == PageForm.Proxy) R.string.tv_qr_label_proxy else R.string.tv_qr_label_index))
        } else {
            // The code's place kept while the page starts, so nothing moves when it comes.
            Box(Modifier.size(QR).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text(stringResource(R.string.stores_code_expired), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                stringResource(when (form) {
                    PageForm.TorrentProxy -> R.string.tv_scan_torrent_proxy
                    PageForm.Proxy -> R.string.tv_scan_proxy
                    else -> R.string.tv_scan_torrent
                }),
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Silk,
            )
            Text(
                when {
                    expired -> stringResource(R.string.stores_page_closed)
                    shown != null -> shown
                    started -> stringResource(R.string.stores_tv_no_network)
                    else -> stringResource(R.string.downloads_notice_starting)
                },
                style = MaterialTheme.typography.bodySmall,
                color = ObiSilk.Haze,
                // The whole address, its key included, so it can be typed as well as scanned.
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            if (!expired) Text(stringResource(R.string.stores_page_closes), style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
            if (expired) {
                TonalButton(onClick = { again += 1 }) { ButtonLabel(stringResource(R.string.action_new_code)) }
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
            Field(index, onIndex, stringResource(R.string.torrents_index), KeyboardType.Uri, focus = first)
            Text(TorrentWords.indexNote(LocalContext.current, index), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 24.dp))
        }
        if (proxy) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(host, onHost, stringResource(R.string.torrents_proxy_host), KeyboardType.Uri, Modifier.weight(2f), focus = first.takeIf { !withIndex })
                Field(port, onPort, stringResource(R.string.torrents_sheet_port), KeyboardType.Number, Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(user, onUser, stringResource(R.string.tv_field_user_optional), KeyboardType.Text, Modifier.weight(1f))
                Field(password, onPassword, stringResource(R.string.tv_field_password_optional), KeyboardType.Password, Modifier.weight(1f), secret = true)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            TonalButton(onClick = onDone) { ButtonLabel(stringResource(R.string.action_done)) }
        }
    }
}

/**
 * One typed field, Search's pill: the raised ground and its name in haze until something is
 * typed, silk with ground ink while focused. TalkBack says its name whatever it holds.
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
        if (text.isEmpty()) {
            // Drawn only while empty, so it is not what names the field: the field's own semantics do.
            Text(name, style = MaterialTheme.typography.bodyLarge, color = if (focused) ObiSilk.Ground else ObiSilk.Haze, maxLines = 1, modifier = Modifier.clearAndSetSemantics {})
        }
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
                .semantics { contentDescription = name }
                .let { if (focus != null) it.focusRequester(focus) else it }
                .onFocusChanged { focused = it.isFocused },
        )
    }
}

/** The QR code's side: smaller than sign-in's, as the protection rows sit beside it. */
private val QR = 140.dp
