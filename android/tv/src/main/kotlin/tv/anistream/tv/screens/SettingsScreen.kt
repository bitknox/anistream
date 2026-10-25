// Settings in two panes, as Android TV's own: sections on the left, the section's rows on the
// right. Moving through the sections shows their rows; Right goes into them.

package tv.anistream.tv.screens

import android.provider.Settings as SystemSettings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.launch
import tv.anistream.design.Nagare
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.ProviderRow
import tv.anistream.engine.model.StoreRow
import tv.anistream.engine.model.StoreState
import tv.anistream.store.ProviderStanding
import tv.anistream.store.standing
import tv.anistream.engine.ffi.FfiPairedPhone
import tv.anistream.engine.model.SettingChange
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.SettingsView
import tv.anistream.engine.model.SyncState
import tv.anistream.store.AccountsModel
import tv.anistream.store.AppStore
import tv.anistream.store.SettingsCatalog
import tv.anistream.store.UpdateCheck
import tv.anistream.store.UpdateState
import tv.anistream.store.Wording
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.TitleLanguage
import tv.anistream.tv.ui.plainReason
import tv.anistream.tv.home.LocalWatchNext
import tv.anistream.tv.remote.ConfirmDialog
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.remote.RemovePhoneDialog
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.ActionSheet
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.CardAction
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.focusSoon
import tv.anistream.tv.ui.EnterFocus
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.SafeSpec
import tv.anistream.tv.ui.ScrollRule
import tv.anistream.tv.ui.TvIcons
import tv.anistream.tv.ui.TvListRow
import tv.anistream.tv.ui.firstFocus
import tv.anistream.tv.ui.remembered

/** The sections, top to bottom. */
enum class SettingsSection(val label: String) {
    Playback("Playback"),
    Sources("Sources"),
    PluginStores("Plugin stores"),
    Accounts("Accounts"),
    Remotes("Remotes"),
    ThisTv("This TV"),
    About("About"),
}

/** Playback's rows: the phone's, less what does not apply on a TV (upscaling, fullscreen). */
val PlaybackRows: List<Pair<SettingKey, String>> = listOf(
    SettingKey.TRANSLATION to "Translation",
    SettingKey.SUBTITLES to "Subtitles",
    SettingKey.QUALITY to "Quality",
    SettingKey.AUTO_NEXT to "Auto-play next",
    SettingKey.SKIP_OPENING to "Skip opening automatically",
    SettingKey.SKIP_FILLER to "Skip filler automatically",
    SettingKey.MARK_CHAPTERS to "Mark opening and ending",
    SettingKey.COMMIT_THRESHOLD to "Counts as watched at",
    SettingKey.SPOILER_PROTECTION to "Spoiler protection",
    SettingKey.NIGHT_MODE to "Night mode",
)

/** The trackers, in the phone's order: their engine ids and names. */
private val Trackers = listOf(
    "anilist" to "AniList",
    "mal" to "MyAnimeList",
    "kitsu" to "Kitsu",
    "simkl" to "Simkl",
    "trakt" to "Trakt",
)

@Composable
fun SettingsScreen(nav: TvNavigator, memory: FocusMemory, start: SettingsSection = SettingsSection.Playback) {
    val store = LocalAppStore.current
    var section by rememberSaveable { mutableStateOf(start) }
    val view by store.settings.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.loadStores()
        // What the title language's line says of AniList's choice.
        store.loadTitleLanguage()
    }
    ToTop(nav, memory, scrolled = { false }) {}

    Row(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY, end = Canvas.SafeX)) {
        Column(Modifier.width(256.dp).fillMaxHeight().focusRestorer(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("Settings", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 16.dp).semantics { heading() })
            SettingsSection.entries.forEach { each ->
                TvListRow(
                    each.label,
                    onClick = {},
                    selected = each == section,
                    modifier = Modifier
                        .onFocusChanged { if (it.isFocused) section = each }
                        .firstFocus(memory, "section/${each.name}", each == start),
                )
            }
        }
        Spacer(Modifier.width(32.dp))
        val density = LocalDensity.current
        val spec = remember(density) { with(density) { SafeSpec(Canvas.FocusRoom.toPx(), 40.dp.toPx()) } }
        // Coming over from the sections, focus goes back to the row it left in this section, or to
        // the section's first row, never to whichever row happens to sit level with the section.
        val firstRow = remember { FocusRequester() }
        val first = Modifier.focusRequester(firstRow)
        ScrollRule(spec) {
            LazyColumn(
                state = rememberLazyListState(),
                modifier = Modifier.fillMaxHeight().padding(top = 56.dp).focusRestorer(firstRow),
                contentPadding = PaddingValues(bottom = 60.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                when (section) {
                    SettingsSection.Playback -> playback(store, view, memory, first)
                    SettingsSection.Sources -> sources(memory, first)
                    SettingsSection.PluginStores -> pluginStores(nav, memory, first)
                    SettingsSection.Accounts -> accounts(nav, memory, first)
                    SettingsSection.Remotes -> remotes(memory, first)
                    SettingsSection.ThisTv -> thisTv(memory, first)
                    SettingsSection.About -> about(nav, memory, first)
                }
            }
        }
    }
    EnterFocus(memory)
}

private fun LazyListScope.playback(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    item(key = "titles", contentType = "setting") { TitleLanguageRow(store, view, memory, first) }
    PlaybackRows.forEachIndexed { index, (key, label) ->
        item(key = key.name, contentType = "setting") {
            val actions = LocalActions.current
            val row = store.settings.row(key, view)
            val switch = SettingsCatalog.isSwitch(key)
            val overridden = store.settings.overridden(key, view)
            val options = SettingsCatalog.options(key)
            TvListRow(
                label,
                onClick = {
                    // A row with choices opens its radio panel; the choice applies at once.
                    if (!switch && options.isNotEmpty() && actions != null) {
                        val current = SettingsCatalog.current(row)
                        actions.open(ActionSheet(label, options.map { option ->
                            CardAction(option.label, selected = option == current) { store.setSetting(key, option.value) }
                        }))
                    } else {
                        change(store, key, row?.change, switch, view)
                    }
                },
                supporting = overridden,
                value = if (switch) null else row?.let { SettingsCatalog.current(it)?.label ?: it.value },
                switch = if (switch) store.settings.isOn(key, view) else null,
                enabled = SettingsCatalog.editable(row),
                modifier = Modifier.remembered(memory, "row/${key.name}"),
            )
        }
    }
}

/**
 * Title language, Playback's first row as on the phone: AniList's choice, English or Romaji, in a
 * radio panel, with a line under the label saying what the choice does now.
 */
@Composable
private fun TitleLanguageRow(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    val actions = LocalActions.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val viewer by store.accounts.viewerTitleLanguage.collectAsStateWithLifecycle()
    val signedIn = trackers["anilist"]?.connected == true
    val row = store.settings.row(SettingKey.TITLES, view)
    val choice = TitleChoice.of(SettingsCatalog.current(row)?.value)
    TvListRow(
        "Title language",
        onClick = {
            actions?.open(
                ActionSheet(
                    "Title language",
                    TitleChoice.entries.map { each ->
                        CardAction(each.label, selected = each == choice) {
                            if (each != choice) store.setSetting(SettingKey.TITLES, SettingValue.Str(each.value))
                        }
                    },
                ),
            )
        },
        supporting = TitleChoice.line(choice, signedIn, viewer),
        value = choice.label,
        enabled = row != null,
        modifier = first.remembered(memory, "row/titles"),
    )
}

/** The three forms a title can be named in, as the settings file names them. */
enum class TitleChoice(val value: String, val label: String) {
    AniList("anilist", "AniList's choice"),
    English("english", "English"),
    Romaji("romaji", "Romaji"),
    ;

    companion object {
        fun of(value: SettingValue?): TitleChoice = entries.firstOrNull { (value as? SettingValue.Str)?.v1 == it.value } ?: AniList

        /** The line under the label: what the choice does now, the phone's words. */
        fun line(choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String = when (choice) {
            AniList -> when {
                !signedIn -> "English while signed out of AniList"
                viewer != null -> "${name(viewer)}, from your AniList account"
                else -> "Follows your AniList account"
            }
            English -> "Romaji where there is no English title"
            Romaji -> "Japanese titles in Latin letters"
        }

        private fun name(language: TitleLanguage): String = when (language) {
            TitleLanguage.ROMAJI -> "Romaji"
            TitleLanguage.ENGLISH -> "English"
            TitleLanguage.NATIVE -> "Native"
        }
    }
}

/** A switch flips; a ladder steps forward, and from its end back to its start. */
private fun change(store: AppStore, key: SettingKey, change: SettingChange?, switch: Boolean, view: SettingsView?) {
    if (switch) {
        store.setSwitch(key, !store.settings.isOn(key, view))
        return
    }
    val steps = change as? SettingChange.Steps ?: return
    if (steps.forward) {
        store.stepSetting(key, 1)
    } else {
        val options = SettingsCatalog.options(key).size
        store.stepSetting(key, -maxOf(1, options - 1).toLong())
    }
}

private fun LazyListScope.sources(memory: FocusMemory, first: Modifier) {
    item(key = "sources", contentType = "list") {
        val store = LocalAppStore.current
        val rows by store.providers.rows.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (rows.isEmpty()) {
                Quiet("No sources yet. Plugins from a plugin store add them.")
            }
            val actions = LocalActions.current
            rows.forEachIndexed { index, provider ->
                TvListRow(
                    provider.name.ifEmpty { provider.id },
                    onClick = { actions?.open(providerSheet(provider)) },
                    supporting = providerOrigin(provider),
                    // A word only for a problem.
                    value = providerProblem(provider),
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "provider/${provider.id}"),
                )
            }
        }
    }
}

private fun LazyListScope.pluginStores(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "stores", contentType = "list") {
        val store = LocalAppStore.current
        val actions = LocalActions.current
        val stores by store.stores.stores.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            stores.forEachIndexed { index, row ->
                TvListRow(
                    row.name,
                    onClick = { actions?.open(storeSheet(store, row)) },
                    supporting = storeLine(row),
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "store/${row.id}"),
                )
            }
            TvListRow(
                "Add a plugin store",
                onClick = nav::openJoinStore,
                modifier = (if (stores.isEmpty()) first else Modifier).remembered(memory, "store/add"),
            )
        }
    }
}

/** Where a source came from: its store, by hand, or built in. */
private fun providerOrigin(row: ProviderRow): String = when (val origin = row.origin) {
    is ProviderOrigin.Store -> origin.name ?: "A store you left"
    ProviderOrigin.ByHand -> "Added by hand"
    ProviderOrigin.BuiltIn -> "Built in"
}

/** A source's word, only for a problem: "Not answering", "Held back". */
private fun providerProblem(row: ProviderRow): String? = when (row.standing) {
    ProviderStanding.Failing -> "Not answering"
    ProviderStanding.HeldBack -> "Held back"
    else -> null
}

/** A source's page as a panel: its version, its hosts and how fast it answers. */
private fun providerSheet(row: ProviderRow): ActionSheet = ActionSheet(
    row.name.ifEmpty { row.id },
    body = {
        InfoLines(
            listOfNotNull(
                "From" to providerOrigin(row),
                row.version?.let { "Version" to it },
                row.hosts.takeIf { it.isNotEmpty() }?.let { "Contacts" to it.joinToString(", ") },
                row.latencyMs?.let { "Answers in" to "$it ms" },
                providerProblem(row)?.let { "State" to it },
            ),
        )
    },
)

/** A store's line: its host, then why it is not answering. */
private fun storeLine(row: StoreRow): String = listOfNotNull(
    row.host,
    when (row.state) {
        is StoreState.Offline -> "Not answering"
        StoreState.Revoked -> "No longer lets this TV in"
        StoreState.Ready -> null
    },
).joinToString(" · ")

/** A store's page as a panel: its plugins with their hosts, its key, and Leave. */
private fun storeSheet(store: AppStore, row: StoreRow): ActionSheet = ActionSheet(
    row.name,
    body = {
        InfoLines(
            listOf("Address" to row.host, "Key" to row.fingerprint) +
                row.plugins.map { plugin -> plugin.name to listOf("Version ${plugin.version}", plugin.hosts.joinToString(", ")).filter { it.isNotEmpty() }.joinToString(" · ") },
            action = "Leave this store",
            onAction = { store.leaveStore(row.id) },
        )
    },
)

private fun LazyListScope.accounts(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "accounts", contentType = "list") {
        val store = LocalAppStore.current
        val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
        var signingOut by remember { mutableStateOf<String?>(null) }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Trackers.forEachIndexed { index, (id, name) ->
                TvListRow(
                    name,
                    onClick = { if (trackers[id]?.connected == true) signingOut = id else nav.openSignIn(id) },
                    supporting = accountLine(trackers[id]),
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "account/$id"),
                )
            }
        }
        signingOut?.let { id ->
            val name = AccountsModel.name(id)
            ConfirmDialog(
                title = "Sign out of $name?",
                body = "This TV stops syncing with $name. Your list stays on $name.",
                action = "Sign out",
                onConfirm = {
                    store.disconnect(id)
                    signingOut = null
                },
                onCancel = { signingOut = null },
            )
        }
    }
}

/** "Signed in as Mika", or "Not signed in". */
fun accountLine(state: SyncState?): String = when {
    state == null || !state.connected -> "Not signed in"
    state.user != null -> "Signed in as ${state.user}"
    else -> "Signed in"
}

private fun LazyListScope.remotes(memory: FocusMemory, first: Modifier) {
    item(key = "remotes", contentType = "list") {
        val remote = LocalRemoteHost.current
        val context = LocalContext.current
        val name = remember {
            remote?.name
                ?: runCatching { SystemSettings.Global.getString(context.contentResolver, SystemSettings.Global.DEVICE_NAME) }.getOrNull()
                ?: "This TV"
        }
        var removing by remember { mutableStateOf<FfiPairedPhone?>(null) }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            InfoRow("This TV's name for phones", name)
            TvListRow(
                "Pair a phone",
                onClick = { remote?.openPairing() },
                enabled = remote != null,
                modifier = first.remembered(memory, "remote/pair"),
            )
            TvListRow(
                "Let phones on this network find this TV",
                onClick = { remote?.let { it.allowFinding(!it.findable) } },
                switch = remote?.findable ?: false,
                enabled = remote != null,
                modifier = Modifier.remembered(memory, "remote/find"),
            )
            remote?.phones?.forEach { phone ->
                TvListRow(
                    phone.name,
                    onClick = { removing = phone },
                    supporting = phoneLine(phone, System.currentTimeMillis() / 1000),
                    icon = TvIcons.Phone,
                    modifier = Modifier.remembered(memory, "remote/phone/${phone.fingerprint}"),
                )
            }
        }
        removing?.let { phone ->
            RemovePhoneDialog(
                phone.name,
                onRemove = {
                    remote?.remove(phone)
                    removing = null
                    memory.focus("remote/pair")
                },
                onCancel = { removing = null },
            )
        }
    }
}

/** "Used today", "Used 3 days ago", or when it paired if it never connected again. */
fun phoneLine(phone: FfiPairedPhone, now: Long): String {
    val days = { at: Long -> ((now - at).coerceAtLeast(0) / 86_400).toInt() }
    val ago = { d: Int ->
        when (d) {
            0 -> "today"
            1 -> "yesterday"
            else -> "$d days ago"
        }
    }
    return phone.lastSeen?.let { "Used ${ago(days(it))}" } ?: "Paired ${ago(days(phone.pairedAt))}"
}

private fun LazyListScope.thisTv(memory: FocusMemory, first: Modifier) {
    item(key = "this-tv", contentType = "list") {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            val watchNext = LocalWatchNext.current
            TvListRow(
                "Show anistream on the home screen",
                onClick = { watchNext?.let { it.enable(!it.enabled) } },
                supporting = "Watch Next",
                switch = watchNext?.enabled ?: false,
                enabled = watchNext != null,
                modifier = first.remembered(memory, "tv/watch-next"),
            )
            ImageCacheRow(memory)
        }
    }
}

/** "Clear the image cache", with what it holds now; clearing it shows the new size once done. */
@Composable
private fun ImageCacheRow(memory: FocusMemory) {
    val store = LocalAppStore.current
    val scope = rememberCoroutineScope()
    var bytes by remember { mutableStateOf<Long?>(null) }
    var clearing by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { bytes = store.imageCacheBytes() }
    TvListRow(
        "Clear the image cache",
        onClick = {
            if (!clearing) {
                clearing = true
                scope.launch {
                    store.clearImageCache()
                    bytes = store.imageCacheBytes()
                    clearing = false
                }
            }
        },
        value = if (clearing) "Clearing" else bytes?.let(::megabytes),
        modifier = Modifier.remembered(memory, "tv/cache"),
    )
}

/** "84 MB", to the nearest megabyte. */
fun megabytes(bytes: Long): String = "${(bytes + (1L shl 19)) shr 20} MB"

private fun LazyListScope.about(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "about", contentType = "list") {
        val context = LocalContext.current
        val version = remember {
            runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "unknown"
        }
        val actions = LocalActions.current
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Nagare(cell = 4)
                Spacer(Modifier.width(16.dp))
                Column {
                    Text("anistream", style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
                    Text("Version $version", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                }
            }
            val updating = UpdateRows(nav, memory, first)
            TvListRow(
                "Licences",
                onClick = { actions?.open(ActionSheet("Licences", body = { InfoLines(Licences) })) },
                modifier = (if (updating) Modifier else first).remembered(memory, "about/licences"),
            )
            Quiet("anistream hosts no content.")
        }
    }
}

/**
 * About's update rows, where this build updates itself or a release is staged: the newer release,
 * which opens the update's page, or Check for updates with what the last check said; then the
 * daily check's switch. Whether there were any, so the first focus goes to the first of them.
 */
@Composable
private fun UpdateRows(nav: TvNavigator, memory: FocusMemory, first: Modifier): Boolean {
    val store = LocalAppStore.current
    val state by store.updates.state.collectAsStateWithLifecycle()
    val check by store.updates.check.collectAsStateWithLifecycle()
    val view by store.settings.view.collectAsStateWithLifecycle()
    val offer = state.offer
    if (offer == null && !store.updates.updatesItself) return false
    if (offer != null) {
        TvListRow(
            "Update to anistream ${offer.name}",
            onClick = nav::openUpdate,
            supporting = updateLine(state),
            icon = TvIcons.Update,
            modifier = first.remembered(memory, "about/update"),
        )
    } else {
        TvListRow(
            "Check for updates",
            onClick = { if (check != UpdateCheck.Checking) store.checkForUpdates() },
            value = when (val now = check) {
                UpdateCheck.Unknown, UpdateCheck.Found -> null
                UpdateCheck.Checking -> "Checking"
                UpdateCheck.UpToDate -> "Up to date"
                is UpdateCheck.Failed -> "Couldn't check"
            },
            supporting = (check as? UpdateCheck.Failed)?.let { plainReason(it.reason) },
            modifier = first.remembered(memory, "about/check"),
        )
    }
    if (store.updates.updatesItself) {
        TvListRow(
            "Check for updates automatically",
            onClick = { store.setSwitch(SettingKey.UPDATE_CHECK, !store.settings.isOn(SettingKey.UPDATE_CHECK, view)) },
            supporting = "Once a day, from anistream's releases",
            switch = store.settings.isOn(SettingKey.UPDATE_CHECK, view),
            enabled = store.settings.row(SettingKey.UPDATE_CHECK, view) != null,
            modifier = Modifier.remembered(memory, "about/auto-check"),
        )
    }
    return true
}

/** The update row's line: "Available · 48 MB", "Downloading 34 %", or why it stopped. */
fun updateLine(state: UpdateState): String = when (state) {
    UpdateState.None -> ""
    is UpdateState.Available -> listOfNotNull("Available", state.offer.bytes.takeIf { it > 0 }?.let(Wording::bytes)).joinToString(" · ")
    is UpdateState.Allowing -> "Waiting for Android to allow installs"
    is UpdateState.Downloading -> state.fraction?.let { "Downloading ${(it * 100).toInt()} %" } ?: "Downloading"
    is UpdateState.Verifying -> "Checking the download"
    is UpdateState.Installing -> "Waiting for you to confirm"
    is UpdateState.Done -> "Installed"
    is UpdateState.Failed -> "Couldn't update"
}

/** What anistream is under, and what it uses, as the phone's About lists them. */
private val Licences = listOf(
    "Licence" to "MIT, open source and non-commercial",
    "Typefaces" to "Roboto Flex and Zen Kaku Gothic New, under the SIL Open Font License 1.1",
    "Libraries" to "JNA and AndroidX, under the Apache License 2.0",
)

/** A row that only says something: its label and its value, not focusable. */
@Composable
private fun InfoRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp).semantics(mergeDescendants = true) {}, verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
        Text(value, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
}

/**
 * A panel's lines, each a label over its value, focusable so Up and Down move through them and
 * TalkBack reads each; then one action when there is one.
 */
@Composable
private fun InfoLines(lines: List<Pair<String, String>>, action: String? = null, onAction: () -> Unit = {}) {
    val host = LocalActions.current
    val first = remember { FocusRequester() }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        itemsIndexed(lines) { index, (label, value) ->
            TvListRow(label, onClick = {}, supporting = value, modifier = if (index == 0) Modifier.focusRequester(first) else Modifier)
        }
        if (action != null) {
            item {
                TvListRow(action, onClick = { host?.close(); onAction() })
            }
        }
    }
    LaunchedEffect(Unit) { first.focusSoon() }
}

@Composable
private fun Quiet(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
}
