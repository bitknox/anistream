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
import androidx.compose.ui.Modifier
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
import tv.anistream.design.ObiSilk
import tv.anistream.engine.ffi.FfiPairedPhone
import tv.anistream.engine.model.SettingChange
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.SettingsView
import tv.anistream.engine.model.SyncState
import tv.anistream.store.AccountsModel
import tv.anistream.store.AppStore
import tv.anistream.store.SettingsCatalog
import tv.anistream.tv.home.LocalWatchNext
import tv.anistream.tv.remote.ConfirmDialog
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.remote.RemovePhoneDialog
import tv.anistream.tv.shell.TvNavigator
import tv.anistream.tv.ui.Canvas
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
    LaunchedEffect(Unit) { store.loadStores() }
    ToTop(nav, memory, scrolled = { false }) {}

    Row(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY, end = Canvas.SafeX)) {
        Column(Modifier.width(256.dp).fillMaxHeight().focusRestorer(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("Settings", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 16.dp))
            SettingsSection.entries.forEach { each ->
                TvListRow(
                    each.label,
                    onClick = {},
                    selected = each == section,
                    modifier = Modifier
                        .onFocusChanged { if (it.isFocused) section = each }
                        .firstFocus(memory, "section/${each.name}", each == SettingsSection.Playback),
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
                    SettingsSection.PluginStores -> pluginStores(memory, first)
                    SettingsSection.Accounts -> accounts(nav, memory, first)
                    SettingsSection.Remotes -> remotes(memory, first)
                    SettingsSection.ThisTv -> thisTv(memory, first)
                    SettingsSection.About -> about(memory, first)
                }
            }
        }
    }
    EnterFocus(memory)
}

private fun LazyListScope.playback(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    PlaybackRows.forEachIndexed { index, (key, label) ->
        item(key = key.name, contentType = "setting") {
            val row = store.settings.row(key, view)
            val switch = SettingsCatalog.isSwitch(key)
            val overridden = store.settings.overridden(key, view)
            TvListRow(
                label,
                onClick = { change(store, key, row?.change, switch, view) },
                supporting = overridden,
                value = if (switch) null else row?.let { SettingsCatalog.current(it)?.label ?: it.value },
                switch = if (switch) store.settings.isOn(key, view) else null,
                enabled = SettingsCatalog.editable(row),
                modifier = (if (index == 0) first else Modifier).remembered(memory, "row/${key.name}"),
            )
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
            rows.forEachIndexed { index, provider ->
                TvListRow(
                    provider.name,
                    onClick = {},
                    supporting = provider.version?.let { "Version $it" },
                    value = if (provider.usable) null else "off",
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "provider/${provider.id}"),
                )
            }
        }
    }
}

private fun LazyListScope.pluginStores(memory: FocusMemory, first: Modifier) {
    item(key = "stores", contentType = "list") {
        val store = LocalAppStore.current
        val stores by store.stores.stores.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            stores.forEachIndexed { index, row ->
                TvListRow(row.name, onClick = {}, supporting = row.host, modifier = (if (index == 0) first else Modifier).remembered(memory, "store/${row.id}"))
            }
            TvListRow(
                "Add a plugin store",
                onClick = {},
                supporting = "From a paired phone",
                enabled = false,
                modifier = (if (stores.isEmpty()) first else Modifier).remembered(memory, "store/add"),
            )
        }
    }
}

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
            TvListRow("This TV's name for phones", onClick = {}, value = name, modifier = first.remembered(memory, "remote/name"))
            TvListRow(
                "Pair a phone",
                onClick = { remote?.openPairing() },
                enabled = remote != null,
                modifier = Modifier.remembered(memory, "remote/pair"),
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

private fun LazyListScope.about(memory: FocusMemory, first: Modifier) {
    item(key = "about", contentType = "list") {
        val context = LocalContext.current
        val version = remember {
            runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "unknown"
        }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            TvListRow("Version", onClick = {}, value = version, modifier = first.remembered(memory, "about/version"))
            TvListRow("Licences", onClick = {}, modifier = Modifier.remembered(memory, "about/licences"))
            Quiet("anistream hosts no content.")
        }
    }
}

@Composable
private fun Quiet(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
}
