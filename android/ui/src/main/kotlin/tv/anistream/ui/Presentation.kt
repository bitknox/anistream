// How a title's fields read on screen.

package tv.anistream.ui

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale
import tv.anistream.store.AppClock
import tv.anistream.store.CalendarModel
import tv.anistream.store.EpisodeAiring
import tv.anistream.store.airing
import tv.anistream.store.clock
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.RelatedTitle
import tv.anistream.engine.model.ResumePoint

/**
 * The title as the screens head it: the form the engine names it by, following the title-language
 * setting, which the app shows as it gets it.
 */
val Entry.showTitle: String get() = title

/** The first genre, in sentence case as the boards write it: "Slice of life". */
val Entry.genreLine: String?
    get() = genres.firstOrNull()?.lowercase()?.replaceFirstChar { it.uppercase() }

/** One broadcast in the calendar: a title, the episode, and when it airs. */
data class Broadcast(val entry: Entry, val episode: UInt, val airs: Instant)

/** The calendar's broadcasts, each dated against the window's anchor. */
fun CalendarModel.Window.broadcasts(now: Instant = AppClock.now()): List<Broadcast> {
    val anchor = anchor?.let(Instant::ofEpochSecond) ?: now
    return rows.mapNotNull { row ->
        val airingIn = row.airingIn ?: return@mapNotNull null
        val episode = row.airingEpisode ?: return@mapNotNull null
        Broadcast(row, episode, anchor.plusSeconds(airingIn))
    }
}

/** The broadcasts that air on [now]'s date where the phone is. */
fun List<Broadcast>.today(now: Instant = AppClock.now(), zone: ZoneId = AppClock.zone): List<Broadcast> {
    val today = LocalDate.ofInstant(now, zone)
    return filter { LocalDate.ofInstant(it.airs, zone) == today }
}

/** The other forms beneath the title, joined: "Stardust Wayfarer · 星屑の旅人". */
val Entry.alternateTitle: String? get() = otherForms().joinToString(" \u00b7 ").ifEmpty { null }

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
data class TitleAction(val episode: UInt, val resume: ResumePoint?, val airing: EpisodeAiring, val airsAt: Long? = null) {
    /** Whether there is an episode to play: never one still to air, which nothing could find. */
    val playable: Boolean get() = airing.hasAired

    /** "Resume E7 · 10:06", "Start E8": what the amber action says. */
    val label: String
        get() = when {
            resume != null -> "Resume E$episode · ${resume.clock()}"
            else -> "Start E$episode"
        }

    /**
     * Caught up, the next episode still to air: "Episode 10 airs Thursday 16:30", a haze line
     * with a water dot in the action's place, or "Episode 10 is not out yet" undated.
     */
    fun caughtUp(context: android.content.Context): String =
        airsAt?.let { "Episode $episode airs ${airsWords(context, Instant.ofEpochSecond(it))}" }
            ?: airing.airs()?.let { "Episode $episode airs $it" }
            ?: "Episode $episode is not out yet"

    companion object {
        fun of(entry: Entry, now: Long = AppClock.now().epochSecond): TitleAction {
            val episode = entry.progress?.next ?: 1u
            val resume = entry.resume
            if (resume != null) return TitleAction(episode, resume, EpisodeAiring.Aired)
            // The schedule says when the very episode airs, where AniList has one.
            val scheduled = entry.detail?.schedule?.firstOrNull { it.episode == episode }
            if (scheduled != null) {
                return if (scheduled.airsAt > now) {
                    TitleAction(episode, null, EpisodeAiring.Upcoming(scheduled.airsAt - now), scheduled.airsAt)
                } else {
                    TitleAction(episode, null, EpisodeAiring.Aired)
                }
            }
            return TitleAction(episode, null, entry.airing(episode))
        }
    }
}

/**
 * The episode rows a page lists: every row that has aired (or has no date, which counts as
 * aired), and of those still to air only the next, built from the schedule when the source has no
 * row for it.
 */
fun Entry.listedEpisodes(rows: List<EpisodeRow>, now: Long = AppClock.now().epochSecond): List<EpisodeRow> {
    val aired = rows.filter { (it.airsAt ?: 0) <= now }
    val next = nextUnaired(rows, now) ?: return aired
    if (aired.any { it.number == next.number.toString() }) return aired
    val row = rows.firstOrNull { it.number == next.number.toString() }?.copy(airsAt = next.airsAt)
        ?: EpisodeRow(
            number = next.number.toString(), title = null, thumbnail = null, description = null,
            durationSecs = null, watched = 0.0, completed = false, kind = null, skippable = false,
            airsAt = next.airsAt,
        )
    return aired + row
}

/**
 * The watched episodes a title page folds away, so its list starts at the last one watched: the
 * episodes before it, while every one of them is watched. In [columns] of two, an even number,
 * so the last watched keeps its place in its pair. None when fewer than two would fold.
 */
fun Entry.foldedEpisodes(rows: List<EpisodeRow>, columns: Int = 1): List<EpisodeRow> {
    val next = progress?.next ?: return emptyList()
    if (next <= 1u) return emptyList()
    val last = rows.indexOfFirst { it.number == (next - 1u).toString() }.takeIf { it >= 0 }
        ?: rows.indexOfLast { it.completed }
    if (last <= 0) return emptyList()
    var folded = rows.take(last).takeWhile { it.completed }.size
    if (columns == 2 && folded % 2 == 1) folded -= 1
    return if (folded < 2) emptyList() else rows.take(folded)
}

/** The folded episodes' row: "Episodes 1–5 watched". */
fun watchedRangeWords(folded: List<EpisodeRow>): String =
    "Episodes ${folded.first().number}\u2013${folded.last().number} watched"

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
