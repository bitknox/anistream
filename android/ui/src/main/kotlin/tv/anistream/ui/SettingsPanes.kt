// Settings' panes: Playback (AndroidSettingsPlayback, AndroidSettingsTitles), Downloads, Sources
// (AndroidSettingsSources), Accounts and About, each a column of groups, the same on a phone's page
// and in a tablet's middle column.

package tv.anistream.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.util.Locale
import tv.anistream.design.Nagare
import tv.anistream.design.tabular
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.StorePluginStatus
import tv.anistream.engine.model.TitleLanguage
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.store.AccountsModel
import tv.anistream.store.LibraryModel
import tv.anistream.store.ProvidersModel
import tv.anistream.store.SettingsCatalog
import tv.anistream.store.Wording

/** The focus keys of the rows a tablet's column shows the effect of. */
internal const val FOCUS_UPSCALING = "upscaling"
internal const val FOCUS_TITLES = "titles"

/** A value as a row says it, in sentence case: "Natural · ArtCNN", "Off", "1080p". */
internal fun sentence(value: String): String = value.replaceFirstChar { it.titlecase(Locale.UK) }

/** "Subtitled" or "Dubbed", for sub or dub as the engine says it. */
internal fun translationWords(value: String): String = when (value.substringBefore(" · ")) {
    "sub" -> "Subtitled"
    "dub" -> "Dubbed"
    else -> sentence(value)
}

/**
 * A stepped setting as a menu of its choices, each named as its row will say it, with how many
 * shows set it for themselves under it.
 */
@Composable
internal fun SettingMenu(
    place: Place,
    key: SettingKey,
    title: String,
    note: String? = null,
    enabled: Boolean = true,
    focus: String? = null,
    label: (String) -> String = ::sentence,
) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    val current = SettingsCatalog.current(row)
    MenuRow(
        place = place,
        title = title,
        value = row?.value?.let { current?.label ?: it }?.let(label),
        choices = SettingsCatalog.options(key).map { MenuChoice(it.value, label(it.label)) },
        current = current?.value,
        note = listOfNotNull(store.settings.overridden(key, view), note).joinToString("\n").ifEmpty { null },
        enabled = enabled && SettingsCatalog.editable(row),
        focus = focus,
    ) { store.setSetting(key, it) }
}

/** A setting that is on or off, with how many shows set it for themselves under it. */
@Composable
internal fun SettingSwitch(place: Place, key: SettingKey, title: String, note: String? = null, enabled: Boolean = true) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    SwitchRow(
        place = place,
        title = title,
        checked = SettingsCatalog.isOn(row),
        note = listOfNotNull(store.settings.overridden(key, view), note).joinToString("\n").ifEmpty { null },
        enabled = enabled && SettingsCatalog.editable(row),
    ) { store.setSetting(key, SettingValue.Bool(it)) }
}

/** Groups 8 apart, so a header's own 8 above it makes 16. */
@Composable
private fun Pane(content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { content() }
}

// Playback

@Composable
internal fun PlaybackPane() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val autoNext = store.settings.isOn(SettingKey.AUTO_NEXT, view)
    Pane {
        SettingsGroup(
            title = "Language",
            footer = "Each show remembers its own audio and subtitles.",
            rows = listOf(
                { place -> TitleLanguageRow(place) },
                { place -> SettingMenu(place, SettingKey.TRANSLATION, "Translation", label = ::translationWords) },
                { place -> SubtitlesRow(place) },
                { place -> SettingMenu(place, SettingKey.QUALITY, "Quality") },
            ),
        )
        SettingsGroup(
            title = "Episodes",
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.AUTO_NEXT, "Auto-play next") },
                { place -> SettingMenu(place, SettingKey.UP_NEXT_COUNTDOWN, "Up Next countdown", enabled = autoNext) },
                { place -> SettingSwitch(place, SettingKey.SKIP_OPENING, "Skip opening automatically") },
                { place -> SettingSwitch(place, SettingKey.SKIP_FILLER, "Skip filler automatically") },
                { place -> SettingSwitch(place, SettingKey.MARK_CHAPTERS, "Mark opening and ending") },
                { place -> SettingMenu(place, SettingKey.COMMIT_THRESHOLD, "Counts as watched at", note = "Or when the ending starts, if it is marked") },
                { place -> SettingSwitch(place, SettingKey.SPOILER_PROTECTION, "Spoiler protection", note = "Blurs stills of unwatched episodes after the next one") },
            ),
        )
        SettingsGroup(
            title = "Picture and sound",
            rows = listOf(
                { place -> SettingMenu(place, SettingKey.UPSCALING, "Upscaling", focus = FOCUS_UPSCALING) },
                { place -> SettingSwitch(place, SettingKey.NIGHT_MODE, "Night mode", note = "Evens out loud and quiet passages, so dialogue stays clear at low volume") },
            ),
        )
    }
}

/** The three forms a title can be named in, as the settings file names them. */
internal enum class TitleChoice(val value: String, val label: String) {
    AniList("anilist", "AniList's choice"),
    English("english", "English"),
    Romaji("romaji", "Romaji");

    companion object {
        fun of(value: SettingValue?): TitleChoice = entries.firstOrNull { (value as? SettingValue.Str)?.v1 == it.value } ?: AniList
    }
}

/** What the title-language row and its preview say: metadata.md, "The title-language row". */
internal object TitleLanguageWords {
    /** The line under the label: what the choice does now. */
    fun line(choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String = when (choice) {
        TitleChoice.AniList -> when {
            !signedIn -> "English while signed out of AniList"
            viewer != null -> "${name(viewer)}, from your AniList account"
            else -> "Follows your AniList account"
        }
        TitleChoice.English -> "Romaji where there is no English title"
        TitleChoice.Romaji -> "Japanese titles in Latin letters"
    }

    /** The note under the tablet's examples. */
    fun note(choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?, examples: List<Entry>): String = when (choice) {
        TitleChoice.AniList -> {
            val signedOut = "While you are signed out of AniList, titles show in English."
            if (signedIn && viewer != null) "Follows your AniList account, which is set to ${name(viewer).lowercase(Locale.UK)}. $signedOut"
            else "Follows your AniList account. $signedOut"
        }
        TitleChoice.English -> examples.firstOrNull { it.titleEnglish.isNullOrEmpty() }
            ?.let { "${it.titleRomaji ?: it.title} has no English title on AniList, so it keeps its romaji one." }
            ?: "English titles as AniList lists them, and romaji where there is none."
        TitleChoice.Romaji -> "Japanese titles written in Latin letters, as AniList lists them."
    }

    fun name(language: TitleLanguage): String = when (language) {
        TitleLanguage.ROMAJI -> "Romaji"
        TitleLanguage.ENGLISH -> "English"
        TitleLanguage.NATIVE -> "Native"
    }

    /**
     * [entry]'s title as [choice] would name it, the way the engine picks: the form asked for,
     * else English, else romaji, else native. AniList's choice follows the signed-in account, and
     * English while nobody is.
     */
    fun example(entry: Entry, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String {
        val language = when (choice) {
            TitleChoice.AniList -> if (signedIn) viewer ?: TitleLanguage.ENGLISH else TitleLanguage.ENGLISH
            TitleChoice.English -> TitleLanguage.ENGLISH
            TitleChoice.Romaji -> TitleLanguage.ROMAJI
        }
        val chosen = when (language) {
            TitleLanguage.ROMAJI -> entry.titleRomaji
            TitleLanguage.ENGLISH -> entry.titleEnglish
            TitleLanguage.NATIVE -> entry.titleNative
        }
        val forms = listOf(chosen, entry.titleEnglish, entry.titleRomaji, entry.titleNative)
        return forms.firstOrNull { !it.isNullOrEmpty() } ?: entry.title
    }
}

/** Three titles to show the choice on: Continue's, then the Library's, then this season's, with covers. */
@Composable
internal fun titleLanguageExamples(): List<Entry> {
    val store = LocalAppStore.current
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val continuing by store.continuing.list.state.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val library = LibraryModel.order.flatMap { store.library.segment(it).state.value.ids }
    return (continuing.ids + library + seasonal.ids)
        .distinct()
        .mapNotNull { titles[it] }
        .filter { it.coverUrl != null }
        .take(3)
}

/** Whether AniList is signed in, for the title-language row's words. */
@Composable
internal fun anilistSignedIn(): Boolean {
    val trackers by LocalAppStore.current.accounts.trackers.collectAsStateWithLifecycle()
    return trackers["anilist"]?.connected == true
}

/**
 * Title language, the first row: AniList's choice, English or Romaji, each in the menu with an
 * example title under it, and a line under the label saying what the choice does now.
 */
@Composable
private fun TitleLanguageRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val viewer by store.accounts.viewerTitleLanguage.collectAsStateWithLifecycle()
    val signedIn = anilistSignedIn()
    val row = store.settings.row(SettingKey.TITLES, view)
    val choice = TitleChoice.of(SettingsCatalog.current(row)?.value)
    val example = titleLanguageExamples().firstOrNull()
    LaunchedEffect(Unit) { store.loadContinue() }
    MenuRow(
        place = place,
        title = "Title language",
        value = choice.label,
        choices = TitleChoice.entries.map { MenuChoice(it, it.label, example?.let { e -> TitleLanguageWords.example(e, it, signedIn, viewer) }) },
        current = choice,
        note = TitleLanguageWords.line(choice, signedIn, viewer),
        enabled = row != null,
        focus = FOCUS_TITLES,
        menuWidth = 300,
    ) { if (it != choice) store.setSetting(SettingKey.TITLES, SettingValue.Str(it.value)) }
}

/** The subtitle language, by name, from the languages people pick most; any other is typed in the file. */
@Composable
private fun SubtitlesRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.SUBTITLES, view)
    val code = SettingsCatalog.typedText(row) ?: row?.value ?: ""
    val languages = listOf("en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh")
    val choices = (languages + listOf(code).filter { it.isNotEmpty() && it !in languages }).map { MenuChoice(it, languageName(it)) }
    MenuRow(place, "Subtitles", languageName(code), choices, code, enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.SUBTITLES, SettingValue.Str(it))
    }
}

/** A language code as its name in English: "English" for "en" or "eng". */
internal fun languageName(code: String): String {
    if (code.isEmpty()) return "–"
    val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.UK)
    return if (name.isNullOrEmpty() || name.equals(code, ignoreCase = true)) code else name
}

// Downloads

@Composable
internal fun DownloadsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val downloads by store.downloads.rows.collectAsStateWithLifecycle()
    val torrents = store.settings.isOn(SettingKey.TORRENTS, view)
    Pane {
        SettingsGroup(
            rows = listOf({ place -> InfoRow(place, "Storage", storageLine(store.downloads.bytes(downloads))) }),
        )
        SettingsGroup(
            title = "New downloads",
            rows = listOfNotNull(
                @Composable { place: Place -> SettingMenu(place, SettingKey.DOWNLOAD_QUALITY, "Quality") },
                { place -> SettingMenu(place, SettingKey.KEEP_NEXT_EPISODES, "Keep next episodes", note = "Downloads the episodes after the one you are on, for each show you watch") },
                { place -> WifiOnlyRow(place) },
                { place -> SettingSwitch(place, SettingKey.MERGE_SUBTITLES, "Merge subtitles in") },
                if (torrents) @Composable { place: Place -> SettingSwitch(place, SettingKey.KEEP_SEEDING, "Keep seeding") } else null,
            ),
        )
    }
}

/** Wi-Fi only: downloads on mobile data, turned over, as a phone says it. */
@Composable
private fun WifiOnlyRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.DOWNLOADS_ON_CELLULAR, view)
    SwitchRow(place, "Wi-Fi only", checked = !SettingsCatalog.isOn(row), note = "Downloads wait for Wi-Fi", enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.DOWNLOADS_ON_CELLULAR, SettingValue.Bool(!it))
    }
}

// Sources

@Composable
internal fun SourcesPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.checkProviders() }
    val torrents = store.settings.isOn(SettingKey.TORRENTS, view)
    val updates = stores.sumOf { store -> store.plugins.count { it.status is StorePluginStatus.Update } }
    Pane {
        SettingsGroup(
            rows = listOf(
                { place -> InfoRow(place, "Source order", Wording.plural(providers.size, "source")) },
                { place -> InfoRow(place, "Plugin stores", if (updates > 0) "${stores.size} · ${Wording.plural(updates, "update")}" else "${stores.size}") },
            ),
        )
        SettingsGroup(
            title = "Torrents",
            footer = if (torrents) CHECK_NOTE else "Off by default. Turning this on asks how to protect your connection first.",
            rows = listOfNotNull(
                @Composable { place: Place -> TorrentSourceRow(place, torrents) },
                if (torrents) @Composable { place: Place -> InfoRow(place, "Torrent index", view?.sources?.index ?: "None set") } else null,
                if (torrents) @Composable { place: Place -> InfoRow(place, "Protect with", protectionWords(view?.torrent?.protection)) } else null,
                if (torrents) @Composable { place: Place -> LastCheckRow(place) } else null,
                if (torrents) @Composable { place: Place -> SettingMenu(place, SettingKey.VPN_ON_FAILURE, "If a check fails") } else null,
                if (torrents) @Composable { place: Place -> SettingSwitch(place, SettingKey.TORRENTS_ON_CELLULAR, "Torrents on cellular") } else null,
            ),
        )
        if (torrents) {
            SettingsGroup(
                title = "Seeding",
                footer = "Seeding runs while anistream is open.",
                rows = listOf(
                    { place -> SettingMenu(place, SettingKey.SEED_RATIO, "Seed after watching") },
                    { place -> SettingMenu(place, SettingKey.SEED_HOURS, "Seed for at most") },
                    { place -> SettingMenu(place, SettingKey.UPLOAD_LIMIT, "Upload limit") },
                ),
            )
        }
    }
}

/** What the connection check does, said wherever torrents are on. */
internal const val CHECK_NOTE =
    "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, " +
        "and pauses torrents if that check fails. Your VPN or proxy is what protects you."

/** The torrent source's switch. Turning it off is a switch; turning it on asks for protection first. */
@Composable
private fun TorrentSourceRow(place: Place, on: Boolean) {
    val store = LocalAppStore.current
    // Turning torrents on goes through the sheet that asks how to protect the connection, which
    // is not in this build yet: until then only turning them off is offered here.
    SwitchRow(place, "Torrent source", checked = on, enabled = on) { wanted ->
        if (!wanted) store.setSwitch(SettingKey.TORRENTS, false)
    }
}

/** "SOCKS5 proxy", "VPN on this device", "No protection". */
internal fun protectionWords(protection: TorrentProtection?): String = when (protection) {
    TorrentProtection.PROXY -> "SOCKS5 proxy"
    TorrentProtection.DEVICE, null -> "VPN on this device"
    TorrentProtection.NONE -> "No protection"
}

/** The connection check's last word, with its dot, where traffic leaves from, and Check now. */
@Composable
private fun LastCheckRow(place: Place) {
    val store = LocalAppStore.current
    val check by store.providers.check.collectAsStateWithLifecycle()
    val words = guardWords(check)
    val colors = MaterialTheme.colorScheme
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 88.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(Modifier.weight(1f)) {
                Text("Last check", style = MaterialTheme.typography.titleMedium)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    StateDot(
                        when {
                            words.problem -> colors.error
                            words.passed -> colors.secondary
                            else -> null
                        },
                    )
                    Text(words.status, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
                }
                words.note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
            }
            TextButton(onClick = store::checkProviders) {
                Text("Check now", style = MaterialTheme.typography.titleMedium, color = colors.onSurface)
            }
        }
    }
}

/** The check's verdict as the row says it: its status, the line under it, and its dot's colour. */
internal data class GuardWords(val status: String, val note: String?, val problem: Boolean, val passed: Boolean)

internal fun guardWords(check: ProvidersModel.Check?, now: Long = System.currentTimeMillis()): GuardWords {
    val ago = check?.let {
        val seconds = ((now - it.at) / 1000).coerceAtLeast(0)
        if (seconds < 60) "$seconds s ago" else AccountWords.agoWords(seconds)
    }
    return when (val view = check?.view) {
        is GuardView.Protected -> GuardWords("Passed" + (ago?.let { " · $it" } ?: ""), "Leaving through ${view.exit}", problem = false, passed = true)
        GuardView.Checking -> GuardWords("Checking", "Checking where torrent traffic leaves from", problem = false, passed = false)
        GuardView.Leaking -> GuardWords("Failed", "Traffic left outside the protection. Torrents are paused.", problem = true, passed = false)
        GuardView.Unreachable -> GuardWords("Failed", "The check could not be made. Torrents are paused.", problem = true, passed = false)
        GuardView.Unprotected -> GuardWords("Unprotected", "No protection is set", problem = false, passed = false)
        null -> GuardWords("Not checked yet", null, problem = false, passed = false)
    }
}

// Accounts

@Composable
internal fun AccountsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val syncedAt by store.accounts.syncedAt.collectAsStateWithLifecycle()
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val anilist = trackers["anilist"]
    val storage = trackers.values.firstNotNullOfOrNull { it.storage }
    Pane {
        SettingsGroup(
            title = "AniList",
            footer = "Progress and list changes sync to AniList. Library shows your AniList lists.",
            rows = if (anilist?.connected == true) {
                val status = AccountWords.status(anilist, syncedAt, working)
                listOf(
                    @Composable { place: Place ->
                        InfoRow(place, anilist.user ?: "Signed in", status.text, leading = { Avatar(anilist.user, 40) })
                    },
                    { place -> ActionRow(place, "Sync now", enabled = "sync" !in working.values, onClick = store::syncNow) },
                    { place -> ActionRow(place, "Sign out") { store.disconnect("anilist") } },
                )
            } else {
                listOf({ place ->
                    InfoRow(place, "AniList", "Brings in your lists and fills Continue with what you are watching.")
                })
            },
        )
        SettingsGroup(
            title = "Other trackers",
            footer = if (storage?.contains("file", ignoreCase = true) == true) "Sign-in tokens are kept in a file on this device."
            else "Sign-in tokens are kept on this device, sealed with a key in its keystore.",
            rows = AccountsModel.order.drop(1).map { tracker ->
                @Composable { place: Place ->
                    val state = trackers[tracker]
                    val connected = state?.connected == true
                    val signOut: @Composable () -> Unit = { TextButton(onClick = { store.disconnect(tracker) }) { Text("Sign out") } }
                    InfoRow(
                        place,
                        AccountsModel.name(tracker),
                        if (connected) AccountWords.status(state, syncedAt, working).text else "Not signed in",
                        trailing = signOut.takeIf { connected },
                    )
                }
            },
        )
    }
}

/** A row that is one action in silk, such as Sync now or Sign out. */
@Composable
internal fun ActionRow(place: Place, title: String, enabled: Boolean = true, onClick: () -> Unit) {
    RowFrame(place, onClick = if (enabled) onClick else null) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// About

@Composable
internal fun AboutPane() {
    Pane {
        Column(Modifier.fillMaxWidth().padding(vertical = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Nagare(cell = 6)
            Text("anistream", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(top = 16.dp))
            Text("Version ${appVersion()}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        SettingsGroup(
            footer = LEGAL_LINE,
            rows = listOf(
                { place -> InfoRow(place, "Licence", "MIT, open source and non-commercial") },
                { place -> InfoRow(place, "Typefaces", "Roboto Flex and Zen Kaku Gothic New, under the SIL Open Font License 1.1") },
                { place -> InfoRow(place, "Libraries", "JNA and AndroidX, under the Apache License 2.0") },
            ),
        )
    }
}
