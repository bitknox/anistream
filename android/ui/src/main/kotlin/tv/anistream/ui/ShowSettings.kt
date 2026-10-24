// Settings for this show (AndroidShowSettings): a modal bottom sheet from the title page's More
// menu with the show's audio and subtitles, its auto-skip, its spoiler protection and its source,
// each following Settings until the show sets its own, and a reset that puts it back.

package tv.anistream.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.util.Locale
import tv.anistream.design.ShowTitle
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.core.Translation
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.store.SettingsCatalog
import tv.anistream.engine.model.RememberedSubtitles
import tv.anistream.engine.model.RememberedTrack
import tv.anistream.engine.model.ShowSettingKey
import tv.anistream.engine.model.ShowSettingRow
import tv.anistream.engine.model.ShowSettingsView

/** The sheet, over the title page, with one detent and its top at 77, as T7 has it. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun ShowSettingsSheet(entry: Entry, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val sheets by store.showSettings.sheets.collectAsStateWithLifecycle()
    LaunchedEffect(entry.id) { store.loadShowSettings(entry.id) }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        // T7: the scrim black at 32 %, the handle haze at 40 %.
        scrimColor = androidx.compose.ui.graphics.Color.Black.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)) },
        modifier = Modifier.padding(top = 77.dp - 24.dp),
    ) {
        ShowSettingsContent(entry, sheets[entry.id])
    }
}

@Composable
private fun ShowSettingsContent(entry: Entry, view: ShowSettingsView?) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    fun row(key: ShowSettingKey) = view?.rows?.firstOrNull { it.key == key }
    fun set(key: ShowSettingKey, value: SettingValue?) = store.setShowSetting(entry.id, key, value)
    // As tall as the sheet goes, its top at 77, whatever the rows below need.
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
        Row(
            Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CoverArt(entry, 40.dp, Modifier.size(40.dp, 60.dp))
            Column {
                Text("Settings for this show", style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
                Text(entry.showTitle, style = ShowTitle.row, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }

        SheetHeading("Audio and subtitles")
        val audio = row(ShowSettingKey.TRANSLATION)
        val translation = if (audio?.value?.startsWith("dub") == true) Translation.DUB else Translation.SUB
        ChoiceRow(
            title = "Audio",
            value = audio?.let { audioWords(it.value) },
            choices = listOf(
                Choice(SettingValue.Str("sub"), "Japanese"),
                Choice(SettingValue.Str("dub"), "English"),
            ),
            current = audio?.let { SettingValue.Str(if (translation == Translation.DUB) "dub" else "sub") },
            enabled = audio != null,
        ) { set(ShowSettingKey.TRANSLATION, it) }
        val remembered = view?.remembered?.firstOrNull { it.translation == translation }?.subtitles
        SubtitlesRow(view, remembered)
        SheetNote(if (remembered != null) "Chosen in the player. Remembered from your last episode." else "Chosen in the player. Following Settings › Playback.")

        SheetHeading("Episodes")
        SwitchChoiceRow("Skip opening", row(ShowSettingKey.SKIP_OPENING), "On", "Off") { set(ShowSettingKey.SKIP_OPENING, it) }
        SwitchChoiceRow("Skip filler", row(ShowSettingKey.SKIP_FILLER), "On", "Off") { set(ShowSettingKey.SKIP_FILLER, it) }
        SwitchChoiceRow("Spoiler protection", row(ShowSettingKey.SPOILER_PROTECTION), "Always", "Never") {
            set(ShowSettingKey.SPOILER_PROTECTION, it)
        }
        SheetNote("Spoiler protection blurs stills after the next episode.")

        SheetDivider()
        // Whether this show's new episodes are notified of: On notifies whatever Settings says, and
        // asks for the permission to post first, as Settings' switch does.
        val continuing by store.continuing.list.state.collectAsStateWithLifecycle()
        val notify = row(ShowSettingKey.NOTIFY_NEW_EPISODES)
        val notifications = LocalNotificationActions.current
        SwitchChoiceRow("New episodes", notify, "On", "Off", default = notify?.let { notifyDefault(it, beingWatched(entry, continuing.ids)) }) { value ->
            val switch: () -> Unit = { set(ShowSettingKey.NOTIFY_NEW_EPISODES, value) }
            if (value == SettingValue.Bool(true) && notifications != null) notifications.permit(switch) else switch()
        }

        SheetDivider()
        SourceRow(entry, view)
        SheetNote("Automatic tries your sources in order.")

        SheetDivider()
        Column(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .clickable(enabled = view != null, role = Role.Button) { store.resetShowSettings(entry.id) }
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Text("Reset to defaults", style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold))
            Text("Also clears the remembered audio and subtitles.", style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
    }
}

/**
 * The show's subtitles, read only: what the player remembers for the audio in effect, or
 * Settings' language. They are chosen in the player, so the row offers no menu.
 */
@Composable
private fun SubtitlesRow(view: ShowSettingsView?, remembered: RememberedSubtitles?) {
    val store = LocalAppStore.current
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val language = store.settings.row(tv.anistream.engine.model.SettingKey.SUBTITLES, settings)?.value
        ?.let(::languageName)
    val default = listOfNotNull("Default", language).joinToString(" · ")
    // TODO(engine): no command sets or forgets a show's remembered subtitles outside the player;
    // once one does, this becomes a menu like Audio.
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .padding(horizontal = 16.dp, vertical = 12.dp)
            .semantics(mergeDescendants = true) {},
        verticalArrangement = Arrangement.Center,
    ) {
        Text("Subtitles", style = MaterialTheme.typography.bodyLarge)
        Text(
            remembered?.let(::subtitleWords) ?: default,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * The source the show keeps: Automatic, or one of the sources on, in the order failover tries
 * them. Picking one pins it for this show; Automatic gives the show back to the order.
 */
@Composable
private fun SourceRow(entry: Entry, view: ShowSettingsView?) {
    val store = LocalAppStore.current
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (rows.isEmpty()) store.checkProviders() }
    val sources = store.orderedProviders().filter { !it.disabled }
    fun name(id: String) = sources.firstOrNull { it.id == id }?.let(::sourceName) ?: id
    ChoiceRow(
        title = "Source",
        value = view?.source?.let(::name) ?: "Automatic",
        choices = listOf(Choice(null, "Automatic")) + sources.map { Choice(SettingValue.Str(it.id), sourceName(it)) },
        current = view?.source?.let { SettingValue.Str(it) },
        enabled = view != null && sources.isNotEmpty(),
        currentIsDefault = view?.source == null,
    ) { picked ->
        store.setProviderPreference(entry.id, (picked as? SettingValue.Str)?.v1)
        // The sheet says the show's source as the engine now keeps it.
        store.loadShowSettings(entry.id)
    }
}

/** A source as a person names it: the plugin's own name, or Torrents. */
private fun sourceName(row: tv.anistream.engine.model.ProviderRow): String = when (row.kind) {
    "torrent" -> "Torrents"
    "remote" -> "Remote"
    else -> row.name.ifEmpty { row.id }
}

/** A choice in a row's menu: the value it sets, and its words. */
private data class Choice(val value: SettingValue?, val label: String)

/**
 * A setting the show can set or leave to Settings: "Use default · On", then its two values, each
 * as a show sets it.
 */
@Composable
private fun SwitchChoiceRow(
    title: String,
    row: ShowSettingRow?,
    on: String,
    off: String,
    /** What following Settings means for this show, where it is not Settings' own value. */
    default: String? = row?.default?.let(::defaultWords),
    onPick: (SettingValue?) -> Unit,
) {
    val own = (row?.own as? SettingValue.Bool)?.v1
    ChoiceRow(
        title = title,
        value = when (own) {
            true -> on
            false -> off
            null -> default
        },
        choices = listOf(Choice(null, default ?: "Use default"), Choice(SettingValue.Bool(true), on), Choice(SettingValue.Bool(false), off)),
        current = row?.own,
        currentIsDefault = row != null && row.own == null,
        enabled = row != null,
        onPick = onPick,
    )
}

/** A row that drops down a menu of its choices, the one in effect checked. */
@Composable
private fun ChoiceRow(
    title: String,
    value: String?,
    choices: List<Choice>,
    current: SettingValue?,
    enabled: Boolean,
    currentIsDefault: Boolean = false,
    onPick: (SettingValue?) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .clickable(enabled = enabled) { open = true }
                .padding(horizontal = 16.dp, vertical = 12.dp)
                .semantics(mergeDescendants = true) {},
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.bodyLarge)
                value?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
            }
            Icon(Icons.MenuArrow, contentDescription = null, tint = colors.onSurfaceVariant)
        }
        Menu(expanded = open, onDismissRequest = { open = false }) {
            for (choice in choices) {
                val checked = if (choice.value == null) currentIsDefault else !currentIsDefault && choice.value == current
                DropdownMenuItem(
                    text = { Text(choice.label, style = MaterialTheme.typography.labelLarge) },
                    leadingIcon = {
                        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                            if (checked) Icon(Icons.Check, contentDescription = "Current")
                        }
                    },
                    modifier = Modifier.heightIn(min = 48.dp),
                    onClick = {
                        open = false
                        onPick(choice.value)
                    },
                )
            }
        }
    }
}

@Composable
private fun SheetHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 16.dp, bottom = 4.dp).semantics { heading() },
    )
}

@Composable
private fun SheetNote(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 4.dp, bottom = 12.dp),
    )
}

@Composable
private fun SheetDivider() {
    HorizontalDivider(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), color = MaterialTheme.colorScheme.outlineVariant)
}

/** The audio a translation plays: Japanese for subtitled, English for dubbed. */
internal fun audioWords(value: String): String = if (value.startsWith("dub")) "English" else "Japanese"

/**
 * Whether the show is being watched, as the new-episode check follows shows: on the list as
 * watching, or under way here, on Continue.
 */
internal fun beingWatched(entry: Entry, continuing: List<AnilistId>): Boolean =
    entry.listStatus == LibrarySegment.WATCHING || entry.id in continuing

/**
 * What following Settings means for new episodes: Settings' value for a show being watched, and
 * off for one that is not, which only its own switch takes in: "Off · not watching".
 */
internal fun notifyDefault(row: ShowSettingRow, watching: Boolean): String =
    if (SettingsCatalog.isOnWords(row.default) && !watching) "Off · not watching" else defaultWords(row.default)

/** What following Settings means for a row: "Use default · On", from Settings' "on". */
internal fun defaultWords(value: String): String =
    "Use default · " + value.removePrefix("Use Default · ").replaceFirstChar { it.titlecase(Locale.UK) }

/** A remembered subtitle track: "Off", "English", or "English (Hotaru)" with its title. */
internal fun subtitleWords(subtitles: RememberedSubtitles): String = when (subtitles) {
    RememberedSubtitles.Off -> "Off"
    is RememberedSubtitles.Track -> trackWords(subtitles.track)
}

internal fun trackWords(track: RememberedTrack): String {
    val language = track.language?.let { code ->
        val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.UK)
        if (name.isNullOrEmpty() || name.equals(code, ignoreCase = true)) code else name
    }
    // The title in words of its own, past the language it repeats: "english signs" is "signs".
    val title = track.title.split(' ').filter { word -> language == null || !word.equals(language, ignoreCase = true) }
        .joinToString(" ").replaceFirstChar { it.titlecase(Locale.UK) }
    return when {
        language == null && title.isEmpty() -> "Unnamed track"
        language == null -> title
        title.isEmpty() -> language
        else -> "$language ($title)"
    }
}
