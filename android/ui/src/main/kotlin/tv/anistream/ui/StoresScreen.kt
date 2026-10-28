// Plugin stores: the stores joined, one row each; a store's page with its plugins and what it is
// (AndroidStore); a plugin's page, with the hosts it may contact; the update confirmation, which
// names the hosts an update adds; and the join sheet (AndroidJoinStore), opened empty or with an
// invite link the app was handed.

package tv.anistream.ui

import tv.anistream.store.plainReason
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
    val loaded by store.stores.loaded.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadStores() }
    ScreenFrame(title = stringResource(R.string.stores_title), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (!loaded && stores.isEmpty()) {
                PlaceholderRows(2)
            } else if (stores.isEmpty()) {
                Column(Modifier.fillMaxWidth().padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    tv.anistream.design.Nagare(cell = 5)
                    Text(stringResource(R.string.stores_empty_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                    SheetText(stringResource(R.string.stores_empty_detail), Modifier.padding(top = 4.dp, start = 24.dp, end = 24.dp))
                }
            } else {
                SettingsGroup(
                    rows = stores.map { row ->
                        @Composable { place: Place ->
                            InfoRow(
                                place,
                                row.name,
                                storeLine(LocalContext.current, row, offline),
                                onClick = { navigator.open(storeRoute(row.id)) },
                            )
                        }
                    },
                )
            }
            Row(Modifier.padding(horizontal = 4.dp)) {
                Button(onClick = { store.stores.openJoin() }) { Text(stringResource(R.string.stores_join_short)) }
            }
            SettingsFootnote(stringResource(R.string.stores_not_from_app))
        }
    }
}

/** A store's one line: its host, then what it has waiting or why it is not answering. */
private fun storeLine(context: Context, row: StoreRow, offline: Boolean): String = listOfNotNull(
    row.host,
    when (val state = row.state) {
        // Offline, no store is said to be failing: the one offline line says why.
        is StoreState.Offline -> if (offline) null else Strings.storesStatusNotAnswering(context)
        StoreState.Revoked -> Strings.storesStatusAccessEnded(context)
        StoreState.Ready -> row.updates.takeIf { it > 0 }?.let { context.resources.getQuantityString(R.plurals.stores_updates_waiting, it, it) }
    },
).joinToString(" · ") // i18n-skip

/** A store's page: its plugins, each to install or update, and what the store is. */
@Composable
internal fun StoreScreen(id: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val row = stores.firstOrNull { it.id == id }
    val offline by store.offline.collectAsStateWithLifecycle()
    var updating by remember { mutableStateOf<StorePluginRow?>(null) }
    var leaving by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { if (!store.stores.loaded.value) store.loadStores() }
    ScreenFrame(title = row?.name ?: stringResource(R.string.stores_fallback_title), subtitle = row?.host, onBack = navigator::back) { padding ->
        row ?: return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when (val state = row.state) {
                is StoreState.Offline -> if (!offline) SettingsFootnote(stringResource(R.string.stores_offline_last_list, plainReason(LocalContext.current, state.v1).trimEnd('.')))
                StoreState.Revoked -> SettingsFootnote(stringResource(R.string.stores_access_ended_page))
                StoreState.Ready -> {}
            }
            SettingsGroup(
                title = stringResource(R.string.stores_plugins),
                footer = stringResource(R.string.stores_plugin_limits),
                rows = row.plugins.map { plugin ->
                    @Composable { place: Place ->
                        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
                        InfoRow(
                            place,
                            plugin.name,
                            plugin.contactsLine(LocalContext.current),
                            onClick = { navigator.open(pluginRoute(row.id, plugin.id)) },
                            trailing = { PluginAction(plugin, installing, onUpdate = { updating = plugin }) { store.installPlugin(row.id, plugin.id) } },
                        )
                    }
                },
            )
            SettingsGroup(
                title = stringResource(R.string.stores_this_store),
                footer = listOfNotNull(
                    if (row.anonymous) stringResource(R.string.stores_joined_nameless) else stringResource(R.string.stores_device_joined_as, row.member),
                    row.checkedAt?.let { stringResource(R.string.stores_checked_ago, AccountWords.agoWords(LocalContext.current, System.currentTimeMillis() / 1000 - it)) },
                ).joinToString(" · "), // i18n-skip
                rows = buildList<@Composable (Place) -> Unit> {
                    add { place -> InfoRow(place, stringResource(R.string.stores_address), row.host) }
                    add { place -> InfoRow(place, stringResource(R.string.stores_key_fingerprint), row.fingerprint) }
                    // Who can join is changed on the store's admin page, in a browser.
                    if (row.open) add { place -> InfoRow(place, stringResource(R.string.stores_who_can_join), stringResource(R.string.stores_anyone_short)) }
                },
            )
            FlowRow(Modifier.padding(horizontal = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // A device that joined with the store's link joins another the same way.
                if (!row.anonymous) {
                    TextButton(
                        onClick = { store.createInvite(row.id, name = null) },
                        enabled = !store.stores.isWorking(working, StoresModel.Work.Invite(row.id)),
                    ) { Text(stringResource(R.string.stores_link_device_title), color = MaterialTheme.colorScheme.onSurface) }
                }
                // Destructive, so confirmed; in silk, as vermilion is only for a problem.
                TextButton(onClick = { leaving = true }) { Text(stringResource(R.string.stores_leave), color = MaterialTheme.colorScheme.onSurface) }
            }
        }
    }
    updating?.let { plugin ->
        val status = plugin.status as? StorePluginStatus.Update
        UpdateDialog(
            plugin.name,
            storeName = row?.name ?: stringResource(R.string.stores_fallback_name),
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
            title = { Text(stringResource(R.string.stores_leave_title, row.name)) },
            text = { Text(stringResource(R.string.stores_leave_rejoin)) },
            confirmButton = {
                TextButton(onClick = {
                    leaving = false
                    store.leaveStore(row.id)
                    navigator.back()
                }) { Text(stringResource(R.string.action_leave), color = MaterialTheme.colorScheme.onSurface) }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { leaving = false } },
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
            Text(stringResource(R.string.stores_plugin_state_installed), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
        }
        is StorePluginStatus.Update -> Button(onClick = onUpdate, enabled = !installing) { Text(stringResource(R.string.action_update)) }
        StorePluginStatus.Available -> FilledTonalButton(onClick = onInstall, enabled = !installing) { WorkLabel(stringResource(R.string.action_install), installing) }
        StorePluginStatus.AddedByHand -> FilledTonalButton(onClick = onUpdate, enabled = !installing) { Text(stringResource(R.string.action_replace)) }
        is StorePluginStatus.OtherStore -> Text(stringResource(R.string.stores_plugin_from, status.v1), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
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
    ScreenFrame(title = plugin?.name ?: stringResource(R.string.stores_fallback_plugin), subtitle = row?.let { stringResource(R.string.stores_plugin_from, it.name) }, onBack = navigator::back) { padding ->
        if (row == null || plugin == null) return@ScreenFrame
        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
        val status = plugin.status
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(Modifier.padding(horizontal = 4.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                when (status) {
                    is StorePluginStatus.Update -> Button(onClick = { updating = true }, enabled = !installing) { Text(stringResource(R.string.updates_update_to, plugin.version)) }
                    StorePluginStatus.Available -> Button(onClick = { store.installPlugin(row.id, plugin.id) }, enabled = !installing) { WorkLabel(stringResource(R.string.action_install), installing) }
                    else -> {}
                }
                if (status is StorePluginStatus.Installed || status is StorePluginStatus.Update) {
                    TextButton(onClick = { removing = true }) { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.onSurface) }
                }
            }
            SettingsGroup(
                footer = stringResource(R.string.stores_plugin_limits),
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            stringResource(R.string.sources_version),
                            when (status) {
                                is StorePluginStatus.Update -> stringResource(R.string.stores_plugin_update_line, status.from, plugin.version)
                                StorePluginStatus.Available -> plugin.version
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
        UpdateDialog(plugin.name, row?.name ?: stringResource(R.string.stores_fallback_name), status?.from, plugin.version, status?.newHosts.orEmpty(), onDismiss = { updating = false }) {
            updating = false
            store.installPlugin(storeId, plugin.id)
        }
    }
    if (removing && plugin != null) {
        AlertDialog(
            onDismissRequest = { removing = false },
            title = { Text(stringResource(R.string.stores_remove_plugin_title, plugin.name)) },
            text = { Text(stringResource(R.string.stores_remove_plugin_detail)) },
            confirmButton = {
                QuietButton(stringResource(R.string.action_remove)) {
                    removing = false
                    store.removePlugin(plugin.id)
                }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { removing = false } },
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
    val offline by store.offline.collectAsStateWithLifecycle()
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
    val clear: @Composable () -> Unit = { IconButton(onClick = { link = "" }) { Icon(Icons.Clear, contentDescription = stringResource(R.string.action_clear)) } }
    SettingsSheet(onDismiss, tall = true) {
        SheetTitle(stringResource(R.string.stores_join_title))
        SheetField(
            link,
            { link = it },
            stringResource(R.string.stores_invite_link),
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            supporting = if (link.isEmpty()) stringResource(R.string.stores_join_detail) else null,
            trailing = clear.takeIf { link.isNotEmpty() },
        )
        when (val shown = preview) {
            is StoresModel.Preview.Checking -> Row(
                Modifier.padding(top = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                WorkRing(true)
                SheetText(stringResource(R.string.stores_checking_link))
            }
            is StoresModel.Preview.Failed -> {
                // The link was refused in place: the error glyph, the reason in silk, and Reject.
                val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
                LaunchedEffect(shown) { haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.Reject) }
                Row(
                    Modifier.padding(top = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Problem, contentDescription = stringResource(R.string.error_label), tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp))
                    Text(
                        if (offline) stringResource(R.string.state_offline_line) else plainReason(LocalContext.current, shown.reason),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
            is StoresModel.Preview.Store -> InvitedStorePreview(shown.store)
            null -> {}
        }
        // What the store will call this device: its kind, never its own name, to change first.
        if (invited != null && !invited.anonymous) {
            SheetField(
                deviceName,
                store.stores::nameDevice,
                stringResource(R.string.stores_device_name),
                modifier = Modifier.padding(top = 16.dp),
                keyboard = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                supporting = stringResource(R.string.stores_device_name_detail),
            )
        }
        HorizontalDivider(Modifier.padding(top = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
        SheetText(stringResource(R.string.stores_not_from_app), Modifier.padding(top = 16.dp))
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(
                onClick = { store.joinStore(link) },
                enabled = invited != null && !invited.alreadyJoined && !busy,
            ) { WorkLabel(stringResource(R.string.stores_join_button), busy) }
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
    PreviewLine(invited.name, invited.runByLine(LocalContext.current), titleWeight = FontWeight.Medium) {
        if (invited.plugins.isNotEmpty()) {
            Text(pluralStringResource(R.plurals.stores_offers_plugins, invited.plugins.size, invited.plugins.size), style = tabular, color = colors.onSurfaceVariant)
        }
    }
    PreviewLine(
        stringResource(if (invited.anonymous) R.string.stores_preview_key_link else R.string.stores_preview_key_invite),
        invited.fingerprint,
        subtitleStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
    ) {
        Icon(Icons.Shield, contentDescription = stringResource(R.string.stores_key_verified), tint = colors.secondary)
    }
    if (invited.open) {
        PreviewLine(
            stringResource(R.string.stores_anyone_with_link),
            if (invited.anonymous) stringResource(R.string.stores_anyone_note) else null,
        )
    }
    if (invited.alreadyJoined) SheetText(stringResource(R.string.stores_already_joined_note), Modifier.padding(vertical = 8.dp))
    if (invited.plugins.isNotEmpty()) {
        HorizontalDivider(color = colors.outlineVariant)
        invited.plugins.forEach { plugin ->
            val context = LocalContext.current
            val line = listOfNotNull(plugin.contactsLine(context), plugin.standing(context)).joinToString(" · ") // i18n-skip
            PreviewLine(plugin.name, line) {
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
        SheetTitle(stringResource(R.string.stores_link_device_title))
        SheetText(stringResource(R.string.stores_link_sheet_detail))
        Text(
            shown.link,
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            modifier = Modifier.padding(top = 16.dp),
        )
        SheetButtons {
            QuietButton(stringResource(R.string.stores_copy_link)) { clipboard.setText(AnnotatedString(shown.link)) }
            Button(onClick = {
                val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, shown.link)
                context.startActivity(Intent.createChooser(send, null))
            }) { Text(stringResource(R.string.action_share)) }
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
