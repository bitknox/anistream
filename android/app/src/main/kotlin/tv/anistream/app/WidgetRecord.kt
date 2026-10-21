// What the home-screen widgets show: the engine's record of Continue and today's broadcasts
// (`WidgetSnapshot`), kept in the app's own files for the widgets to read.

package tv.anistream.app

import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import tv.anistream.engine.model.WidgetAiring
import tv.anistream.engine.model.WidgetContinue
import tv.anistream.engine.model.WidgetData

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
        /** The shape of the record this build reads: the engine's `WIDGET_DATA_VERSION`. */
        const val READS = 1u

        /** Where the app keeps it. */
        fun of(files: File): WidgetRecord = WidgetRecord(files.resolve("widgets/record.json"))

        fun encode(data: WidgetData): JSONObject = JSONObject()
            .put("version", data.version.toLong())
            .put("generated_at", data.generatedAt)
            .put("continue_watching", JSONArray(data.continueWatching.map(::encode)))
            .put("airing_today", JSONArray(data.airingToday.map(::encode)))

        private fun encode(row: WidgetContinue): JSONObject = JSONObject()
            .put("id", row.id.toLong())
            .put("title", row.title)
            .put("episode", row.episode.toLong())
            .put("watched", row.watched.toLong())
            .putOpt("episodes", row.episodes?.toLong())
            .putOpt("fraction", row.fraction)
            .putOpt("cover_url", row.coverUrl)

        private fun encode(row: WidgetAiring): JSONObject = JSONObject()
            .put("id", row.id.toLong())
            .put("title", row.title)
            .put("episode", row.episode.toLong())
            .put("airs_at", row.airsAt)
            .put("on_list", row.onList)
            .putOpt("cover_url", row.coverUrl)

        fun decode(json: JSONObject): WidgetData? {
            val version = json.getLong("version").toUInt()
            if (version != READS) return null
            return WidgetData(
                version = version,
                generatedAt = json.getLong("generated_at"),
                continueWatching = json.optJSONArray("continue_watching").objects().map { row ->
                    WidgetContinue(
                        id = row.getLong("id").toUInt(),
                        title = row.getString("title"),
                        episode = row.getLong("episode").toUInt(),
                        watched = row.optLong("watched").toUInt(),
                        episodes = row.optLongOrNull("episodes")?.toUInt(),
                        fraction = if (row.isNull("fraction")) null else row.getDouble("fraction"),
                        coverUrl = row.optStringOrNull("cover_url"),
                    )
                },
                airingToday = json.optJSONArray("airing_today").objects().map { row ->
                    WidgetAiring(
                        id = row.getLong("id").toUInt(),
                        title = row.getString("title"),
                        episode = row.getLong("episode").toUInt(),
                        airsAt = row.getLong("airs_at"),
                        onList = row.optBoolean("on_list"),
                        coverUrl = row.optStringOrNull("cover_url"),
                    )
                },
            )
        }

        private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).map(::getJSONObject)

        private fun JSONObject.optLongOrNull(name: String): Long? = if (isNull(name)) null else getLong(name)

        private fun JSONObject.optStringOrNull(name: String): String? = if (isNull(name)) null else getString(name)
    }
}
