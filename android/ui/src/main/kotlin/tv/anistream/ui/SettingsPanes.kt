// Settings' panes: Playback (AndroidSettingsPlayback, AndroidSettingsTitles), Downloads, Sources
// (AndroidSettingsSources), Accounts and About, each a column of groups, the same on a phone's page
// and in a tablet's middle column.

package tv.anistream.ui

import android.os.Build
import tv.anistream.i18n.Language
import tv.anistream.store.AppLanguage
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import java.util.Locale
import tv.anistream.design.Nagare
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.NoticeReason
import tv.anistream.engine.model.UpcomingNotice
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.StorePluginStatus
import tv.anistream.engine.model.TitleLanguage
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.engine.model.TorrentProxy
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import android.content.Context
import java.text.NumberFormat
import androidx.compose.ui.res.pluralStringResource
import tv.anistream.store.AccountsModel
import tv.anistream.store.AirTime
import tv.anistream.store.ComingModel
import tv.anistream.store.DevicePreferences
import tv.anistream.store.LibraryModel
import tv.anistream.store.ProvidersModel
import tv.anistream.store.TorrentWords
import tv.anistream.store.SettingsCatalog
import tv.anistream.store.Wording
import tv.anistream.store.updates

/** The focus keys of the rows a tablet's column shows the effect of. */
internal const val FOCUS_UPSCALING = "upscaling"
internal const val FOCUS_TITLES = "titles"

/** A value as a row says it, in sentence case, for a token the app itself does not word. */
internal fun sentence(value: String): String = value.replaceFirstChar { it.titlecase(Locale.getDefault()) }

/** "Subbed" or "Dubbed", for sub or dub as the engine says it. */
internal fun translationWords(context: Context, value: String): String = when (value.substringBefore(" · ")) {
    "sub" -> Strings.audioSubbed(context)
    "dub" -> Strings.audioDubbed(context)
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
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    val current = SettingsCatalog.current(row)
    MenuRow(
        place = place,
        title = title,
        value = SettingsCatalog.words(context, row),
        choices = SettingsCatalog.options(key).map { MenuChoice(it.value, SettingsCatalog.words(context, key, it)) },
        current = current?.value,
        note = listOfNotNull(store.settings.overridden(LocalContext.current, key, view), note).joinToString("\n").ifEmpty { null }, // i18n-skip
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
        note = listOfNotNull(store.settings.overridden(LocalContext.current, key, view), note).joinToString("\n").ifEmpty { null }, // i18n-skip
        enabled = enabled && SettingsCatalog.editable(row),
    ) { store.setSetting(key, SettingValue.Bool(it)) }
}

/** Groups 8 apart, so a header's own 8 above it makes 16. */
@Composable
private fun Pane(content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { content() }
}

// Notifications

/**
 * Notifications, as AndroidSettingsNotify draws it: the New episodes switch for this device on a
 * card of its own, with the shows that set their own, and the note that the permission is asked
 * then; then the system's settings for the channel. Turning the switch on goes through the app,
 * which asks for the permission to post then, and only then; where Android says no, the switch
 * stays off and its line says why. Under it, "Coming up · next 7 days": the episodes of the shows
 * the check follows, dimmed while the switch is off, each opening its show's settings, where its
 * own switch is.
 */
@Composable
internal fun NotificationsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val key = SettingKey.EPISODE_NOTIFICATIONS
    val row = store.settings.row(key, view)
    val app = actions.notifications
    val refused = app?.refused?.collectAsStateWithLifecycle()?.value == true
    val week by store.coming.week.collectAsStateWithLifecycle()
    var opened by remember { mutableStateOf<AnilistId?>(null) }
    LaunchedEffect(Unit) { store.loadComing() }
    Pane {
        SettingsGroup(
            footer = stringResource(R.string.settings_notify_permission_note),
            rows = listOf(
                { place ->
                    SwitchRow(
                        place = place,
                        title = stringResource(R.string.settings_new_episodes),
                        checked = SettingsCatalog.isOn(row),
                        note = listOfNotNull(
                            if (refused) stringResource(R.string.settings_notify_refused) else stringResource(R.string.settings_new_episodes_detail),
                            store.settings.overridden(LocalContext.current, key, view),
                        ).joinToString("\n"),
                        enabled = SettingsCatalog.editable(row),
                    ) { on -> app?.turn?.invoke(on) ?: store.setSwitch(key, on) }
                },
            ),
        )
        week?.let { week ->
            SettingsGroup(
                title = stringResource(R.string.settings_coming_up),
                footer = ComingModel.footer(LocalContext.current, week),
                rows = ComingModel.listed(week).map { episode ->
                    @Composable { place: Place -> ComingRow(place, episode) { opened = episode.id } }
                },
            )
        }
        if (app != null) {
            SettingsGroup(
                modifier = Modifier.padding(top = 8.dp),
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            stringResource(R.string.settings_notify_system_title),
                            stringResource(R.string.settings_notify_system_detail),
                            onClick = app.openSystemSettings,
                            trailing = { Icon(Icons.External, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
                        )
                    },
                ),
            )
        }
    }

    // A show's settings, over the pane; its own switch may change what is coming up.
    opened?.let { id ->
        val titles by store.titles.entries.collectAsStateWithLifecycle()
        val entry = titles[id]
        if (entry == null) {
            LaunchedEffect(id) { store.loadDetail(id) }
        } else {
            ShowSettingsSheet(entry) {
                opened = null
                store.loadComing()
            }
        }
    }
}

/**
 * One episode coming up: the show's cover, its title, "E4 · Today 22:00", and "On in this show's
 * settings" for a show followed by its own switch. Dimmed when it does not notify, as the engine
 * says: a show following the switch while it is off. A show switched on stays.
 */
@Composable
private fun ComingRow(place: Place, episode: UpcomingNotice, onClick: () -> Unit) {
    val dimmed = !episode.notifies
    val note = if (episode.reason == NoticeReason.SWITCHED_ON) stringResource(R.string.settings_notifications_show_on) else null
    val context = LocalContext.current
    InfoRow(
        place,
        episode.title,
        Strings.calendarRowTime(context, episode.episode.toInt(), AirTime.coming(context, Instant.ofEpochSecond(episode.airsAt))),
        note = note,
        onClick = onClick,
        leading = {
            ArtworkImage(
                episode.coverUrl,
                plate = null,
                maxPixelSize = pixels(60.dp),
                modifier = Modifier.size(width = 40.dp, height = 60.dp).clip(RoundedCornerShape(6.dp)),
            )
        },
        trailing = { Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
        modifier = Modifier.alpha(if (dimmed) 0.4f else 1f),
        // A show's title, so the show-title face, as everywhere a title is named.
        titleStyle = ShowTitle.row,
    )
}

// General

/** How titles read and which are listed: nothing here is about playing an episode. */
@Composable
internal fun GeneralPane() {
    Pane {
        SettingsGroup(
            title = stringResource(R.string.settings_language_app),
            rows = listOf({ place -> LanguageRow(place) }),
        )
        SettingsGroup(
            title = stringResource(R.string.settings_group_titles),
            rows = listOf(
                { place -> TitleLanguageRow(place) },
                { place -> SettingSwitch(place, SettingKey.ADULT_TITLES, stringResource(R.string.settings_adult), note = stringResource(R.string.settings_adult_detail, "AniList")) },
            ),
        )
    }
}

// Playback

@Composable
internal fun PlaybackPane() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val autoNext = store.settings.isOn(SettingKey.AUTO_NEXT, view)
    Pane {
        SettingsGroup(
            title = stringResource(R.string.settings_language),
            footer = stringResource(R.string.settings_audio_detail),
            rows = listOf(
                { place -> SettingMenu(place, SettingKey.TRANSLATION, stringResource(R.string.settings_audio)) },
                { place -> SubtitlesRow(place) },
                { place -> FollowCaptionsRow(place) },
                { place -> SettingMenu(place, SettingKey.QUALITY, stringResource(R.string.settings_quality)) },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.settings_group_episodes),
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.AUTO_NEXT, stringResource(R.string.settings_auto_play)) },
                { place -> SettingMenu(place, SettingKey.UP_NEXT_COUNTDOWN, stringResource(R.string.settings_up_next_countdown), enabled = autoNext) },
                { place -> SettingSwitch(place, SettingKey.SKIP_OPENING, stringResource(R.string.settings_skip_opening)) },
                { place -> SettingSwitch(place, SettingKey.SKIP_FILLER, stringResource(R.string.settings_skip_filler)) },
                { place -> SettingSwitch(place, SettingKey.MARK_CHAPTERS, stringResource(R.string.settings_mark_opening)) },
                { place -> SettingSwitch(place, SettingKey.SKIP_BUTTON, stringResource(R.string.settings_skip_button), note = stringResource(R.string.settings_skip_button_detail)) },
                { place -> SettingMenu(place, SettingKey.COMMIT_THRESHOLD, stringResource(R.string.settings_counts_watched), note = stringResource(R.string.settings_counts_watched_detail)) },
                { place -> SettingSwitch(place, SettingKey.SPOILER_PROTECTION, stringResource(R.string.settings_spoiler), note = stringResource(R.string.settings_spoiler_detail)) },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.player_picture_and_sound),
            rows = listOf(
                { place -> SettingMenu(place, SettingKey.UPSCALING, stringResource(R.string.player_upscaling), focus = FOCUS_UPSCALING) },
                { place -> SettingSwitch(place, SettingKey.NIGHT_MODE, stringResource(R.string.player_night_mode), note = stringResource(R.string.player_night_mode_detail)) },
            ),
        )
    }
}

/** Kept on this device (DevicePreferences): the caption style it follows is this device's. */
@Composable
private fun FollowCaptionsRow(place: Place) {
    val context = LocalContext.current
    var follow by remember { mutableStateOf(DevicePreferences.followsSystemCaptions(context)) }
    SwitchRow(
        place,
        stringResource(R.string.settings_system_captions),
        follow,
        note = stringResource(R.string.settings_system_captions_detail),
    ) {
        follow = it
        DevicePreferences.of(context).edit().putBoolean(DevicePreferences.FOLLOW_SYSTEM_CAPTIONS, it).apply()
    }
}

/** The three forms a title can be named in, as the settings file names them. */
internal enum class TitleChoice(val value: String) {
    AniList("anilist"),
    English("english"),
    Romaji("romaji");

    /** The choice as the menu names it. */
    fun label(context: Context): String = when (this) {
        AniList -> Strings.settingsTitleTracker(context, "AniList") // i18n-skip
        English -> Strings.settingsTitleEnglish(context)
        Romaji -> Strings.settingsTitleRomaji(context)
    }

    companion object {
        fun of(value: SettingValue?): TitleChoice = entries.firstOrNull { (value as? SettingValue.Str)?.v1 == it.value } ?: AniList
    }
}

/** What the title-language row and its preview say: metadata.md, "The title-language row". */
internal object TitleLanguageWords {
    /** The line under the label: what the choice does now. */
    fun line(context: Context, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String = when (choice) {
        TitleChoice.AniList -> when {
            !signedIn -> Strings.settingsTitlesLineSignedOut(context)
            viewer != null -> Strings.settingsTitlesLineAccountLanguage(context, name(context, viewer))
            else -> Strings.settingsTitlesLineFollows(context)
        }
        TitleChoice.English -> Strings.settingsTitlesLineEnglish(context)
        TitleChoice.Romaji -> Strings.settingsTitlesLineRomaji(context)
    }

    /** The note under the tablet's examples. */
    fun note(context: Context, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?, examples: List<Entry>): String = when (choice) {
        TitleChoice.AniList ->
            if (signedIn && viewer != null) Strings.settingsTitlesNoteAccount(context, name(context, viewer))
            else Strings.settingsTitlesNoteFollows(context)
        TitleChoice.English -> examples.firstOrNull { it.titleEnglish.isNullOrEmpty() }
            ?.let { Strings.settingsTitlesNoteEnglishMissing(context, it.titleRomaji ?: it.title) }
            ?: Strings.settingsTitlesNoteEnglish(context)
        TitleChoice.Romaji -> Strings.settingsTitlesNoteRomaji(context)
    }

    /** A title language as every sentence names it, with its capital: "English". */
    fun name(context: Context, language: TitleLanguage): String = when (language) {
        TitleLanguage.ROMAJI -> Strings.settingsTitleRomaji(context)
        TitleLanguage.ENGLISH -> Strings.settingsTitleEnglish(context)
        TitleLanguage.NATIVE -> Strings.settingsTitleNative(context)
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
    val context = LocalContext.current
    LaunchedEffect(Unit) { store.loadContinue() }
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_title_language),
        value = choice.label(context),
        choices = TitleChoice.entries.map { MenuChoice(it, it.label(context), example?.let { e -> TitleLanguageWords.example(e, it, signedIn, viewer) }) },
        current = choice,
        note = TitleLanguageWords.line(context, choice, signedIn, viewer),
        enabled = row != null,
        focus = FOCUS_TITLES,
        menuWidth = 300,
    ) { if (it != choice) store.setSetting(SettingKey.TITLES, SettingValue.Str(it.value)) }
}

/**
 * The app's language: the system's per-app setting from Android 13, the app's own choice below.
 * Only the languages this build ships are listed, a draft one labelled so; it follows the
 * system's until one is chosen.
 */
@Composable
private fun LanguageRow(place: Place) {
    val context = LocalContext.current
    var chosen by remember { mutableStateOf(AppLanguage.chosen(context)) }
    val follow = stringResource(R.string.language_follow_system)
    val draft = stringResource(R.string.language_draft_label)
    fun name(language: Language) = if (language.draft) "${language.name} · $draft" else language.name // i18n-skip
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_language),
        value = chosen?.let(::name) ?: follow,
        choices = listOf(MenuChoice<Language?>(null, follow)) + AppLanguage.shipped.map { MenuChoice<Language?>(it, name(it)) },
        current = chosen,
        note = if (chosen?.draft == true) stringResource(R.string.language_draft_note) else null,
    ) { picked ->
        chosen = picked
        AppLanguage.choose(context, picked)
        // From Android 13 the system recreates the activity; below, the app does.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) generateSequence(context) { (it as? android.content.ContextWrapper)?.baseContext }.filterIsInstance<android.app.Activity>().firstOrNull()?.recreate()
    }
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
    MenuRow(place, stringResource(R.string.player_subtitles), languageName(code), choices, code, enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.SUBTITLES, SettingValue.Str(it))
    }
}

/** A language code as its name in the app's language: "English" for "en" or "eng". */
internal fun languageName(code: String): String {
    if (code.isEmpty()) return "–"
    val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.getDefault())
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
        val free = freeBytes()
        val used = store.downloads.bytes(downloads)
        SettingsGroup(
            rows = listOf(
                { place -> StorageRow(place, used, free) },
                { place ->
                    val navigator = LocalNavigator.current
                    InfoRow(
                        place,
                        stringResource(R.string.downloads_title),
                        if (downloads.isEmpty()) stringResource(R.string.settings_none_yet) else pluralStringResource(R.plurals.settings_downloads_count, downloads.size, downloads.size),
                        onClick = { navigator.open(DOWNLOADS_ROUTE) },
                    )
                },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.settings_new_downloads),
            rows = listOfNotNull(
                @Composable { place: Place -> SettingMenu(place, SettingKey.DOWNLOAD_QUALITY, stringResource(R.string.settings_quality)) },
                { place -> SettingMenu(place, SettingKey.KEEP_NEXT_EPISODES, stringResource(R.string.settings_keep_next), note = stringResource(R.string.settings_keep_next_detail)) },
                { place -> WifiOnlyRow(place) },
                { place -> SettingSwitch(place, SettingKey.MERGE_SUBTITLES, stringResource(R.string.settings_merge_subtitles)) },
                if (torrents) @Composable { place: Place -> SettingSwitch(place, SettingKey.KEEP_SEEDING, stringResource(R.string.settings_keep_seeding)) } else null,
            ),
        )
    }
}

/**
 * What the downloads take beside what is free, as a bar: amber for the downloads' share of what
 * the app could use, on the rule.
 */
@Composable
private fun StorageRow(place: Place, used: Long, free: Long?) {
    val colors = MaterialTheme.colorScheme
    val description = stringResource(R.string.downloads_storage_share_description, Wording.bytes(LocalContext.current, used))
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 88.dp)) {
        Column(Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.settings_group_storage), style = MaterialTheme.typography.titleMedium)
            Text(storageWords(LocalContext.current, used, free), style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
            val share = if (free != null && used + free > 0) used.toFloat() / (used + free) else 0f
            androidx.compose.material3.LinearProgressIndicator(
                progress = { share.coerceIn(0f, 1f) },
                modifier = Modifier.padding(top = 8.dp).fillMaxWidth().height(4.dp)
                    .semantics { contentDescription = description },
                // Water, not amber: a share of the disk is neither an action nor progress.
                color = colors.secondary,
                trackColor = colors.outline,
                gapSize = if (share > 0f) 4.dp else 0.dp,
                drawStopIndicator = {},
            )
        }
    }
}

/** Wi-Fi only: downloads on mobile data, turned over, as a phone says it. */
@Composable
private fun WifiOnlyRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.DOWNLOADS_ON_CELLULAR, view)
    SwitchRow(place, stringResource(R.string.settings_wifi_only), checked = !SettingsCatalog.isOn(row), note = stringResource(R.string.settings_wifi_only_detail), enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.DOWNLOADS_ON_CELLULAR, SettingValue.Bool(!it))
    }
}

// Sources

@Composable
internal fun SourcesPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val storesLoaded by store.stores.loaded.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.checkProviders()
        store.loadStores()
    }
    val torrents = store.settings.isOn(SettingKey.TORRENTS, view)
    val updates = stores.sumOf { it.updates }
    var sheet by rememberSaveable { mutableStateOf<TorrentSheetMode?>(null) }
    // The protection picked in the menu, which the sheet then starts from.
    var picked by rememberSaveable { mutableStateOf<TorrentProtection?>(null) }
    val context = LocalContext.current
    val cellular = remember { context.packageManager.hasSystemFeature(android.content.pm.PackageManager.FEATURE_TELEPHONY) }
    Pane {
        SettingsGroup(
            rows = listOf(
                { place ->
                    val navigator = LocalNavigator.current
                    InfoRow(place, stringResource(R.string.sources_order), pluralStringResource(R.plurals.settings_summary_sources, providers.size, providers.size).takeIf { providersLoaded }, onClick = { navigator.open(PROVIDERS_ROUTE) })
                },
                { place ->
                    val navigator = LocalNavigator.current
                    InfoRow(
                        place,
                        stringResource(R.string.nav_plugin_stores),
                        when {
                            !storesLoaded && stores.isEmpty() -> null
                            stores.isEmpty() -> stringResource(R.string.settings_none_joined)
                            updates > 0 -> stringResource(R.string.settings_stores_with_updates, stores.size, pluralStringResource(R.plurals.stores_updates_waiting, updates, updates))
                            else -> NumberFormat.getIntegerInstance().format(stores.size)
                        },
                        onClick = { navigator.open(STORES_ROUTE) },
                    )
                },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.torrents_title),
            footer = if (torrents) checkNote(context, view?.torrent?.onFailure, view?.torrent?.protection) else stringResource(TorrentWords.OFF_NOTE),
            rows = listOfNotNull(
                @Composable { place: Place -> TorrentSourceRow(place, torrents) { sheet = TorrentSheetMode.TurnOn } },
                if (torrents) @Composable { place: Place ->
                    InfoRow(place, stringResource(R.string.torrents_index), view?.sources?.index ?: stringResource(R.string.settings_index_none), onClick = { picked = null; sheet = TorrentSheetMode.Index })
                } else null,
                if (torrents) @Composable { place: Place ->
                    MenuRow(
                        place,
                        stringResource(R.string.torrents_protect_with),
                        protectionWords(context, view?.torrent?.protection),
                        listOf(TorrentProtection.PROXY, TorrentProtection.DEVICE, TorrentProtection.NONE).map { MenuChoice(it, protectionWords(context, it)) },
                        view?.torrent?.protection,
                    ) { choice ->
                        if (choice != view?.torrent?.protection) {
                            picked = choice
                            sheet = TorrentSheetMode.Change
                        }
                    }
                } else null,
                // Beside the choice, once a proxy is saved: its details, applied only once they pass the check.
                if (torrents && view?.torrent?.protection == TorrentProtection.PROXY) @Composable { place: Place ->
                    val proxy = view?.torrent?.proxy
                    InfoRow(
                        place,
                        stringResource(R.string.settings_edit_proxy),
                        proxy?.let { TorrentWords.proxyLine(context, it.host, it.port.toString(), it.user.orEmpty()) },
                        onClick = { picked = null; sheet = TorrentSheetMode.EditProxy },
                    )
                } else null,
                if (torrents) @Composable { place: Place -> LastCheckRow(place) } else null,
                if (torrents) @Composable { place: Place -> FailedCheckRow(place) } else null,
                // Only where the device has a cellular radio to spare.
                if (torrents && cellular) @Composable { place: Place -> SettingSwitch(place, SettingKey.TORRENTS_ON_CELLULAR, stringResource(R.string.settings_torrents_cellular)) } else null,
            ),
        )
        if (torrents) {
            SettingsGroup(
                title = stringResource(R.string.downloads_seeding_title),
                footer = stringResource(R.string.settings_seeding_best_effort),
                rows = listOf(
                    { place -> SettingMenu(place, SettingKey.SEED_RATIO, stringResource(R.string.settings_seed_after)) },
                    { place -> SettingMenu(place, SettingKey.SEED_HOURS, stringResource(R.string.settings_seed_for)) },
                    { place -> SettingMenu(place, SettingKey.UPLOAD_LIMIT, stringResource(R.string.settings_upload_limit)) },
                ),
            )
        }
    }
    sheet?.let { mode ->
        TorrentSheet(mode, start = picked) {
            sheet = null
            picked = null
        }
    }
}

/** The check's note for what a failed check does, and that none runs with no protection. */
internal fun checkNote(context: Context, onFailure: FailedCheck?, protection: TorrentProtection?): String = TorrentWords.checkNote(context, onFailure, protection)

/** The torrent source's switch. Turning it off is a switch; turning it on asks for protection first. */
@Composable
private fun TorrentSourceRow(place: Place, on: Boolean, turnOn: () -> Unit) {
    val store = LocalAppStore.current
    SwitchRow(place, stringResource(R.string.sources_torrent_source), checked = on) { wanted ->
        if (wanted) turnOn() else store.setSwitch(SettingKey.TORRENTS, false)
    }
}

/**
 * If a check fails: pause or stop torrents as a setting, or warn only, which keeps them running
 * on what may be the viewer's own address and so is confirmed first and asked for with the
 * protection it goes with.
 */
@Composable
private fun FailedCheckRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val torrent = view?.torrent
    var warning by remember { mutableStateOf(false) }
    val choices = listOf(
        MenuChoice(FailedCheck.PAUSE, stringResource(R.string.torrents_fail_pause)),
        MenuChoice(FailedCheck.STOP, stringResource(R.string.torrents_fail_stop)),
        MenuChoice(FailedCheck.WARN, stringResource(R.string.torrents_fail_warn)),
    )
    val current = torrent?.onFailure ?: FailedCheck.PAUSE
    MenuRow(
        place,
        stringResource(R.string.torrents_if_check_fails),
        choices.firstOrNull { it.value == current }?.label,
        choices,
        current,
        enabled = torrent != null,
    ) { picked ->
        when (picked) {
            current -> {}
            FailedCheck.WARN -> warning = true
            FailedCheck.PAUSE -> store.setSetting(SettingKey.VPN_ON_FAILURE, SettingValue.Str("pause"))
            FailedCheck.STOP -> store.setSetting(SettingKey.VPN_ON_FAILURE, SettingValue.Str("stop"))
        }
    }
    if (warning && torrent != null) {
        WarnOnlyDialog(onDismiss = { warning = false }) {
            warning = false
            store.setTorrentProtection(
                torrent.protection,
                proxy = torrent.proxy?.let { TorrentProxy(host = it.host, port = it.port, user = it.user, password = null) },
                onFailure = FailedCheck.WARN,
                acknowledged = true,
            )
        }
    }
}

/** "SOCKS5 proxy", "VPN on this device", "No protection". */
internal fun protectionWords(context: Context, protection: TorrentProtection?): String = TorrentWords.protection(context, protection)

/** The connection check's last word, with its dot, where traffic leaves from, and Check now. */
@Composable
private fun LastCheckRow(place: Place) {
    val store = LocalAppStore.current
    val check by store.providers.check.collectAsStateWithLifecycle()
    val view by store.settings.view.collectAsStateWithLifecycle()
    val words = guardWords(LocalContext.current, check, onFailure = view?.torrent?.onFailure)
    val colors = MaterialTheme.colorScheme
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 88.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.torrents_last_check), style = MaterialTheme.typography.titleMedium)
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
            TextButton(onClick = store::checkConnection, enabled = check?.view != GuardView.Checking) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    WorkRing(check?.view == GuardView.Checking)
                    Text(stringResource(R.string.torrents_check_now), style = MaterialTheme.typography.titleMedium, color = colors.onSurface)
                }
            }
        }
    }
}

internal fun guardWords(context: Context, check: ProvidersModel.Check?, now: Long = System.currentTimeMillis(), onFailure: FailedCheck? = null): TorrentWords.Check =
    TorrentWords.check(context, check, now, onFailure)

// Accounts

@Composable
internal fun AccountsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val sheets = LocalAppSheets.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val syncedAt by store.accounts.syncedAt.collectAsStateWithLifecycle()
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val conflicts by store.accounts.conflicts.collectAsStateWithLifecycle()
    var reviewing by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val anilist = trackers["anilist"]
    val storage = trackers.values.firstNotNullOfOrNull { it.storage }
    /** The tracker's own sign-in: Kitsu's sheet, the browser or a code for the rest. */
    val signIn = { tracker: String -> if (tracker == "kitsu") sheets.kitsu = true else store.connect(tracker) }
    Pane {
        SettingsGroup(
            title = "AniList", // i18n-skip
            footer = stringResource(R.string.settings_anilist_footer),
            rows = if (anilist?.connected == true) {
                val status = AccountWords.status(anilist, syncedAt, working)
                listOfNotNull(
                    @Composable { place: Place ->
                        InfoRow(
                            place,
                            anilist.user ?: stringResource(R.string.accounts_signed_in_plain),
                            status.text,
                            leading = { Avatar(anilist.user, 40) },
                            trailing = if (anilist.needsReauth) ({ SignInButton(stringResource(R.string.accounts_sign_in_again_button), busy = store.accounts.isWorking(working, "anilist")) { signIn("anilist") } }) else null,
                        )
                    },
                    @Composable { place: Place -> ActionRow(place, stringResource(R.string.accounts_sync_now), enabled = "sync" !in working.values, onClick = store::syncNow) },
                    if (conflicts.isNotEmpty()) @Composable { place: Place ->
                        ActionRow(place, pluralStringResource(R.plurals.accounts_review_differences, conflicts.size, conflicts.size)) { reviewing = true }
                    } else null,
                    @Composable { place: Place -> ActionRow(place, stringResource(R.string.accounts_sign_out)) { store.disconnect("anilist") } },
                )
            } else {
                listOf(@Composable { place: Place ->
                    InfoRow(
                        place,
                        "AniList", // i18n-skip
                        stringResource(R.string.accounts_brings_lists),
                        trailing = { SignInButton(stringResource(R.string.accounts_sign_in), busy = store.accounts.isWorking(working, "anilist")) { signIn("anilist") } },
                    )
                })
            },
        )
        SettingsGroup(
            title = stringResource(R.string.discussion_settings_group),
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.EPISODE_DISCUSSIONS, stringResource(R.string.settings_discussions_label), note = stringResource(R.string.settings_discussions_note)) },
                { place -> SettingSwitch(place, SettingKey.COMMENT_IMAGES, stringResource(R.string.settings_discussion_images_label), note = stringResource(R.string.settings_discussion_images_note)) },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.accounts_other_trackers),
            footer = stringResource(if (storage?.contains("file", ignoreCase = true) == true) R.string.accounts_tokens_file else R.string.accounts_tokens_keystore),
            rows = AccountsModel.order.drop(1).map { tracker ->
                @Composable { place: Place ->
                    val state = trackers[tracker]
                    val connected = state?.connected == true
                    val busy = store.accounts.isWorking(working, tracker)
                    val button: @Composable () -> Unit = when {
                        connected && state?.needsReauth == true -> ({ SignInButton(stringResource(R.string.accounts_sign_in_again_button), busy) { signIn(tracker) } })
                        connected -> ({ TextButton(onClick = { store.disconnect(tracker) }) { Text(stringResource(R.string.accounts_sign_out), color = MaterialTheme.colorScheme.onSurface) } })
                        else -> ({ SignInButton(stringResource(R.string.accounts_sign_in), busy) { signIn(tracker) } })
                    }
                    InfoRow(
                        place,
                        AccountsModel.name(tracker),
                        if (connected) AccountWords.status(state, syncedAt, working).text else stringResource(R.string.accounts_state_signed_out),
                        trailing = button,
                    )
                }
            },
        )
        TvSettingsGroup()
    }
    if (reviewing) ConflictsSheet { reviewing = false }
}

/** Sign in, tonal: a row's own action, with the screen's one amber action kept for elsewhere. */
@Composable
private fun SignInButton(label: String, busy: Boolean, onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick, enabled = !busy) { WorkLabel(label, busy) }
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

/** The privacy page: what anistream sends, where, and what it keeps on the device. */
internal const val PRIVACY_URL = "https://anistream.tv/docs/privacy/"

/**
 * About: Nagare, the name and the version; Updates, with the newer release's row while there is
 * one and the daily check's switch; then Privacy and the licences.
 */
@Composable
internal fun AboutPane() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val update by store.updates.state.collectAsStateWithLifecycle()
    Pane {
        Column(Modifier.fillMaxWidth().padding(vertical = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Nagare(cell = 6)
            Text("anistream", // i18n-skip
                style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(top = 16.dp))
            Text(stringResource(R.string.about_version, appVersion()), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        SettingsGroup(
            title = stringResource(R.string.updates_title),
            rows = listOfNotNull(
                if (update.offer != null) @Composable { place: Place -> UpdateRow(place) } else null,
                { place -> CheckNowRow(place) },
                { place -> SettingSwitch(place, SettingKey.UPDATE_CHECK, stringResource(R.string.settings_check_updates), note = stringResource(R.string.updates_schedule)) },
            ),
        )
        SettingsGroup(
            footer = stringResource(R.string.about_report_bug_detail),
            rows = listOf(
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.about_report_bug),
                        stringResource(R.string.settings_report_bug_value),
                        onClick = { openBugReport(context, store) },
                        trailing = { Icon(Icons.External, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
                    )
                },
            ),
        )
        SettingsGroup(
            footer = LEGAL_LINE,
            rows = listOf(
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.settings_privacy),
                        stringResource(R.string.settings_privacy_detail),
                        onClick = { store.openExternal(PRIVACY_URL) },
                        trailing = { Icon(Icons.External, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
                    )
                },
                { place -> InfoRow(place, stringResource(R.string.settings_licence_title), stringResource(R.string.settings_licence_value)) },
                { place -> InfoRow(place, stringResource(R.string.settings_typefaces), stringResource(R.string.settings_typefaces_value)) },
                { place -> InfoRow(place, stringResource(R.string.settings_libraries), stringResource(R.string.settings_libraries_value)) },
            ),
        )
    }
}
