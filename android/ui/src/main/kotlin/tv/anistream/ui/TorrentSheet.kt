// Turning torrents on (AndroidTorrentSheet): what protects their traffic, chosen before any
// torrent starts: a SOCKS5 proxy with its address, the VPN already running on the device, or no
// protection, which is confirmed first; and the torrent index, which anistream does not provide.
// The same sheet changes the protection later, from Settings › Sources. A new or changed proxy is
// saved only once the connection check passes through it, and a failed check saves nothing. The
// torrent index has a sheet of its own, and Edit proxy is the proxy's form, filled in.

package tv.anistream.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.R
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.ProxyView
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.engine.model.TorrentProxy
import tv.anistream.store.ProvidersModel
import tv.anistream.store.TorrentWords

/** Why the torrent sheet is open: torrents turning on, their protection or index changing, or the saved proxy's details. */
internal enum class TorrentSheetMode { TurnOn, Change, Index, EditProxy }

/**
 * The sheet. Turn on is the one amber action, and only once the choice is whole: an index, and a
 * proxy's host and port when the proxy is chosen. No protection asks to be confirmed as it is
 * picked.
 */
@Composable
internal fun TorrentSheet(mode: TorrentSheetMode, start: TorrentProtection? = null, onDismiss: () -> Unit) {
    when (mode) {
        TorrentSheetMode.EditProxy -> EditProxySheet(onDismiss)
        TorrentSheetMode.Index -> IndexSheet(onDismiss)
        TorrentSheetMode.TurnOn, TorrentSheetMode.Change -> ProtectSheet(mode, start, onDismiss)
    }
}

@Composable
private fun ProtectSheet(mode: TorrentSheetMode, start: TorrentProtection?, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val torrent = view?.torrent
    val saved = torrent?.proxy
    val savedProtection = torrent?.protection?.takeIf { mode == TorrentSheetMode.Change } ?: TorrentProtection.DEVICE
    var acknowledged by rememberSaveable { mutableStateOf(torrent?.protection == TorrentProtection.NONE) }
    // The protection picked in Settings' menu is where the sheet starts; no protection is confirmed first.
    var protection by rememberSaveable {
        mutableStateOf(start?.takeIf { it != TorrentProtection.NONE || acknowledged } ?: savedProtection)
    }
    var host by rememberSaveable { mutableStateOf(saved?.host.orEmpty()) }
    var port by rememberSaveable { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by rememberSaveable { mutableStateOf(saved?.user.orEmpty()) }
    // Never saved with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    var index by rememberSaveable { mutableStateOf(view?.sources?.index.orEmpty()) }
    var asking by remember { mutableStateOf(start == TorrentProtection.NONE && !acknowledged) }

    val trimmed = index.trim()
    val indexOk = TorrentWords.indexOk(trimmed)
    val proxy = protection == TorrentProtection.PROXY
    val ready = indexOk && (!proxy || TorrentWords.proxyOk(host, port))
    val indexNote = TorrentWords.indexNote(context, trimmed)
    val details = if (proxy) proxyOf(host, port, user, password) else null
    // A new or changed proxy is checked before anything is saved; the same saved one is not again.
    val checks = proxy && (mode == TorrentSheetMode.TurnOn || torrent?.protection != TorrentProtection.PROXY || !sameAs(saved, host, port, user) || password.isNotEmpty())
    val save = { checked: TorrentProxy? ->
        if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, tv.anistream.engine.core.SettingValue.Str(trimmed))
        store.setTorrentProtection(
            protection,
            proxy = checked,
            onFailure = torrent?.onFailure ?: FailedCheck.PAUSE,
            acknowledged = protection == TorrentProtection.NONE && acknowledged,
        )
        password = ""
        if (mode == TorrentSheetMode.TurnOn) store.setSwitch(SettingKey.TORRENTS, true)
        snackbars.show(context.getString(if (mode == TorrentSheetMode.TurnOn) TorrentWords.SAID_ON else TorrentWords.SAID_PROTECTION))
        onDismiss()
    }
    val check = proxyCheck(details.takeIf { checks }, onPassed = save)

    SettingsSheet(onDismiss, tall = true) {
        SheetTitle(stringResource(if (mode == TorrentSheetMode.TurnOn) R.string.torrents_sheet_turn_on_title else R.string.torrents_sheet_protect_title))
        SheetText(stringResource(TorrentWords.WHY))
        Text(
            stringResource(R.string.torrents_protect_with),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
        )
        Column(Modifier.selectableGroup()) {
            ProtectionChoice(TorrentWords.protection(context, TorrentProtection.PROXY), TorrentWords.protectionNote(context, TorrentProtection.PROXY), proxy, enabled = !check.checking) {
                protection = TorrentProtection.PROXY
            }
            if (proxy) {
                ProxyFields(
                    host, port, user, password,
                    onHost = { host = it }, onPort = { port = it }, onUser = { user = it }, onPassword = { password = it },
                    Modifier.padding(start = 48.dp, bottom = 8.dp),
                    enabled = !check.checking,
                )
            }
            ProtectionChoice(TorrentWords.protection(context, TorrentProtection.DEVICE), TorrentWords.protectionNote(context, TorrentProtection.DEVICE), protection == TorrentProtection.DEVICE, enabled = !check.checking) {
                protection = TorrentProtection.DEVICE
            }
            ProtectionChoice(TorrentWords.protection(context, TorrentProtection.NONE), TorrentWords.protectionNote(context, TorrentProtection.NONE), protection == TorrentProtection.NONE, enabled = !check.checking) {
                if (protection != TorrentProtection.NONE) {
                    if (acknowledged) protection = TorrentProtection.NONE else asking = true
                }
            }
        }
        Spacer(Modifier.heightIn(min = 16.dp))
        SheetField(
            index,
            { index = it },
            stringResource(R.string.torrents_index),
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            supporting = indexNote,
            error = !indexOk,
            enabled = !check.checking,
        )
        SheetText(
            checkNote(context, torrent?.onFailure, protection),
            Modifier.padding(top = 16.dp),
        )
        // Held for the check while a proxy is the choice, so the buttons stay where they are.
        if (proxy) {
            CheckStatus(
                checking = check.checking,
                failed = check.failed?.let {
                    TorrentWords.proxyFailed(context, it, if (mode == TorrentSheetMode.TurnOn) TorrentWords.Kept.TurningOn else TorrentWords.Kept.Protection)
                },
            )
        }
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(
                enabled = ready && !check.checking,
                onClick = { if (checks && details != null) store.checkTorrentProtection(TorrentProtection.PROXY, details) else save(details) },
            ) { Text(stringResource(if (mode == TorrentSheetMode.TurnOn) R.string.torrents_sheet_turn_on_button else R.string.action_save)) }
        }
    }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text(stringResource(TorrentWords.NONE_TITLE)) },
            text = { Text(stringResource(TorrentWords.NONE_BODY)) },
            confirmButton = {
                TextButton(onClick = {
                    asking = false
                    acknowledged = true
                    protection = TorrentProtection.NONE
                }) { Text(stringResource(R.string.torrents_protect_none)) }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { asking = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
}

/** The torrent index alone, from the Torrent index row. */
@Composable
private fun IndexSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    var index by rememberSaveable { mutableStateOf(view?.sources?.index.orEmpty()) }
    val trimmed = index.trim()
    val indexOk = TorrentWords.indexOk(trimmed)
    SettingsSheet(onDismiss) {
        SheetTitle(stringResource(R.string.torrents_index))
        SheetField(
            index,
            { index = it },
            stringResource(R.string.torrents_index),
            Modifier.padding(top = 16.dp),
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Done),
            supporting = TorrentWords.indexNote(context, trimmed),
            error = !indexOk,
        )
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(
                enabled = indexOk,
                onClick = {
                    if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, tv.anistream.engine.core.SettingValue.Str(trimmed))
                    snackbars.show(context.getString(TorrentWords.SAID_INDEX))
                    onDismiss()
                },
            ) { Text(stringResource(R.string.action_save)) }
        }
    }
}

/** A SOCKS5 proxy's host, port, user name and password, as the sheet and Edit proxy ask for them. */
@Composable
private fun ProxyFields(
    host: String,
    port: String,
    user: String,
    password: String,
    onHost: (String) -> Unit,
    onPort: (String) -> Unit,
    onUser: (String) -> Unit,
    onPassword: (String) -> Unit,
    modifier: Modifier = Modifier,
    note: String = stringResource(R.string.torrents_sheet_proxy_auth_note),
    enabled: Boolean = true,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SheetField(host, onHost, stringResource(R.string.torrents_sheet_host), Modifier.weight(2f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Next), enabled = enabled)
            SheetField(port, { onPort(it.filter(Char::isDigit).take(5)) }, stringResource(R.string.torrents_sheet_port), Modifier.weight(1f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next), enabled = enabled)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SheetField(user, onUser, stringResource(R.string.torrents_proxy_user), Modifier.weight(1f), keyboard = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Next), enabled = enabled)
            SheetField(password, onPassword, stringResource(R.string.torrents_proxy_password), Modifier.weight(1f), password = true, keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done), enabled = enabled)
        }
        SheetText(note)
    }
}

/**
 * Edit proxy: the saved proxy's form, filled in but for its password. Save checks the new details
 * first, and only a passed check saves them; a failed one says why and keeps the saved proxy.
 */
@Composable
private fun EditProxySheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved = view?.torrent?.proxy
    var host by rememberSaveable { mutableStateOf(saved?.host.orEmpty()) }
    var port by rememberSaveable { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by rememberSaveable { mutableStateOf(saved?.user.orEmpty()) }
    // Never saved with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    val details = proxyOf(host, port, user, password)
    val check = proxyCheck(details) { checked ->
        store.setTorrentProtection(TorrentProtection.PROXY, checked, view?.torrent?.onFailure ?: FailedCheck.PAUSE, acknowledged = false)
        password = ""
        snackbars.show(context.getString(TorrentWords.SAID_PROXY))
        onDismiss()
    }
    SettingsSheet(onDismiss) {
        SheetTitle(stringResource(R.string.torrents_sheet_edit_proxy_title))
        SheetText(stringResource(TorrentWords.EDIT_NOTE))
        ProxyFields(
            host, port, user, password,
            onHost = { host = it }, onPort = { port = it }, onUser = { user = it }, onPassword = { password = it },
            Modifier.padding(top = 16.dp),
            note = stringResource(TorrentWords.PASSWORD_KEPT),
            enabled = !check.checking,
        )
        CheckStatus(checking = check.checking, failed = check.failed?.let { TorrentWords.editFailed(context, it) })
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(
                enabled = details != null && !check.checking,
                onClick = { details?.let { store.checkTorrentProtection(TorrentProtection.PROXY, it) } },
            ) { Text(stringResource(R.string.action_save)) }
        }
    }
}

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

/**
 * The check's line, in a place held for two lines whatever is in it, so the sheet neither grows
 * nor moves its buttons: the ring and "Checking the proxy" while it runs, or the error glyph and
 * why it failed, with Reject. TalkBack says each as it comes.
 */
@Composable
private fun CheckStatus(checking: Boolean, failed: String?) {
    Box(
        Modifier
            .fillMaxWidth()
            .padding(top = 16.dp)
            .heightIn(min = STATUS_HEIGHT)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            checking -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                WorkRing(true)
                SheetText(stringResource(TorrentWords.CHECKING_PROXY))
            }
            failed != null -> {
                val haptics = LocalHapticFeedback.current
                LaunchedEffect(failed) { haptics.performHapticFeedback(HapticFeedbackType.Reject) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Problem, contentDescription = stringResource(R.string.error_label), tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp))
                    Text(failed, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurface)
                }
            }
        }
    }
}

/** Two lines of bodyMedium: the longest a failed check says. */
private val STATUS_HEIGHT = 40.dp

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

/** One protection: a radio in amber when chosen, its name and what it does. */
@Composable
private fun ProtectionChoice(title: String, note: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .selectable(selected = selected, enabled = enabled, role = Role.RadioButton, onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected = selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(selectedColor = MaterialTheme.colorScheme.primary, unselectedColor = MaterialTheme.colorScheme.onSurfaceVariant),
            modifier = Modifier.padding(horizontal = 12.dp),
        )
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            SheetText(note)
        }
    }
}

/**
 * Warn only, on a failed connection check, behind the same kind of confirmation as no
 * protection: torrents then keep running on what may be the viewer's own address.
 */
@Composable
internal fun WarnOnlyDialog(onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(TorrentWords.WARN_TITLE)) },
        text = { Text(stringResource(TorrentWords.WARN_BODY)) },
        confirmButton = { TextButton(onClick = onConfirm) { Text(stringResource(R.string.torrents_fail_warn)) } },
        dismissButton = { QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}
