// Show settings (TVShowSettings): the panel of one show's own choices, each row opening its radio
// panel. A choice applies at once; "Use default" has the show follow Settings again. Below the
// choices: the subtitles the player remembers, the source the show keeps, and Reset. New episodes
// is left out, since the TV posts no notifications (the launcher's Watch Next row takes their
// place), and so are Quality and Keep next for now.

package tv.anistream.tv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.Text
import java.util.Locale
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.ProviderRow
import tv.anistream.engine.model.RememberedSubtitles
import tv.anistream.engine.model.RememberedTrack
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.ShowSettingKey
import tv.anistream.engine.model.ShowSettingRow
import tv.anistream.engine.model.ShowSettingsView
import tv.anistream.store.AppStore

/** The Show settings panel for [id], titled by the show. */
fun showSettingsSheet(store: AppStore, id: AnilistId, title: String): ActionSheet =
    ActionSheet("Settings for this show", body = { ShowSettingsBody(store, id, title) })

/** One row of the panel: its name, its key, and its choices with their words. */
private data class ShowChoice(val label: String, val key: ShowSettingKey, val choices: List<Pair<SettingValue, String>>)

private val ShowChoices = listOf(
    ShowChoice("Audio", ShowSettingKey.TRANSLATION, listOf(SettingValue.Str("sub") to "Japanese", SettingValue.Str("dub") to "English")),
    ShowChoice("Skip opening", ShowSettingKey.SKIP_OPENING, onOff("On", "Off")),
    ShowChoice("Skip filler", ShowSettingKey.SKIP_FILLER, onOff("On", "Off")),
    ShowChoice("Spoilers", ShowSettingKey.SPOILER_PROTECTION, onOff("Blur stills ahead", "Show everything")),
)

private fun onOff(on: String, off: String) = listOf(SettingValue.Bool(true) to on, SettingValue.Bool(false) to off)

@Composable
private fun ShowSettingsBody(store: AppStore, id: AnilistId, title: String) {
    val host = LocalActions.current
    val sheets by store.showSettings.sheets.collectAsStateWithLifecycle()
    LaunchedEffect(id) { store.loadShowSettings(id) }
    val view = sheets[id]
    val first = remember { FocusRequester() }
    // Pressed this time the panel is open, so the row says what it did.
    var reset by remember { mutableStateOf(false) }
    val anySet = view != null && (view.rows.any { it.own != null } || view.remembered.isNotEmpty() || view.source != null)
    LaunchedEffect(anySet) { if (anySet) reset = false }
    // The show's name stays under the panel title while the rows scroll.
    Text(title, style = TvTitle.list, color = ObiSilk.Haze, maxLines = 1, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp))
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp), contentPadding = PaddingValues(bottom = 27.dp)) {
        items(ShowChoices, key = { it.key.name }) { choice ->
            val row = view?.rows?.firstOrNull { it.key == choice.key }
            TvListRow(
                choice.label,
                onClick = { host?.open(choiceSheet(store, id, choice, row)) },
                value = row?.let { valueWords(choice, it) },
                enabled = row != null,
                modifier = if (choice === ShowChoices.first()) Modifier.focusRequester(first) else Modifier,
            )
            // The subtitles go with the audio: what the player remembers for it, chosen there.
            if (choice.key == ShowSettingKey.TRANSLATION) SubtitlesRow(store, view)
        }
        item(key = "source") { SourceRow(store, id, view) }
        item(key = "reset") {
            TvListRow(
                "Reset to defaults",
                onClick = {
                    reset = true
                    store.resetShowSettings(id)
                },
                supporting = when {
                    reset && !anySet -> "Back to defaults"
                    anySet -> "Also clears the remembered audio and subtitles"
                    else -> "Nothing set for this show"
                },
                // Nothing to reset: the row stays, dimmed, so the panel doesn't change shape. Once
                // pressed it stays enabled, so focus isn't dropped from under the viewer.
                enabled = anySet || reset,
            )
        }
    }
    LaunchedEffect(view != null) { if (view != null) first.focusSoon() }
}

/**
 * The show's subtitles, read only: what the player remembers for the audio in effect, or
 * Settings' language. They are chosen in the player, so Center opens nothing.
 */
@Composable
private fun SubtitlesRow(store: AppStore, view: ShowSettingsView?) {
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val audio = view?.rows?.firstOrNull { it.key == ShowSettingKey.TRANSLATION }
    val translation = if (audio?.value?.startsWith("dub") == true) Translation.DUB else Translation.SUB
    val remembered = view?.remembered?.firstOrNull { it.translation == translation }?.subtitles
    val language = store.settings.row(SettingKey.SUBTITLES, settings)?.value?.let(::languageName)
    // Text, not a row to land on: Center would open nothing.
    TvInfoRow(
        "Subtitles",
        supporting = if (remembered != null) "Chosen in the player, remembered from your last episode" else "Chosen in the player",
        value = remembered?.let(::subtitleWords) ?: listOfNotNull("Default", language).joinToString(" · "),
    )
}

/**
 * The source the show keeps: Automatic, or one of the sources on, in the order failover tries
 * them. Picking one pins it for this show; Automatic gives the show back to the order.
 */
@Composable
private fun SourceRow(store: AppStore, id: AnilistId, view: ShowSettingsView?) {
    val host = LocalActions.current
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (rows.isEmpty()) store.checkProviders() }
    val sources = remember(rows) { store.orderedProviders().filter { !it.disabled } }
    val kept = view?.source
    val name = { source: String -> sources.firstOrNull { it.id == source }?.let(::sourceName) ?: source }
    TvListRow(
        "Source",
        onClick = {
            val pick = { source: String? ->
                store.setProviderPreference(id, source)
                // The panel says the show's source as the engine now keeps it.
                store.loadShowSettings(id)
            }
            host?.open(
                ActionSheet(
                    "Source",
                    listOf(CardAction("Automatic", selected = kept == null) { pick(null) }) +
                        sources.map { source -> CardAction(sourceName(source), selected = source.id == kept) { pick(source.id) } },
                ),
            )
        },
        supporting = "Automatic tries your sources in order",
        value = kept?.let(name) ?: "Automatic",
        enabled = view != null && sources.isNotEmpty(),
    )
}

/** A source as a person names it: the plugin's own name, or Torrents. */
private fun sourceName(row: ProviderRow): String = when (row.kind) {
    "torrent" -> "Torrents"
    "remote" -> "Remote"
    else -> row.name.ifEmpty { row.id }
}

/** A language by its name in English: "Japanese" for `ja`, or the code where none is known. */
fun languageName(code: String): String {
    if (code.isEmpty()) return "–"
    val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.UK)
    return if (name.isNullOrEmpty() || name.equals(code, ignoreCase = true)) code else name
}

/** A remembered subtitle track: "Off", "English", or "English (Signs)" with its title. */
fun subtitleWords(subtitles: RememberedSubtitles): String = when (subtitles) {
    RememberedSubtitles.Off -> "Off"
    is RememberedSubtitles.Track -> trackWords(subtitles.track)
}

/** A track by its language, then its own title past the language it repeats. */
fun trackWords(track: RememberedTrack): String {
    val language = track.language?.let(::languageName)
    val title = track.title.split(' ').filter { word -> language == null || !word.equals(language, ignoreCase = true) }
        .joinToString(" ").trim().replaceFirstChar { it.titlecase(Locale.UK) }
    return when {
        language == null && title.isEmpty() -> "Unnamed track"
        language == null -> title
        title.isEmpty() -> language
        else -> "$language ($title)"
    }
}

/** What a row says now: the show's own choice, else what following Settings gives. */
private fun valueWords(choice: ShowChoice, row: ShowSettingRow): String {
    val own = row.own
    if (own != null) return choice.choices.firstOrNull { matches(it.first, own) }?.second ?: row.value
    return defaultWords(choice.key, row.default)
}

private fun matches(a: SettingValue, b: SettingValue): Boolean = when {
    a is SettingValue.Str && b is SettingValue.Str -> b.v1.startsWith(a.v1)
    else -> a == b
}

/**
 * "Use default · On", as the phone writes following Settings. Audio says the language, as its
 * choices do: "Use default · Japanese", not the engine's "Sub".
 */
private fun defaultWords(key: ShowSettingKey, value: String): String {
    val words = value.removePrefix("Use Default · ")
    if (key == ShowSettingKey.TRANSLATION) return "Use default · " + if (words.lowercase(Locale.UK).startsWith("dub")) "English" else "Japanese"
    return "Use default · " + words.replaceFirstChar { it.titlecase(Locale.UK) }
}

/** A row's radio panel: Use default, then its choices; the one in effect marked. */
private fun choiceSheet(store: AppStore, id: AnilistId, choice: ShowChoice, row: ShowSettingRow?): ActionSheet {
    val own = row?.own
    return ActionSheet(
        choice.label,
        listOf(CardAction(row?.let { defaultWords(choice.key, it.default) } ?: "Use default", selected = own == null) { store.setShowSetting(id, choice.key, null) }) +
            choice.choices.map { (value, words) ->
                CardAction(words, selected = own != null && matches(value, own)) { store.setShowSetting(id, choice.key, value) }
            },
    )
}
