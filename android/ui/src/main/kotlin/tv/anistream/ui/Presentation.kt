// How a title's fields read on screen.

package tv.anistream.ui

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale
import tv.anistream.store.CalendarModel
import tv.anistream.store.EpisodeAiring
import tv.anistream.store.airing
import tv.anistream.store.clock
import uniffi.anistream_core.AnilistId
import uniffi.anistream_model.Entry
import uniffi.anistream_model.RelatedTitle
import uniffi.anistream_model.ResumePoint

/**
 * The title as the screens head it: the romaji when AniList has an English title beside it, as the
 * boards set it ("Hoshikuzu no Tabibito"), and the one title there is otherwise.
 */
val Entry.showTitle: String get() = secondary ?: title

/** The first genre, in sentence case as the boards write it: "Slice of life". */
val Entry.genreLine: String?
    get() = genres.firstOrNull()?.lowercase()?.replaceFirstChar { it.uppercase() }

/** One broadcast in the calendar: a title, the episode, and when it airs. */
data class Broadcast(val entry: Entry, val episode: UInt, val airs: Instant)

/** The calendar's broadcasts, each dated against the window's anchor. */
fun CalendarModel.Window.broadcasts(now: Instant = Instant.now()): List<Broadcast> {
    val anchor = anchor?.let(Instant::ofEpochSecond) ?: now
    return rows.mapNotNull { row ->
        val airingIn = row.airingIn ?: return@mapNotNull null
        val episode = row.airingEpisode ?: return@mapNotNull null
        Broadcast(row, episode, anchor.plusSeconds(airingIn))
    }
}

/** The broadcasts that air on [now]'s date where the phone is. */
fun List<Broadcast>.today(now: Instant = Instant.now(), zone: ZoneId = ZoneId.systemDefault()): List<Broadcast> {
    val today = LocalDate.ofInstant(now, zone)
    return filter { LocalDate.ofInstant(it.airs, zone) == today }
}

/** The other title, beneath the first: the English one ("Stardust Wayfarer"). */
val Entry.alternateTitle: String? get() = if (secondary == null) null else title

/**
 * The metadata line on a title page, set in capitals by its style: "TV · 24 EP · SUMMER 2026 ·
 * 79%". The season with its year when AniList gives it, the year alone otherwise.
 */
val Entry.metadataLine: String
    get() {
        val parts = mutableListOf<String>()
        format?.let { parts += it.replace('_', ' ') }
        episodes?.takeIf { it > 0u && format != "MOVIE" }?.let { parts += "$it EP" }
        val airedIn = listOfNotNull(season?.name, year?.toString()).joinToString(" ")
        if (airedIn.isNotEmpty()) parts += airedIn
        score?.let { parts += "$it%" }
        return parts.joinToString(" · ").uppercase(Locale.ROOT)
    }

/** The episode a title's action plays, and how it says so. */
data class TitleAction(val episode: UInt, val resume: ResumePoint?, val airing: EpisodeAiring) {
    /** Whether there is an episode to play: never one still to air, which nothing could find. */
    val playable: Boolean get() = airing.hasAired

    /**
     * "Resume E7 · 10:06", "Start E8", or with the episode still to air, the status in the
     * button's place: "Airs Thursday 16:30", or "E8 is not out yet" when the schedule has no date.
     */
    val label: String
        get() = when {
            resume != null -> "Resume E$episode · ${resume.clock()}"
            !playable -> airing.airs()?.let { "Airs $it" } ?: "E$episode is not out yet"
            else -> "Start E$episode"
        }

    companion object {
        fun of(entry: Entry): TitleAction {
            val episode = entry.progress?.next ?: 1u
            val resume = entry.resume
            return TitleAction(episode, resume, if (resume == null) entry.airing(episode) else EpisodeAiring.Aired)
        }
    }
}

/**
 * A title's seasons, from AniList's relations in watch order, and what else it has: films, OVAs
 * and specials.
 */
data class TitleFamily(val seasons: List<Season>, val related: List<RelatedTitle>) {
    data class Season(val id: AnilistId, val title: String, val isThis: Boolean)

    companion object {
        private val series = setOf("TV", "TV_SHORT", "TV SHORT", "ONA")

        fun of(entry: Entry): TitleFamily {
            val before = entry.related.filter { it.relation == "prequel" && it.format in series }
            val after = entry.related.filter { it.relation == "sequel" && it.format in series }
            val all = before.map { Season(it.id, it.title, false) } +
                Season(entry.id, entry.title, true) +
                after.map { Season(it.id, it.title, false) }
            return TitleFamily(
                seasons = if (all.size > 1) all else emptyList(),
                related = entry.related.filter { it.format !in series },
            )
        }
    }
}

/** "TV", "Movie", "OVA": a format as a label says it. */
fun formatLabel(format: String?): String? = when (format) {
    null -> null
    "MOVIE" -> "Movie"
    "TV_SHORT" -> "TV short"
    "SPECIAL" -> "Special"
    "MUSIC" -> "Music"
    else -> format
}
