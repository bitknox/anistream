// What the home-screen widgets show: the engine's record of Continue and the coming broadcasts
// (`WidgetSnapshot`), kept in the app's own files for the widgets to read.

package tv.anistream.app

import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.WidgetAiring
import tv.anistream.engine.model.WidgetContinue
import tv.anistream.engine.model.WidgetData
import tv.anistream.engine.model.WidgetNextAiring

/**
 * The widgets' repository. Android's widgets run in the app's process, so there is no file shared
 * with another one: the record is kept in [file], under the app's files, and read back from it in
 * a process that starts for a widget alone. Written whole and renamed into place, so a reader
 * never sees half of one; a record of a shape this build does not read reads as none.
 */
class WidgetRecord(private val file: File) {
    private val _latest = MutableStateFlow(read())

    /** The last record written, or read at start. */
    val latest: StateFlow<WidgetData?> = _latest.asStateFlow()

    /** Keep [data] as the widgets' record. */
    @Synchronized
    fun write(data: WidgetData) {
        file.parentFile?.mkdirs()
        val next = File(file.path + ".next")
        next.writeText(encode(data).toString())
        if (!next.renameTo(file)) {
            next.delete()
            return
        }
        _latest.value = data
    }

    /** The record in [file], or `null` when there is none this build can read. */
    fun read(): WidgetData? = runCatching { file.takeIf(File::isFile)?.readText()?.let { decode(JSONObject(it)) } }.getOrNull()

    companion object {
        /** The shape of the record this build reads and writes: the engine's `WIDGET_DATA_VERSION`. */
        const val READS = 2u

        /** Where the app keeps it. */
        fun of(files: File): WidgetRecord = WidgetRecord(files.resolve("widgets/record.json"))

        fun encode(data: WidgetData): JSONObject = JSONObject()
            .put("version", data.version.toLong())
            .put("generated_at", data.generatedAt)
            .put("signed_in", data.signedIn)
            .putOpt("tracker", data.tracker)
            .put("continue_watching", JSONArray(data.continueWatching.map(::encode)))
            .put("airing_today", JSONArray(data.airingToday.map(::encode)))
            .put("next_days", JSONArray(data.nextDays.map(::encode)))
            .putOpt("next_airing", data.nextAiring?.let(::encode))

        private fun encode(row: WidgetContinue): JSONObject = JSONObject()
            .put("id", row.id.toLong())
            .put("title", row.title)
            .put("episode", row.episode.toLong())
            .put("watched", row.watched.toLong())
            .putOpt("episodes", row.episodes?.toLong())
            .putOpt("fraction", row.fraction)
            .putOpt("cover_url", row.coverUrl)
            .putOpt("episode_title", row.episodeTitle)
            .put("resume", row.resume)
            .putOpt("position_secs", row.positionSecs?.toLong())
            .putOpt("duration_secs", row.durationSecs?.toLong())
            .putOpt("minutes_left", row.minutesLeft?.toLong())
            .putOpt("next_airing", row.nextAiring)
            .putOpt("next_airing_episode", row.nextAiringEpisode?.toLong())
            .putOpt("banner_url", row.bannerUrl)
            .putOpt("cover_color", row.coverColor)

        private fun encode(row: WidgetAiring): JSONObject = JSONObject()
            .put("id", row.id.toLong())
            .put("title", row.title)
            .put("episode", row.episode.toLong())
            .put("airs_at", row.airsAt)
            .put("on_list", row.onList)
            .putOpt("cover_url", row.coverUrl)
            .put("watched", row.watched)
            .putOpt("translation", row.translation?.name?.lowercase())
            .putOpt("banner_url", row.bannerUrl)
            .putOpt("cover_color", row.coverColor)

        private fun encode(next: WidgetNextAiring): JSONObject = JSONObject()
            .put("id", next.id.toLong())
            .put("title", next.title)
            .put("episode", next.episode.toLong())
            .put("airs_at", next.airsAt)

        fun decode(json: JSONObject): WidgetData? {
            val version = json.getLong("version").toUInt()
            // The first shape reads too, from a build before the second: what it lacks takes its
            // default, and the version stays, for the widgets to know what it could not say.
            if (version !in 1u..READS) return null
            return WidgetData(
                version = version,
                generatedAt = json.getLong("generated_at"),
                continueWatching = json.optJSONArray("continue_watching").objects().map(::continueOf),
                airingToday = json.optJSONArray("airing_today").objects().map(::airingOf),
                signedIn = json.optBoolean("signed_in"),
                tracker = json.string("tracker"),
                nextAiring = json.optJSONObject("next_airing")?.let { next ->
                    WidgetNextAiring(next.getLong("id").toUInt(), next.getString("title"), next.getLong("episode").toUInt(), next.getLong("airs_at"))
                },
                nextDays = json.optJSONArray("next_days").objects().map(::airingOf),
            )
        }

        private fun continueOf(row: JSONObject) = WidgetContinue(
            id = row.getLong("id").toUInt(),
            title = row.getString("title"),
            episode = row.getLong("episode").toUInt(),
            watched = row.optLong("watched").toUInt(),
            episodes = row.long("episodes")?.toUInt(),
            fraction = if (row.isNull("fraction")) null else row.getDouble("fraction"),
            coverUrl = row.string("cover_url"),
            episodeTitle = row.string("episode_title"),
            resume = row.optBoolean("resume"),
            positionSecs = row.long("position_secs")?.toUInt(),
            durationSecs = row.long("duration_secs")?.toUInt(),
            minutesLeft = row.long("minutes_left")?.toUInt(),
            nextAiring = row.long("next_airing"),
            nextAiringEpisode = row.long("next_airing_episode")?.toUInt(),
            bannerUrl = row.string("banner_url"),
            coverColor = row.string("cover_color"),
        )

        private fun airingOf(row: JSONObject) = WidgetAiring(
            id = row.getLong("id").toUInt(),
            title = row.getString("title"),
            episode = row.getLong("episode").toUInt(),
            airsAt = row.getLong("airs_at"),
            onList = row.optBoolean("on_list"),
            coverUrl = row.string("cover_url"),
            watched = row.optBoolean("watched"),
            translation = row.string("translation")?.let { name -> Translation.entries.firstOrNull { it.name.equals(name, ignoreCase = true) } },
            bannerUrl = row.string("banner_url"),
            coverColor = row.string("cover_color"),
        )

        private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).map(::getJSONObject)

        private fun JSONObject.long(name: String): Long? = if (isNull(name)) null else getLong(name)

        private fun JSONObject.string(name: String): String? = if (isNull(name)) null else getString(name)
    }
}
