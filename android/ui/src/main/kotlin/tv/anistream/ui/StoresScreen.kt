// Plugin stores: the stores joined, one row each; a store's page with its plugins and what it is
// (AndroidStore); a plugin's page, with the hosts it may contact; the update confirmation, which
// names the hosts an update adds; and the join sheet (AndroidJoinStore), opened empty or with an
// invite link the app was handed.

package tv.anistream.ui

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import kotlinx.coroutines.delay
import tv.anistream.design.tabular
import tv.anistream.engine.model.StorePluginRow
import tv.anistream.engine.model.StorePluginStatus
import tv.anistream.engine.model.StoreRow
import tv.anistream.engine.model.StoreState
import tv.anistream.store.InviteLinks
import tv.anistream.store.StoresModel
import tv.anistream.store.Wording
import tv.anistream.store.contactsLine
import tv.anistream.store.installedVersion
import tv.anistream.store.runByLine
import tv.anistream.store.standing
import tv.anistream.store.updates

internal const val STORES_ROUTE = "stores"

internal fun storeRoute(id: String) = "store/${android.net.Uri.encode(id)}"

internal fun pluginRoute(store: String, plugin: String) = "store/${android.net.Uri.encode(store)}/plugin/${android.net.Uri.encode(plugin)}"

/** The stores' routes: the list, a store's page and a plugin's. */
internal fun NavGraphBuilder.storePages() {
    composable(STORES_ROUTE) { StoresScreen() }
    composable("store/{id}") { entry ->
        StoreScreen(entry.arguments?.getString("id") ?: return@composable)
    }
    composable("store/{id}/plugin/{plugin}") { entry ->
        PluginScreen(
            entry.arguments?.getString("id") ?: return@composable,
            entry.arguments?.getString("plugin") ?: return@composable,
        )
    }
}

/** The stores joined, one row each with how it stands, and Join a store. */
@Composable
internal fun StoresScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadStores() }
    ScreenFrame(title = "Plugin stores", onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (stores.isEmpty()) {
                Column(Modifier.fillMaxWidth().padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    tv.anistream.design.Nagare(cell = 5)
                    Text("No plugin stores yet", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                    SheetText("Plugin stores are run by people you know. Join one with the invite link its owner sends you.", Modifier.padding(top = 4.dp, start = 24.dp, end = 24.dp))
                }
            } else {
                SettingsGroup(
                    rows = stores.map { row ->
                        @Composable { place: Place ->
                            InfoRow(
                                place,
                                row.name,
                                storeLine(row),
                                onClick = { navigator.open(storeRoute(row.id)) },
                            )
                        }
                    },
                )
            }
            Row(Modifier.padding(horizontal = 4.dp)) {
                Button(onClick = { store.stores.openJoin() }) { Text("Join a store") }
            }
            SettingsFootnote("Plugins come from a store's owner, not from anistream. $LEGAL_LINE")
        }
    }
}

/** A store's one line: its host, then what it has waiting or why it is not answering. */
private fun storeLine(row: StoreRow): String = listOfNotNull(
    row.host,
    when (val state = row.state) {
        is StoreState.Offline -> "Not answering"
        StoreState.Revoked -> "No longer lets this device in"
        StoreState.Ready -> row.updates.takeIf { it > 0 }?.let { Wording.plural(it, "update") }
    },
).joinToString(" · ")

/** A store's page: its plugins, each to install or update, and what the store is. */
@Composable
internal fun StoreScreen(id: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val row = stores.firstOrNull { it.id == id }
    var updating by remember { mutableStateOf<StorePluginRow?>(null) }
    var leaving by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { if (!store.stores.loaded.value) store.loadStores() }
    ScreenFrame(title = row?.name ?: "Plugin store", subtitle = row?.host, onBack = navigator::back) { padding ->
        row ?: return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when (val state = row.state) {
                is StoreState.Offline -> SettingsFootnote("Not answering: ${state.v1}. The plugins below are the last list that was checked.")
                StoreState.Revoked -> SettingsFootnote("This store no longer lets this device in. Its plugins stay installed until you remove them.")
                StoreState.Ready -> {}
            }
            SettingsGroup(
                title = "Plugins",
                footer = "Each plugin can contact only the hosts listed with it and cannot read your files.",
                rows = row.plugins.map { plugin ->
                    @Composable { place: Place ->
                        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
                        InfoRow(
                            place,
                            plugin.name,
                            plugin.contactsLine,
                            onClick = { navigator.open(pluginRoute(row.id, plugin.id)) },
                            trailing = { PluginAction(plugin, installing, onUpdate = { updating = plugin }) { store.installPlugin(row.id, plugin.id) } },
                        )
                    }
                },
            )
            SettingsGroup(
                title = "This store",
                footer = listOfNotNull(
                    if (row.anonymous) "Joined without a name" else "Joined as ${row.member}",
                    row.checkedAt?.let { "checked ${AccountWords.agoWords(System.currentTimeMillis() / 1000 - it)}" },
                ).joinToString(" · "),
                rows = buildList<@Composable (Place) -> Unit> {
                    add { place -> InfoRow(place, "Address", row.host) }
                    add { place -> InfoRow(place, "Key fingerprint", row.fingerprint) }
                    // Who can join is changed on the store's admin page, in a browser.
                    if (row.open) add { place -> InfoRow(place, "Who can join", "Anyone with the link") }
                },
            )
            FlowRow(Modifier.padding(horizontal = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // A device that joined with the store's link joins another the same way.
                if (!row.anonymous) {
                    TextButton(
                        onClick = { store.createInvite(row.id, name = null) },
                        enabled = !store.stores.isWorking(working, StoresModel.Work.Invite(row.id)),
                    ) { Text("Link another device", color = MaterialTheme.colorScheme.onSurface) }
                }
                TextButton(onClick = { leaving = true }) { Text("Leave store", color = MaterialTheme.colorScheme.error) }
            }
        }
    }
    updating?.let { plugin ->
        val status = plugin.status as? StorePluginStatus.Update
        UpdateDialog(
            plugin.name,
            storeName = row?.name ?: "the store",
            from = status?.from,
            to = plugin.version,
            newHosts = status?.newHosts.orEmpty(),
            onDismiss = { updating = null },
        ) {
            updating = null
            store.installPlugin(id, plugin.id)
        }
    }
    if (leaving && row != null) {
        AlertDialog(
            onDismissRequest = { leaving = false },
            title = { Text("Leave ${row.name}?") },
            text = { Text("Plugins installed from it stay installed. To join again, you need a new invite from its owner.") },
            confirmButton = {
                TextButton(onClick = {
                    leaving = false
                    store.leaveStore(row.id)
                    navigator.back()
                }) { Text("Leave", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { QuietButton("Cancel") { leaving = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
    InviteLinkSheetHost()
}

/** What a store's plugin offers: Installed with a water check, Update in amber, Install tonal. */
@Composable
private fun PluginAction(plugin: StorePluginRow, installing: Boolean, onUpdate: () -> Unit, onInstall: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    when (val status = plugin.status) {
        StorePluginStatus.Installed -> Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(18.dp))
            Text("Installed", style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
        }
        is StorePluginStatus.Update -> Button(onClick = onUpdate, enabled = !installing) { Text("Update") }
        StorePluginStatus.Available -> FilledTonalButton(onClick = onInstall, enabled = !installing) { Text(if (installing) "Installing" else "Install") }
        StorePluginStatus.AddedByHand -> FilledTonalButton(onClick = onUpdate, enabled = !installing) { Text("Replace") }
        is StorePluginStatus.OtherStore -> Text("From ${status.v1}", style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
    }
}

/** A store's plugin: its version, the hosts it may contact, and installing, updating or removing it. */
@Composable
internal fun PluginScreen(storeId: String, pluginId: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val row = stores.firstOrNull { it.id == storeId }
    val plugin = row?.plugins?.firstOrNull { it.id == pluginId }
    var updating by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    ScreenFrame(title = plugin?.name ?: "Plugin", subtitle = row?.let { "From ${it.name}" }, onBack = navigator::back) { padding ->
        if (row == null || plugin == null) return@ScreenFrame
        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
        val status = plugin.status
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(Modifier.padding(horizontal = 4.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                when (status) {
                    is StorePluginStatus.Update -> Button(onClick = { updating = true }, enabled = !installing) { Text("Update to ${plugin.version}") }
                    StorePluginStatus.Available -> Button(onClick = { store.installPlugin(row.id, plugin.id) }, enabled = !installing) { Text(if (installing) "Installing" else "Install") }
                    else -> {}
                }
                if (status is StorePluginStatus.Installed || status is StorePluginStatus.Update) {
                    TextButton(onClick = { removing = true }) { Text("Remove", color = MaterialTheme.colorScheme.onSurface) }
                }
            }
            SettingsGroup(
                footer = "${plugin.name} can contact only these hosts and cannot read your files.",
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            "Version",
                            when (status) {
                                is StorePluginStatus.Update -> "${status.from} · ${plugin.version} available"
                                StorePluginStatus.Available -> "${plugin.version}, not installed"
                                else -> plugin.installedVersion
                            },
                        )
                    },
                    { place -> InfoRow(place, "Contacts", plugin.hosts.ifEmpty { listOf("No hosts") }.joinToString("\n")) },
                ),
            )
        }
    }
    if (updating && plugin != null) {
        val status = plugin.status as? StorePluginStatus.Update
        UpdateDialog(plugin.name, row?.name ?: "the store", status?.from, plugin.version, status?.newHosts.orEmpty(), onDismiss = { updating = false }) {
            updating = false
            store.installPlugin(storeId, plugin.id)
        }
    }
    if (removing && plugin != null) {
        AlertDialog(
            onDismissRequest = { removing = false },
            title = { Text("Remove ${plugin.name}?") },
            text = { Text("It is deleted from this device. The store can install it again.") },
            confirmButton = {
                QuietButton("Remove") {
                    removing = false
                    store.removePlugin(plugin.id)
                }
            },
            dismissButton = { QuietButton("Cancel") { removing = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
}

/**
 * The join sheet (AndroidJoinStore): the invite link, checked against the store it names a moment
 * after it is pasted or typed, what it leads to, and Join store, the one amber action.
 */
@Composable
internal fun JoinStoreSheet(start: String, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val preview by store.stores.preview.collectAsStateWithLifecycle()
    val joined by store.stores.joined.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val deviceName by store.stores.deviceName.collectAsStateWithLifecycle()
    var link by rememberSaveable(start) { mutableStateOf(start) }
    val busy = store.stores.isWorking(working, StoresModel.Work.Join)
    LaunchedEffect(link) {
        delay(300)
        val checked = (store.stores.preview.value as? StoresModel.Preview.Store)?.store?.link
        when {
            !InviteLinks.looksLikeInvite(link) -> store.stores.clearPreview()
            // Checked already, as the sheet opened again with the same link.
            checked == link.trim() -> {}
            else -> store.previewStore(link)
        }
    }
    LaunchedEffect(joined) {
        val id = joined ?: return@LaunchedEffect
        store.stores.clearJoined()
        onDismiss()
        navigator.open(storeRoute(id))
    }
    val invited = (preview as? StoresModel.Preview.Store)?.store
    val clear: @Composable () -> Unit = { IconButton(onClick = { link = "" }) { Icon(Icons.Clear, contentDescription = "Clear") } }
    SettingsSheet(onDismiss, tall = true) {
        SheetTitle("Join a plugin store")
        SheetField(
            link,
            { link = it },
            "Invite link",
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            supporting = if (link.isEmpty()) "Paste the link the store's owner sent you." else null,
            trailing = clear.takeIf { link.isNotEmpty() },
        )
        when (val shown = preview) {
            is StoresModel.Preview.Checking -> SheetText("Checking the link", Modifier.padding(top = 16.dp))
            is StoresModel.Preview.Failed -> Text(
                shown.reason.replaceFirstChar { it.uppercase() } + ".",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 16.dp),
            )
            is StoresModel.Preview.Store -> InvitedStorePreview(shown.store)
            null -> {}
        }
        // What the store will call this device: its kind, never its own name, to change first.
        if (invited != null && !invited.anonymous) {
            SheetField(
                deviceName,
                store.stores::nameDevice,
                "Device name",
                modifier = Modifier.padding(top = 16.dp),
                keyboard = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                supporting = "The store's owner sees this device by this name.",
            )
        }
        HorizontalDivider(Modifier.padding(top = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
        SheetText("Plugins come from the store's owner, not from anistream. You can leave a store at any time.", Modifier.padding(top = 16.dp))
        SheetButtons {
            QuietButton("Cancel", onClick = onDismiss)
            Button(
                onClick = { store.joinStore(link) },
                enabled = invited != null && !invited.alreadyJoined && !busy,
            ) { Text(if (busy) "Joining" else "Join store") }
        }
    }
}

/**
 * What the link leads to (AndroidJoinStore): the store, who runs it and how many plugins it
 * offers, its key matched against the link's, and each plugin with what it may contact, where it
 * stands here and its version. For a store anyone with its link can join, that it is open, and
 * that its open link joins without a name. A store from before the preview shows its name and key.
 */
@Composable
private fun InvitedStorePreview(invited: tv.anistream.engine.model.InvitedStore) {
    val colors = MaterialTheme.colorScheme
    val tabular = MaterialTheme.typography.bodyMedium.tabular()
    HorizontalDivider(Modifier.padding(top = 16.dp), color = colors.outlineVariant)
    PreviewLine(invited.name, invited.runByLine, titleWeight = FontWeight.Medium) {
        if (invited.plugins.isNotEmpty()) {
            Text(Wording.plural(invited.plugins.size, "plugin"), style = tabular, color = colors.onSurfaceVariant)
        }
    }
    PreviewLine(
        if (invited.anonymous) "Key matches the link" else "Key matches the invite",
        invited.fingerprint,
        subtitleStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
    ) {
        Icon(Icons.Shield, contentDescription = "Verified", tint = colors.secondary)
    }
    if (invited.open) {
        PreviewLine(
            "Anyone with the link can join",
            if (invited.anonymous) "You join without a name. Another device joins with the same link." else null,
        )
    }
    if (invited.alreadyJoined) SheetText("This device has joined this store already.", Modifier.padding(vertical = 8.dp))
    if (invited.plugins.isNotEmpty()) {
        HorizontalDivider(color = colors.outlineVariant)
        invited.plugins.forEach { plugin ->
            PreviewLine(plugin.name, listOfNotNull(plugin.contactsLine, plugin.standing).joinToString(" · ")) {
                Text(plugin.version, style = tabular, color = colors.onSurfaceVariant)
            }
        }
    }
}

/** One line of the join sheet's preview: a title, a line under it in haze, and what sits at its end. */
@Composable
private fun PreviewLine(
    title: String,
    subtitle: String?,
    titleWeight: FontWeight? = null,
    subtitleStyle: androidx.compose.ui.text.TextStyle = MaterialTheme.typography.bodyMedium,
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, fontWeight = titleWeight, maxLines = 1, overflow = TextOverflow.Ellipsis)
            subtitle?.let { Text(it, style = subtitleStyle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
        trailing()
    }
}

/** An invite link for another device of yours, to share or copy, once the store has made it. */
@Composable
private fun InviteLinkSheetHost() {
    val store = LocalAppStore.current
    val invite by store.stores.invite.collectAsStateWithLifecycle()
    val shown = invite ?: return
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    SettingsSheet(onDismiss = store.stores::clearInvite) {
        SheetTitle("Link another device")
        SheetText("Open this link on your other device to join the store there. It works once.")
        Text(
            shown.link,
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            modifier = Modifier.padding(top = 16.dp),
        )
        SheetButtons {
            QuietButton("Copy link") { clipboard.setText(AnnotatedString(shown.link)) }
            Button(onClick = {
                val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, shown.link)
                context.startActivity(Intent.createChooser(send, null))
            }) { Text("Share") }
        }
    }
}

/** The join sheet over any screen, while the store has one open: empty, or with an invite link. */
@Composable
internal fun JoinStoreSheetHost() {
    val store = LocalAppStore.current
    val joining by store.stores.joining.collectAsStateWithLifecycle()
    val link = joining ?: return
    JoinStoreSheet(link) { store.stores.closeJoin() }
}
