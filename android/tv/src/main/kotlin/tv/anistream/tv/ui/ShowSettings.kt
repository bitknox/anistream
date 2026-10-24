// Show settings (TVShowSettings): the panel of one show's own choices, each row opening its radio
// panel. A choice applies at once; "Use default" has the show follow Settings again.

package tv.anistream.tv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
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
import tv.anistream.engine.model.ShowSettingKey
import tv.anistream.engine.model.ShowSettingRow
import tv.anistream.store.AppStore

/** The Show settings panel for [id], titled by the show. */
fun showSettingsSheet(store: AppStore, id: AnilistId, title: String): ActionSheet =
    ActionSheet("Settings for this show", body = { ShowSettingsBody(store, id, title) })

/** One row of the panel: its name, its key, and its choices with their words. */
private data class ShowChoice(val label: String, val key: ShowSettingKey, val choices: List<Pair<SettingValue, String>>)

private val ShowChoices = listOf(
    ShowChoice("Audio", ShowSettingKey.TRANSLATION, listOf(SettingValue.Str("sub") to "Japanese", SettingValue.Str("dub") to "English")),
    ShowChoice("Skip opening automatically", ShowSettingKey.SKIP_OPENING, onOff("On", "Off")),
    ShowChoice("Skip filler automatically", ShowSettingKey.SKIP_FILLER, onOff("On", "Off")),
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
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item(key = "title") {
            Text(title, style = TvTitle.list, color = ObiSilk.Haze, maxLines = 1, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp))
        }
        items(ShowChoices, key = { it.key.name }) { choice ->
            val row = view?.rows?.firstOrNull { it.key == choice.key }
            TvListRow(
                choice.label,
                onClick = { host?.open(choiceSheet(store, id, choice, row)) },
                value = row?.let { valueWords(choice, it) },
                enabled = row != null,
                modifier = if (choice === ShowChoices.first()) Modifier.focusRequester(first) else Modifier,
            )
        }
    }
    LaunchedEffect(view != null) { if (view != null) first.focusSoon() }
}

/** What a row says now: the show's own choice, else what following Settings gives. */
private fun valueWords(choice: ShowChoice, row: ShowSettingRow): String {
    val own = row.own
    if (own != null) return choice.choices.firstOrNull { matches(it.first, own) }?.second ?: row.value
    return defaultWords(row.default)
}

private fun matches(a: SettingValue, b: SettingValue): Boolean = when {
    a is SettingValue.Str && b is SettingValue.Str -> b.v1.startsWith(a.v1)
    else -> a == b
}

/** "Use default · on", as the phone writes following Settings. */
private fun defaultWords(value: String): String =
    "Use default · " + value.removePrefix("Use Default · ").replaceFirstChar { it.titlecase(Locale.UK) }

/** A row's radio panel: Use default, then its choices; the one in effect marked. */
private fun choiceSheet(store: AppStore, id: AnilistId, choice: ShowChoice, row: ShowSettingRow?): ActionSheet {
    val own = row?.own
    return ActionSheet(
        choice.label,
        listOf(CardAction(row?.let { defaultWords(it.default) } ?: "Use default", selected = own == null) { store.setShowSetting(id, choice.key, null) }) +
            choice.choices.map { (value, words) ->
                CardAction(words, selected = own != null && matches(value, own)) { store.setShowSetting(id, choice.key, value) }
            },
    )
}
