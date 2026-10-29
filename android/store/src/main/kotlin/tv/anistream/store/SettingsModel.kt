// The config as the engine shows it, and the choices each stepped setting takes.

package tv.anistream.store

import java.util.Locale
import android.content.Context
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
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
    fun overridden(context: Context, key: SettingKey, view: SettingsView? = _view.value): String? {
        val shows = view?.overridden?.firstOrNull { it.key == key }?.shows?.toInt() ?: return null
        return context.resources.getQuantityString(R.plurals.settings_overridden, shows, shows)
    }
}

/**
 * One value a setting can be set to, as `SetSetting` takes it, and the words the engine's row says
 * once it is chosen. The label is the engine's own token, which a row is matched by; the person
 * reads [SettingsCatalog.words].
 */
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
        SettingKey.TRAILERS -> listOf("always" to "always", "wifi-only" to "Wi-Fi only", "off" to "off")
            .map { SettingOption(SettingValue.Str(it.first), it.second) }
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
            .map { SettingOption(SettingValue.Int(it.toLong()), if (it == 0) "off" else if (it == 1) "1 episode" else "$it episodes") }
        // Warn only is asked for with its acknowledgement, through the torrent protection command.
        SettingKey.VPN_ON_FAILURE -> listOf("pause" to "pause torrents", "stop" to "stop torrents")
            .map { SettingOption(SettingValue.Str(it.first), it.second) }
        else -> if (isSwitch(key)) {
            listOf(SettingOption(SettingValue.Bool(false), "off"), SettingOption(SettingValue.Bool(true), "on"))
        } else {
            emptyList()
        }
    }

    /** An upload cap as the engine says it: `unlimited`, `524 kB/s`. `upload_label`. */
    fun uploadLabel(kib: Long): String = if (kib == 0L) "unlimited" else "${engineBytes(kib * 1024)}/s"

    /** The engine's size words, which a row is matched by: the same rule in English whatever the language. */
    private fun engineBytes(bytes: Long): String {
        for ((unit, scale) in listOf("GB" to 1_000_000_000L, "MB" to 1_000_000L, "kB" to 1_000L, "B" to 1L)) {
            if (bytes >= scale) {
                val value = bytes.toDouble() / scale
                return if (value < 10 && scale > 1) String.format(Locale.ROOT, "%.1f %s", value, unit) else "${Math.round(value)} $unit"
            }
        }
        return "0 B"
    }

    /** An option as the person reads it, in their language: "Subbed", "Off", "Until ratio 1.5". */
    fun words(context: Context, key: SettingKey, option: SettingOption): String {
        val value = option.value
        return when {
            key == SettingKey.TITLES && value is SettingValue.Str -> when (value.v1) {
                "anilist" -> Strings.settingsTitleTracker(context, "AniList")
                "english" -> Strings.settingsTitleEnglish(context)
                else -> Strings.settingsTitleRomaji(context)
            }
            key == SettingKey.TRAILERS && value is SettingValue.Str -> when (value.v1) {
                "wifi-only" -> Strings.settingsTrailersWifiOnly(context)
                "off" -> Strings.valueOff(context)
                else -> Strings.settingsTrailersAlways(context)
            }
            key == SettingKey.COMMIT_THRESHOLD && value is SettingValue.Float -> percentText(Math.round(value.v1 * 100).toInt())
            key == SettingKey.UPSCALING && value is SettingValue.Str -> when (value.v1) {
                "natural" -> Strings.settingsUpscalingNatural(context)
                "sharp" -> Strings.settingsUpscalingSharp(context)
                else -> Strings.valueOff(context)
            }
            key == SettingKey.TRANSLATION && value is SettingValue.Str -> if (value.v1 == "dub") Strings.audioDubbed(context) else Strings.audioSubbed(context)
            key == SettingKey.UP_NEXT_COUNTDOWN && value is SettingValue.Int ->
                if (value.v1 == 0L) Strings.valueOff(context) else Strings.durationSeconds(context, value.v1.toInt())
            key == SettingKey.SEED_RATIO && value is SettingValue.Float ->
                if (value.v1 > 0) Strings.settingsSeedRatioUntil(context, java.text.NumberFormat.getInstance().apply { minimumFractionDigits = 1; maximumFractionDigits = 1 }.format(value.v1)) else Strings.valueOff(context)
            key == SettingKey.SEED_HOURS && value is SettingValue.Int -> Strings.durationHours(context, value.v1.toInt())
            key == SettingKey.UPLOAD_LIMIT && value is SettingValue.Int ->
                if (value.v1 == 0L) Strings.settingsUploadUnlimited(context) else Strings.settingsUploadRate(context, Wording.bytes(context, value.v1 * 1024))
            key == SettingKey.DOWNLOAD_QUALITY && value is SettingValue.Str -> if (value.v1 == "higher") Strings.settingsQualityHigher(context) else Strings.settingsQualityStandard(context)
            key == SettingKey.KEEP_NEXT_EPISODES && value is SettingValue.Int ->
                if (value.v1 == 0L) Strings.valueOff(context) else context.resources.getQuantityString(R.plurals.episodes_count, value.v1.toInt(), value.v1.toInt())
            key == SettingKey.VPN_ON_FAILURE && value is SettingValue.Str -> if (value.v1 == "stop") Strings.torrentsFailStop(context) else Strings.torrentsFailPause(context)
            value is SettingValue.Bool -> if (value.v1) Strings.valueOn(context) else Strings.valueOff(context)
            else -> option.label.replaceFirstChar { it.titlecase(AirTime.locale) }
        }
    }

    /** The words for a row's value as the engine said it: the option's, else the engine's own in sentence case. */
    fun words(context: Context, row: SettingRow?): String? {
        row ?: return null
        val option = current(row) ?: return row.value.replaceFirstChar { it.titlecase(AirTime.locale) }
        return words(context, row.key, option)
    }

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
