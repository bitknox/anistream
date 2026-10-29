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
import androidx.compose.ui.platform.LocalContext
import android.content.Context
import androidx.annotation.StringRes
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.AppStore

/** The Show settings panel for [id], titled by the show. */
fun showSettingsSheet(context: Context, store: AppStore, id: AnilistId, title: String): ActionSheet =
    ActionSheet(Strings.showSettingsTitle(context), body = { ShowSettingsBody(store, id, title) })

/** One row of the panel: its name, its key, and its choices with their words. */
private data class ShowChoice(@StringRes val label: Int, val key: ShowSettingKey, val choices: List<Pair<SettingValue, (Context) -> String>>)

private val ShowChoices = listOf(
    ShowChoice(R.string.player_audio, ShowSettingKey.TRANSLATION, listOf(SettingValue.Str("sub") to { _ -> languageName("ja") }, SettingValue.Str("dub") to { _ -> languageName("en") })),
    ShowChoice(R.string.show_settings_skip_opening, ShowSettingKey.SKIP_OPENING, onOff(R.string.value_on, R.string.value_off)),
    ShowChoice(R.string.show_settings_skip_filler, ShowSettingKey.SKIP_FILLER, onOff(R.string.value_on, R.string.value_off)),
    ShowChoice(R.string.settings_spoiler, ShowSettingKey.SPOILER_PROTECTION, onOff(R.string.show_settings_blur_ahead, R.string.show_settings_show_everything)),
)

private fun onOff(@StringRes on: Int, @StringRes off: Int): List<Pair<SettingValue, (Context) -> String>> =
    listOf(SettingValue.Bool(true) to { c: Context -> c.getString(on) }, SettingValue.Bool(false) to { c: Context -> c.getString(off) })

@Composable
private fun ShowSettingsBody(store: AppStore, id: AnilistId, title: String) {
    val host = LocalActions.current
    val context = LocalContext.current
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
                stringResource(choice.label),
                onClick = { host?.open(choiceSheet(context, store, id, choice, row)) },
                value = row?.let { valueWords(context, choice, it) },
                enabled = row != null,
                modifier = if (choice === ShowChoices.first()) Modifier.focusRequester(first) else Modifier,
            )
            // The subtitles go with the audio: what the player remembers for it, chosen there.
            if (choice.key == ShowSettingKey.TRANSLATION) SubtitlesRow(store, view)
        }
        item(key = "source") { SourceRow(store, id, title, view) }
        item(key = "reset") {
            TvListRow(
                stringResource(R.string.show_settings_reset),
                onClick = {
                    reset = true
                    store.resetShowSettings(id)
                },
                supporting = when {
                    reset && !anySet -> stringResource(R.string.tv_show_back_to_defaults)
                    anySet -> stringResource(R.string.show_settings_reset_detail)
                    else -> stringResource(R.string.show_settings_nothing)
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
    val context = LocalContext.current
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val audio = view?.rows?.firstOrNull { it.key == ShowSettingKey.TRANSLATION }
    val translation = if (audio?.value?.startsWith("dub") == true) Translation.DUB else Translation.SUB
    val remembered = view?.remembered?.firstOrNull { it.translation == translation }?.subtitles
    val language = store.settings.row(SettingKey.SUBTITLES, settings)?.value?.let(::languageName)
    // Text, not a row to land on: Center would open nothing.
    TvInfoRow(
        stringResource(R.string.player_subtitles),
        supporting = stringResource(if (remembered != null) R.string.show_settings_subs_remembered else R.string.show_settings_subs_follow),
        value = remembered?.let { subtitleWords(context, it) } ?: listOfNotNull(Strings.showSettingsDefault(context), language).joinToString(" · "),
    )
}

/**
 * The source the show keeps, as its value; Center opens the one source picker, with each source's
 * coverage and the releases under it.
 */
@Composable
private fun SourceRow(store: AppStore, id: AnilistId, name: String, view: ShowSettingsView?) {
    val host = LocalActions.current
    val context = LocalContext.current
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (rows.isEmpty()) store.checkProviders() }
    val sources = remember(rows) { store.orderedProviders().filter { !it.disabled } }
    val kept = view?.source
    val label = Strings.sourcePickerTitle(context)
    TvListRow(
        stringResource(R.string.show_settings_source),
        onClick = { host?.open(sourcePickerSheet(store, id, name, label)) },
        supporting = stringResource(R.string.show_settings_source_detail),
        value = kept?.let { source -> sources.firstOrNull { it.id == source }?.let { sourceName(context, it) } ?: source } ?: stringResource(R.string.value_automatic),
        enabled = view != null && sources.isNotEmpty(),
    )
}

/** A source as a person names it: the plugin's own name, or Torrents. */
private fun sourceName(context: Context, row: ProviderRow): String = when (row.kind) {
    "torrent" -> Strings.sourcesTorrents(context)
    "remote" -> Strings.sourcesRemote(context)
    else -> row.name.ifEmpty { row.id }
}

/** A language by its name in English: "Japanese" for `ja`, or the code where none is known. */
fun languageName(code: String): String {
    if (code.isEmpty()) return "–"
    val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.getDefault())
    return if (name.isNullOrEmpty() || name.equals(code, ignoreCase = true)) code else name
}

/** A remembered subtitle track: "Off", "English", or "English (Signs)" with its title. */
fun subtitleWords(context: Context, subtitles: RememberedSubtitles): String = when (subtitles) {
    RememberedSubtitles.Off -> Strings.valueOff(context)
    is RememberedSubtitles.Track -> trackWords(context, subtitles.track)
}

/** A track by its language, then its own title past the language it repeats. */
fun trackWords(context: Context, track: RememberedTrack): String {
    val language = track.language?.let(::languageName)
    val title = track.title.split(' ').filter { word -> language == null || !word.equals(language, ignoreCase = true) }
        .joinToString(" ").trim().replaceFirstChar { it.titlecase(Locale.getDefault()) }
    return when {
        language == null && title.isEmpty() -> Strings.playerTrackUnnamed(context)
        language == null -> title
        title.isEmpty() -> language
        else -> Strings.showSettingsTrackWith(context, language, title)
    }
}

/** What a row says now: the show's own choice, else what following Settings gives. */
private fun valueWords(context: Context, choice: ShowChoice, row: ShowSettingRow): String {
    val own = row.own
    if (own != null) return choice.choices.firstOrNull { matches(it.first, own) }?.second?.invoke(context) ?: row.value
    return defaultWords(context, choice.key, row.default)
}

private fun matches(a: SettingValue, b: SettingValue): Boolean = when {
    a is SettingValue.Str && b is SettingValue.Str -> b.v1.startsWith(a.v1)
    else -> a == b
}

/**
 * "Use default · On", as the phone writes following Settings. Audio says the language, as its
 * choices do: "Use default · Japanese", not the engine's "Sub".
 */
private fun defaultWords(context: Context, key: ShowSettingKey, value: String): String {
    val words = value.removePrefix("Use Default · ")
    if (key == ShowSettingKey.TRANSLATION) return Strings.valueUseDefaultIs(context, languageName(if (words.lowercase().startsWith("dub")) "en" else "ja"))
    val shown = when (words.lowercase()) {
        "on" -> Strings.valueOn(context)
        "off" -> Strings.valueOff(context)
        else -> words.replaceFirstChar { it.titlecase(Locale.getDefault()) }
    }
    return Strings.valueUseDefaultIs(context, shown)
}

/** A row's radio panel: Use default, then its choices; the one in effect marked. */
private fun choiceSheet(context: Context, store: AppStore, id: AnilistId, choice: ShowChoice, row: ShowSettingRow?): ActionSheet {
    val own = row?.own
    return ActionSheet(
        context.getString(choice.label),
        listOf(CardAction(row?.let { defaultWords(context, choice.key, it.default) } ?: Strings.valueUseDefault(context), selected = own == null) { store.setShowSetting(id, choice.key, null) }) +
            choice.choices.map { (value, words) ->
                CardAction(words(context), selected = own != null && matches(value, own)) { store.setShowSetting(id, choice.key, value) }
            },
    )
}
