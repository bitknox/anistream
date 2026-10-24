// The config as the engine shows it, and the choices each stepped setting takes.

package tv.anistream.store

import java.util.Locale
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.SettingChange
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.SettingRow
import tv.anistream.engine.model.SettingsView

/** The settings, as the engine last said them. */
class SettingsModel {
    private val _view = MutableStateFlow<SettingsView?>(null)

    /** The whole view, from the first `Settings` event at launch on. */
    val view: StateFlow<SettingsView?> = _view.asStateFlow()

    internal fun apply(view: SettingsView) {
        _view.value = view
    }

    /** One row, from the Settings screen's rows or the native apps' own. */
    fun row(key: SettingKey, view: SettingsView? = _view.value): SettingRow? =
        view?.rows?.firstOrNull { it.key == key } ?: view?.nativeRows?.firstOrNull { it.key == key }

    /** Whether a switch is on. */
    fun isOn(key: SettingKey, view: SettingsView? = _view.value): Boolean = SettingsCatalog.isOn(row(key, view))

    /** "Overridden for 3 shows", under a row some shows set for themselves. */
    fun overridden(key: SettingKey, view: SettingsView? = _view.value): String? {
        val shows = view?.overridden?.firstOrNull { it.key == key }?.shows?.toInt() ?: return null
        return "Overridden for ${Wording.plural(shows, "show")}"
    }
}

/** One value a setting can be set to, as `SetSetting` takes it, and the words its row then says. */
data class SettingOption(val value: SettingValue, val label: String)

/**
 * The values a stepped setting takes, in the order its ladder walks them, each with the words the
 * engine's row says once it is chosen: ports of `anistream_model::settings`'s `step` ladders and
 * `value` wording, so a menu offers exactly what the engine steps through.
 */
object SettingsCatalog {
    private val switches = setOf(
        SettingKey.MOTION, SettingKey.AUTO_NEXT, SettingKey.SKIP_OPENING, SettingKey.SKIP_FILLER,
        SettingKey.MARK_CHAPTERS, SettingKey.SKIP_BUTTON, SettingKey.FULLSCREEN, SettingKey.NIGHT_MODE, SettingKey.MERGE_SUBTITLES,
        SettingKey.KEEP_SEEDING, SettingKey.PRESENCE, SettingKey.PRESENCE_TITLE, SettingKey.PRESENCE_RICH, SettingKey.TORRENTS,
        SettingKey.SYNCPLAY, SettingKey.UPDATE_CHECK, SettingKey.SPOILER_PROTECTION,
        SettingKey.DOWNLOADS_ON_CELLULAR, SettingKey.TORRENTS_ON_CELLULAR, SettingKey.EPISODE_NOTIFICATIONS,
        SettingKey.ADULT_TITLES,
    )

    /** The settings with two states, on and off, that a switch shows. */
    fun isSwitch(key: SettingKey): Boolean = key in switches

    /** The values [key] takes, in its ladder's order, each as its row will say it. */
    fun options(key: SettingKey): List<SettingOption> = when (key) {
        SettingKey.TITLES -> listOf(
            SettingOption(SettingValue.Str("anilist"), "AniList's choice"),
            SettingOption(SettingValue.Str("english"), "English"),
            SettingOption(SettingValue.Str("romaji"), "Romaji"),
        )
        SettingKey.QUALITY -> listOf(480, 720, 1080, 2160).map { SettingOption(SettingValue.Int(it.toLong()), "${it}p") }
        SettingKey.COMMIT_THRESHOLD -> listOf(70, 80, 85, 90, 95).map { SettingOption(SettingValue.Float(it / 100.0), "$it%") }
        SettingKey.UPSCALING -> listOf("off" to "off", "natural" to "natural · ArtCNN", "sharp" to "sharp · Anime4K")
            .map { SettingOption(SettingValue.Str(it.first), it.second) }
        SettingKey.TRANSLATION -> listOf("sub", "dub").map { SettingOption(SettingValue.Str(it), it) }
        SettingKey.UP_NEXT_COUNTDOWN -> listOf(0, 5, 10, 15, 20, 30)
            .map { SettingOption(SettingValue.Int(it.toLong()), if (it == 0) "off" else "$it s") }
        SettingKey.SEED_RATIO -> listOf(0, 5, 10, 15, 20).map { tenths ->
            val ratio = tenths / 10.0
            SettingOption(SettingValue.Float(ratio), if (tenths > 0) String.format(Locale.ROOT, "until ratio %.1f", ratio) else "off")
        }
        SettingKey.SEED_HOURS -> listOf(1, 6, 12, 24, 72).map { SettingOption(SettingValue.Int(it.toLong()), "$it h") }
        SettingKey.UPLOAD_LIMIT -> listOf(0, 256, 512, 1024, 2048, 5120)
            .map { SettingOption(SettingValue.Int(it.toLong()), uploadLabel(it.toLong())) }
        SettingKey.DOWNLOAD_QUALITY -> listOf("standard", "higher").map { SettingOption(SettingValue.Str(it), it) }
        SettingKey.KEEP_NEXT_EPISODES -> listOf(0, 1, 2, 3, 5, 10)
            .map { SettingOption(SettingValue.Int(it.toLong()), if (it == 0) "off" else Wording.plural(it, "episode")) }
        // Warn only is asked for with its acknowledgement, through the torrent protection command.
        SettingKey.VPN_ON_FAILURE -> listOf("pause" to "pause torrents", "stop" to "stop torrents")
            .map { SettingOption(SettingValue.Str(it.first), it.second) }
        else -> if (isSwitch(key)) {
            listOf(SettingOption(SettingValue.Bool(false), "off"), SettingOption(SettingValue.Bool(true), "on"))
        } else {
            emptyList()
        }
    }

    /** An upload cap as people read it: `unlimited`, `524 kB/s`. `upload_label`. */
    fun uploadLabel(kib: Long): String = if (kib == 0L) "unlimited" else "${Wording.bytes(kib * 1024)}/s"

    /**
     * The option a row stands on, found by its words. Sub or dub says the saved choice first, with
     * the session's beside it when that differs, and stands on the saved one.
     */
    fun current(row: SettingRow?): SettingOption? {
        row ?: return null
        val saved = if (row.key == SettingKey.TRANSLATION || row.key == SettingKey.THEME) row.value.substringBefore(" · ") else row.value
        return options(row.key).firstOrNull { it.label == saved }
    }

    /** Whether a switch's row says it is on. */
    fun isOn(row: SettingRow?): Boolean = row?.value == "on"

    /** Whether a setting's words say it is on: "on", or a sheet's "Use Default · On". */
    fun isOnWords(value: String): Boolean = value.substringAfterLast("· ").equals("on", ignoreCase = true)

    /** Whether the row can be changed from here at all: `SettingChange::editable`. */
    fun editable(row: SettingRow?): Boolean = row != null && row.change !is SettingChange.Fixed

    /** The text a typed setting starts editing from, when it is typed. */
    fun typedText(row: SettingRow?): String? = (row?.change as? SettingChange.Typed)?.text
}
