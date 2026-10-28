// Settings: on a phone a full-screen page, the account and the panes as a grouped list, each pane
// a page pushed over it (AndroidSettings, AndroidSettingsPlayback, AndroidSettingsSources); on a
// tablet the list, the pane beside it and a column showing the effect of the selected row
// (AndroidTabletSettings). Every row is the engine's: its value, and the choices it steps through.

package tv.anistream.ui

import tv.anistream.store.plainReason
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import tv.anistream.design.Motion
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.SettingsView
import tv.anistream.store.SettingsCatalog
import tv.anistream.engine.model.SyncState
import tv.anistream.store.AccountsModel
import tv.anistream.store.Wording
import androidx.compose.ui.platform.LocalContext

/** The panes of Settings, in the list's order. */
enum class SettingsPane(val route: String, val titleRes: Int) {
    General("general", R.string.settings_group_general),
    Playback("playback", R.string.settings_group_playback),
    Notifications("notifications", R.string.settings_group_notifications),
    Downloads("downloads", R.string.downloads_title),
    Sources("sources", R.string.sources_title),
    Accounts("accounts", R.string.accounts_title),
    About("about", R.string.about_title);

    companion object {
        /** The pane [name] names, by its route, case aside. */
        fun named(name: String?): SettingsPane? =
            entries.firstOrNull { it.route.equals(name, ignoreCase = true) }
    }
}

/** Settings' route: its list on a phone, and the list with Playback beside it on a tablet. */
internal const val SETTINGS_ROUTE = "settings"

/** The route of [pane], or of the list. */
internal fun settingsRoute(pane: SettingsPane?): String = if (pane == null) SETTINGS_ROUTE else "$SETTINGS_ROUTE/${pane.route}"

/** Settings' routes, beside the five destinations' graphs. */
internal fun NavGraphBuilder.settingsPages(actions: AppActions) {
    composable(SETTINGS_ROUTE) {
        CompositionLocalProvider(LocalPageAnimation provides this) { SettingsScreen(null, actions) }
    }
    composable("$SETTINGS_ROUTE/{pane}") { entry ->
        val pane = SettingsPane.named(entry.arguments?.getString("pane"))
        CompositionLocalProvider(LocalPageAnimation provides this) { SettingsScreen(pane, actions) }
    }
}

/** Settings at [pane], or at its list: one page at a time on a phone, list and pane on a tablet. */
@Composable
fun SettingsScreen(pane: SettingsPane?, actions: AppActions) {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) {
        // What the list's lines say: where the downloads stand, and the title language's choice.
        store.loadDownloads()
        store.loadTitleLanguage()
    }
    if (wideWindow()) {
        TabletSettings(pane ?: SettingsPane.Playback, actions)
    } else if (pane == null) {
        PhoneSettingsList(actions)
    } else {
        PhonePane(pane, actions)
    }
}

// The phone

@Composable
private fun PhoneSettingsList(actions: AppActions) {
    val navigator = LocalNavigator.current
    ScreenFrame(title = "Settings", onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SettingsList(selected = null) { navigator.openSettings(it) }
        }
    }
}

@Composable
private fun PhonePane(pane: SettingsPane, actions: AppActions) {
    val navigator = LocalNavigator.current
    ScreenFrame(title = stringResource(pane.titleRes), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding(), bottom = 48.dp)
                .padding(horizontal = 16.dp),
        ) {
            PaneContent(pane, actions)
        }
    }
}

// The list

/**
 * The account, then General, Playback, Notifications, Downloads, Sources and Accounts as one group, then About: each with
 * what it says of itself. On a tablet [selected] is the pane beside it, filled primaryContainer.
 */
@Composable
internal fun SettingsList(selected: SettingsPane?, open: (SettingsPane) -> Unit) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val syncedAt by store.accounts.syncedAt.collectAsStateWithLifecycle()
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val downloads by store.downloads.rows.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val update by store.updates.state.collectAsStateWithLifecycle()
    AccountCard(trackers["anilist"], AccountWords.status(trackers["anilist"], syncedAt, working)) { open(SettingsPane.Accounts) }
    val panes = listOf(
        SettingsPane.General to generalSummary(LocalContext.current, view),
        SettingsPane.Playback to playbackSummary(LocalContext.current, view),
        SettingsPane.Notifications to notificationsSummary(LocalContext.current, view),
        SettingsPane.Downloads to storageLine(LocalContext.current, store.downloads.bytes(downloads)),
        SettingsPane.Sources to sourcesSummary(LocalContext.current, view, providers.size.takeIf { providersLoaded }),
        SettingsPane.Accounts to AccountWords.connected(LocalContext.current, trackers),
    )
    SettingsGroup(
        rows = panes.map { (pane, line) ->
            @Composable { place: Place -> LinkRow(place, stringResource(pane.titleRes), line, pane.icon, selected = pane == selected) { open(pane) } }
        },
    )
    SettingsGroup(
        rows = listOf({ place ->
            val version = listOfNotNull(stringResource(R.string.about_version, appVersion()), update.offer?.let { stringResource(R.string.updates_version_out, it.version) }).joinToString(" · ") // i18n-skip
            LinkRow(place, stringResource(SettingsPane.About.titleRes), version, SettingsPane.About.icon, selected = selected == SettingsPane.About) {
                open(SettingsPane.About)
            }
        }),
    )
}

private val SettingsPane.icon
    get() = when (this) {
        SettingsPane.General -> Icons.Settings
        SettingsPane.Playback -> Icons.PlaybackPage
        SettingsPane.Notifications -> Icons.NotificationsPage
        SettingsPane.Downloads -> Icons.DownloadsPage
        SettingsPane.Sources -> Icons.SourcesPage
        SettingsPane.Accounts -> Icons.AccountsPage
        SettingsPane.About -> Icons.About
    }

/** "New episodes on", or off. */
private fun notificationsSummary(context: Context, view: SettingsView?): String? {
    val row = view?.let { (it.rows + it.nativeRows).firstOrNull { row -> row.key == SettingKey.EPISODE_NOTIFICATIONS } } ?: return null
    return if (SettingsCatalog.isOn(row)) Strings.settingsSummaryNotifyOn(context) else Strings.settingsSummaryNotifyOff(context)
}

/** The title language's choice: "AniList's choice", "English" or "Romaji". */
private fun generalSummary(context: Context, view: SettingsView?): String? {
    val row = view?.rows?.firstOrNull { it.key == SettingKey.TITLES } ?: return null
    return TitleChoice.of(SettingsCatalog.current(row)?.value).label(context)
}

/** "Subtitled · 1080p". */
private fun playbackSummary(context: Context, view: SettingsView?): String? {
    val rows = view?.rows ?: return null
    val translation = rows.firstOrNull { it.key == SettingKey.TRANSLATION }?.value?.let { translationWords(context, it) }
    val quality = rows.firstOrNull { it.key == SettingKey.QUALITY }?.let { row -> SettingsCatalog.words(context, row.key, row.value) }
    return listOfNotNull(translation, quality).joinToString(" · ").ifEmpty { null } // i18n-skip
}

/** "5 sources · torrents on": the providers on screen, and whether torrents are. */
private fun sourcesSummary(context: Context, view: SettingsView?, providers: Int?): String? {
    val sources = view?.sources ?: return null
    // Before the engine has said its providers, the summary says nothing of them.
    providers ?: return null
    val count = if (providers > 0) context.resources.getQuantityString(R.plurals.settings_summary_sources, providers, providers) else Strings.settingsSummaryNoSources(context)
    return count + " · " + (if (sources.torrent) Strings.settingsSummaryTorrentsOn(context) else Strings.settingsSummaryTorrentsOff(context)) // i18n-skip
}

/**
 * The account at the top: who is signed in to AniList and how it last synced, with its dot, or
 * that nobody is. It opens Accounts.
 */
@Composable
private fun AccountCard(anilist: SyncState?, status: AccountStatus, onOpen: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val signedIn = anilist?.connected == true
    val user = anilist?.user
    val line = if (signedIn) Strings.accountsCardLine(LocalContext.current, AccountsModel.name("anilist"), status.text.replaceFirstChar { it.lowercase() }) else Strings.accountsLibraryNeeds(LocalContext.current, AccountsModel.name("anilist"))
    val described = if (signedIn) stringResource(R.string.accounts_card_description, user ?: stringResource(R.string.accounts_signed_in_plain), line) else stringResource(R.string.accounts_card_description_signed_out)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 80.dp)
            .clip(MaterialTheme.shapes.large)
            .background(colors.surfaceContainer)
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 12.dp)
            .semantics(mergeDescendants = true) {
                contentDescription = described
            },
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Avatar(if (signedIn) user else null, 48)
        Column(Modifier.weight(1f)) {
            Text(if (signedIn) user ?: stringResource(R.string.accounts_signed_in_plain) else stringResource(R.string.accounts_state_signed_out), style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (signedIn) StateDot(status.dot)
                Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
        }
    }
}

/** The signed-in user's initial on a raised circle, or the account glyph signed out. */
@Composable
internal fun Avatar(name: String?, size: Int) {
    Box(
        Modifier.size(size.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        if (name.isNullOrEmpty()) {
            Icon(Icons.Account, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            Text(
                name.take(1).uppercase(),
                style = if (size >= 48) MaterialTheme.typography.titleLarge.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Medium)
                else MaterialTheme.typography.titleMedium,
            )
        }
    }
}

/** The app's version, as About says it. */
@Composable
internal fun appVersion(): String {
    val context = androidx.compose.ui.platform.LocalContext.current
    return remember(context) {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "1.0"
    }
}

// The tablet

/**
 * The tablet's Settings, over the whole window: the list on the left, 296 wide; the pane in the
 * middle; and, where the window is wide enough, the column showing the effect of the row last
 * tapped, 320 wide. Picking another pane crossfades the middle on fade-in; the list stays.
 */
@Composable
private fun TabletSettings(start: SettingsPane, actions: AppActions) {
    val navigator = LocalNavigator.current
    var pane by rememberSaveable(start) { mutableStateOf(start) }
    val colors = MaterialTheme.colorScheme
    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(colors.surface)
            .windowInsetsPadding(WindowInsets.safeDrawing),
    ) {
        val aside = maxWidth >= 1100.dp
        Row(Modifier.fillMaxSize()) {
            Column(Modifier.width(344.dp).fillMaxHeight()) {
                Row(Modifier.height(64.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = stringResource(R.string.action_back)) }
                    Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(start = 4.dp).semantics { heading() })
                }
                Column(
                    Modifier
                        .verticalScroll(rememberScrollState())
                        .padding(start = 24.dp, end = 24.dp, top = 8.dp, bottom = 24.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    SettingsList(selected = pane) { pane = it }
                }
            }
            AnimatedContent(
                targetState = pane,
                transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeIn()) },
                contentKey = { it },
                modifier = Modifier.weight(1f).fillMaxHeight(),
                label = "pane",
            ) { shown ->
                val focus = remember(shown) { RowFocus(shown.defaultFocus) }
                CompositionLocalProvider(LocalRowFocus provides focus) {
                    Row(Modifier.fillMaxSize()) {
                        Column(Modifier.weight(1f).fillMaxHeight()) {
                            Box(Modifier.height(64.dp).padding(horizontal = 16.dp), contentAlignment = Alignment.CenterStart) {
                                Text(shown.title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
                            }
                            Column(
                                Modifier
                                    .fillMaxSize()
                                    .verticalScroll(rememberScrollState())
                                    .padding(top = 8.dp, bottom = 52.dp),
                            ) {
                                PaneContent(shown, actions)
                            }
                        }
                        if (aside) {
                            Spacer(Modifier.width(24.dp))
                            Column(Modifier.padding(top = 72.dp, end = 24.dp).width(320.dp)) {
                                SettingsAside(shown, focus.key)
                            }
                        } else {
                            Spacer(Modifier.width(24.dp))
                        }
                    }
                }
            }
        }
    }
}

/** The row a pane's column is about before any is tapped: Playback's upscaling, as the board has it. */
private val SettingsPane.defaultFocus: String?
    get() = when (this) {
        SettingsPane.General -> FOCUS_TITLES
        SettingsPane.Playback -> FOCUS_UPSCALING
        else -> null
    }

/** The panes' contents, the same on a phone's page and in a tablet's middle column. */
@Composable
internal fun PaneContent(pane: SettingsPane, actions: AppActions) {
    when (pane) {
        SettingsPane.General -> GeneralPane()
        SettingsPane.Playback -> PlaybackPane()
        SettingsPane.Notifications -> NotificationsPane(actions)
        SettingsPane.Downloads -> DownloadsPane(actions)
        SettingsPane.Sources -> SourcesPane(actions)
        SettingsPane.Accounts -> AccountsPane(actions)
        SettingsPane.About -> AboutPane()
    }
}

/** How a tracker's sync stands, in words and the dot beside them. */
data class AccountStatus(val text: String, val dot: androidx.compose.ui.graphics.Color?)

/** What the account rows say of a tracker. */
internal object AccountWords {
    /** "Synced 2 min ago", "2 changes waiting to sync", "Sign in again to keep syncing". */
    @Composable
    fun status(state: SyncState?, syncedAt: Map<String, Long>, working: Map<tv.anistream.engine.model.JobId, String>): AccountStatus {
        val colors = MaterialTheme.colorScheme
        val context = LocalContext.current
        state ?: return AccountStatus(Strings.accountsStateSignedOut(context), null)
        if (!state.connected) return AccountStatus(Strings.accountsStateSignedOut(context), null)
        if (state.needsReauth) return AccountStatus(Strings.accountsStateExpiredShort(context), colors.error)
        state.pullError?.let { return AccountStatus(Strings.accountsSyncFailed(context, plainReason(context, it.reason)), colors.error) }
        if ("sync" in working.values) return AccountStatus(Strings.accountsSyncRunning(context), colors.onSurfaceVariant)
        if (state.outbox > 0u) return AccountStatus(context.resources.getQuantityString(R.plurals.accounts_changes_waiting, state.outbox.toInt(), state.outbox.toInt()), colors.onSurfaceVariant)
        val at = syncedAt[state.tracker]
        if (at != null) {
            val seconds = (System.currentTimeMillis() - at) / 1000
            return AccountStatus(if (seconds < 60) Strings.accountsSyncDone(context) + " " + Strings.timeJustNow(context) else Strings.accountsSyncedAgo(context, Wording.ago(context, seconds)), colors.secondary) // i18n-skip
        }
        return AccountStatus(Strings.accountsSyncDone(context), colors.secondary)
    }

    /** The signed-in trackers by name, "AniList, Kitsu", or "None". */
    fun connected(context: Context, trackers: Map<String, SyncState>): String =
        Wording.list(AccountsModel.order.filter { trackers[it]?.connected == true }.map { AccountsModel.name(it) }).ifEmpty { Strings.valueNone(context) }
}
