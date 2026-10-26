// Turning torrents on (AndroidTorrentSheet): what protects their traffic, chosen before any
// torrent starts: a SOCKS5 proxy with its address, the VPN already running on the device, or no
// protection, which is confirmed first; and the torrent index, which anistream ships none of. The
// same sheet changes the protection later, from Settings › Sources.

package tv.anistream.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.engine.model.TorrentProxy
import tv.anistream.store.TorrentWords

/** Why the torrent sheet is open: torrents turning on, or their protection changing. */
internal enum class TorrentSheetMode { TurnOn, Change }

/**
 * The sheet. Turn on is the one amber action, and only once the choice is whole: an index, and a
 * proxy's host and port when the proxy is chosen. No protection asks to be confirmed as it is
 * picked.
 */
@Composable
internal fun TorrentSheet(mode: TorrentSheetMode, start: TorrentProtection? = null, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
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
    val proxyOk = TorrentWords.proxyOk(host, port)
    val ready = indexOk && (protection != TorrentProtection.PROXY || proxyOk)
    val indexNote = TorrentWords.indexNote(trimmed)

    SettingsSheet(onDismiss, tall = true) {
        SheetTitle(if (mode == TorrentSheetMode.TurnOn) "Turn on torrents" else "Protect torrent traffic")
        SheetText(TorrentWords.WHY)
        Text(
            "Protect with",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
        )
        Column(Modifier.selectableGroup()) {
            ProtectionChoice(TorrentWords.protection(TorrentProtection.PROXY), TorrentWords.protectionNote(TorrentProtection.PROXY, deviceWord()), protection == TorrentProtection.PROXY) {
                protection = TorrentProtection.PROXY
            }
            if (protection == TorrentProtection.PROXY) {
                Column(Modifier.padding(start = 48.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SheetField(host, { host = it }, "Host", Modifier.weight(2f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Next))
                        SheetField(port, { port = it.filter(Char::isDigit).take(5) }, "Port", Modifier.weight(1f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next))
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SheetField(user, { user = it }, "User name", Modifier.weight(1f), keyboard = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Next))
                        SheetField(password, { password = it }, "Password", Modifier.weight(1f), password = true, keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done))
                    }
                    SheetText("User name and password only if the proxy asks for them.")
                }
            }
            ProtectionChoice(TorrentWords.protection(TorrentProtection.DEVICE), TorrentWords.protectionNote(TorrentProtection.DEVICE, deviceWord()), protection == TorrentProtection.DEVICE) {
                protection = TorrentProtection.DEVICE
            }
            ProtectionChoice(TorrentWords.protection(TorrentProtection.NONE), TorrentWords.protectionNote(TorrentProtection.NONE, deviceWord()), protection == TorrentProtection.NONE) {
                if (protection != TorrentProtection.NONE) {
                    if (acknowledged) protection = TorrentProtection.NONE else asking = true
                }
            }
        }
        Spacer(Modifier.heightIn(min = 16.dp))
        SheetField(
            index,
            { index = it },
            "Torrent index",
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            supporting = indexNote,
            error = !indexOk,
        )
        SheetText(
            checkNote(torrent?.onFailure, protection),
            Modifier.padding(top = 16.dp),
        )
        SheetButtons {
            QuietButton("Cancel", onClick = onDismiss)
            Button(
                enabled = ready,
                onClick = {
                    if (trimmed != view?.sources?.index) store.setSetting(SettingKey.TORRENT_INDEX, tv.anistream.engine.core.SettingValue.Str(trimmed))
                    store.setTorrentProtection(
                        protection,
                        proxy = if (protection == TorrentProtection.PROXY) {
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
                    if (mode == TorrentSheetMode.TurnOn) store.setSwitch(SettingKey.TORRENTS, true)
                    onDismiss()
                },
            ) { Text(if (mode == TorrentSheetMode.TurnOn) "Turn on" else "Save") }
        }
    }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text(TorrentWords.NONE_TITLE) },
            text = { Text(TorrentWords.NONE_BODY) },
            confirmButton = {
                TextButton(onClick = {
                    asking = false
                    acknowledged = true
                    protection = TorrentProtection.NONE
                }) { Text("No protection") }
            },
            dismissButton = { QuietButton("Cancel") { asking = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
}

/** One protection: a radio in amber when chosen, its name and what it does. */
@Composable
private fun ProtectionChoice(title: String, note: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick),
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
        title = { Text(TorrentWords.WARN_TITLE) },
        text = { Text(TorrentWords.WARN_BODY) },
        confirmButton = { TextButton(onClick = onConfirm) { Text("Warn only") } },
        dismissButton = { QuietButton("Cancel", onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}
