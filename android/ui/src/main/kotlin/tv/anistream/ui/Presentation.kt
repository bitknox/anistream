// How a title's fields read on screen.

package tv.anistream.ui

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import android.content.Context
import tv.anistream.store.AppClock
import tv.anistream.store.CalendarModel
import tv.anistream.store.ContinueItem
import tv.anistream.store.EpisodeAiring
import tv.anistream.store.airing
import tv.anistream.store.clock
import tv.anistream.store.playableNext
import tv.anistream.store.watchedAll
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.engine.model.RelatedTitle
import tv.anistream.engine.model.ResumePoint
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.percentText
import androidx.compose.ui.platform.LocalContext

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
 * The metadata line on a title page: "TV · 24 episodes · Summer 2026 · 79%". The season with its
 * year when AniList gives it, the year alone otherwise.
 */
fun Entry.metadataLine(context: Context): String {
    val parts = mutableListOf<String>()
    formatLabel(context, format)?.let { parts += it }
    episodes?.takeIf { it > 0u && format != "MOVIE" }?.let {
        parts += context.resources.getQuantityString(R.plurals.episodes_count, it.toInt(), it.toInt())
    }
    val airedIn = when {
        season != null && year != null -> Strings.seasonWithYear(context, seasonName(context, season!!), year.toString())
        season != null -> seasonName(context, season!!)
        else -> year?.toString()
    }
    airedIn?.let { parts += it }
    score?.let { parts += percentText(it.toInt()) }
    return parts.joinToString(" · ") // i18n-skip
}

/** A season's name: "Autumn". */
fun seasonName(context: Context, season: tv.anistream.engine.model.Season): String = when (season) {
    tv.anistream.engine.model.Season.WINTER -> Strings.seasonWinter(context)
    tv.anistream.engine.model.Season.SPRING -> Strings.seasonSpring(context)
    tv.anistream.engine.model.Season.SUMMER -> Strings.seasonSummer(context)
    tv.anistream.engine.model.Season.FALL -> Strings.seasonAutumn(context)
}

/** The episode a title's action plays, and how it says so. */
data class TitleAction(
    val episode: UInt,
    val resume: ResumePoint?,
    val airing: EpisodeAiring,
    val airsAt: Long? = null,
    val translation: Translation? = null,
    /** Every episode from the next on is one no source lists: nothing to play. */
    val unsourced: Boolean = false,
    /** Every episode is watched: the action plays episode 1 again. */
    val watchAgain: Boolean = false,
) {
    /**
     * Whether there is an episode to play: never one still to air, which nothing could find, and
     * never one no source lists.
     */
    val playable: Boolean get() = airing.hasAired && !unsourced

    /** "Resume episode 7 · 10:06", "Play episode 8", and for a dub "Play episode 8 · Dubbed": what the amber action says. */
    fun label(context: Context): String {
        val dubbed = translation == Translation.DUB
        return when {
            watchAgain && resume == null -> Strings.titleButtonWatchAgain(context)
            resume != null ->
                if (dubbed) Strings.titleButtonResumeDubbed(context, episode.toInt(), resume.clock())
                else Strings.titleButtonResume(context, episode.toInt(), resume.clock())
            else -> if (dubbed) Strings.titleButtonPlayDubbed(context, episode.toInt()) else Strings.titleButtonPlay(context, episode.toInt())
        }
    }

    /**
     * Caught up, the next episode still to air: "Episode 10 airs Thursday 16:30", a haze line
     * with a water dot in the action's place, or "Episode 10 is not out yet" undated.
     */
    fun caughtUp(context: Context): String =
        if (unsourced) Strings.episodeUnsourced(context)
        else airsAt?.let { Strings.episodeAirs(context, episode.toInt(), airsWords(context, Instant.ofEpochSecond(it))) }
            ?: airing.airs()?.let { Strings.episodeAirs(context, episode.toInt(), it) }
            ?: Strings.episodeNotOut(context, episode.toInt())

    companion object {
        /**
         * The action for [entry]: the engine's next episode, or in [rows], once the table is loaded,
         * the first after it that some source lists.
         */
        fun of(entry: Entry, now: Long = AppClock.now().epochSecond, rows: List<EpisodeRow>? = null): TitleAction {
            val target = entry.playableNext(rows)
            val episode = target.episode
            val resume = entry.resume
            val dub = entry.watchedIn
            if (resume != null) return TitleAction(episode, resume, EpisodeAiring.Aired, translation = dub)
            if (target.unsourced) return TitleAction(episode, null, entry.airing(episode), translation = dub, unsourced = true)
            if (entry.watchedAll) return TitleAction(episode, null, EpisodeAiring.Aired, translation = dub, watchAgain = true)
            // The schedule says when the very episode airs, where AniList has one.
            val scheduled = entry.detail?.schedule?.firstOrNull { it.episode == episode }
            if (scheduled != null) {
                return if (scheduled.airsAt > now) {
                    TitleAction(episode, null, EpisodeAiring.Upcoming(scheduled.airsAt - now), scheduled.airsAt, dub)
                } else {
                    TitleAction(episode, null, EpisodeAiring.Aired, translation = dub)
                }
            }
            return TitleAction(episode, null, entry.airing(episode), translation = dub)
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

/** The folded episodes' row: "Episodes 1 to 5 watched". */
fun watchedRangeWords(context: Context, folded: List<EpisodeRow>): String =
    Strings.titleWatchedRange(context, folded.first().number.toInt(), folded.last().number.toInt())

/**
 * A title's seasons, and what else it has: films, OVAs and specials.
 *
 * The seasons are the franchise's, as the engine walks them along the prequel and sequel chain:
 * the same list, numbered the same, on every one of their pages. Until the engine has walked them
 * there are none, rather than seasons numbered from what this page alone knows, which would
 * renumber as the rest arrived. The related cards leave this title's series prequels and sequels
 * to the picker either way.
 */
data class TitleFamily(val seasons: List<Season>, val related: List<RelatedTitle>) {
    /** A season, with the card the engine sent for it: what a page shows of it before its own title has come. */
    data class Season(val id: AnilistId, val title: String, val isThis: Boolean, val card: LinkedTitle? = null)

    /** The card the engine sent for season [id], if it is one of these. */
    fun card(id: AnilistId): LinkedTitle? = seasons.firstOrNull { it.id == id }?.card

    /** The seasons either side of [id] in the picker, the one before first: one or none at the ends. */
    fun neighbours(id: AnilistId): List<AnilistId> {
        val at = seasons.indexOfFirst { it.id == id }
        if (at < 0) return emptyList()
        return listOfNotNull(seasons.getOrNull(at - 1)?.id, seasons.getOrNull(at + 1)?.id)
    }

    companion object {
        private val series = setOf("TV", "TV_SHORT", "TV SHORT", "ONA")

        /**
         * [entry]'s family. Its seasons are its own once the engine has walked them; before that, or
         * for a season whose page has not come yet, the [known] seasons of another of the
         * franchise's pages, when [entry] is among them, since every season has the same ones.
         */
        fun of(entry: Entry, known: List<LinkedTitle>? = null): TitleFamily {
            val own = entry.detail?.seasons
            val seasons = own ?: known?.takeIf { cards -> cards.any { it.id == entry.id } }.orEmpty()
            return TitleFamily(
                seasons = seasons.map { Season(it.id, it.title, it.id == entry.id, it) },
                related = entry.related.filter { it.format !in series },
            )
        }
    }
}

/** "TV", "Movie", "OVA": a format as a label says it. */
fun formatLabel(context: Context, format: String?): String? = when (format) {
    null -> null
    "TV" -> Strings.formatTv(context)
    "MOVIE" -> Strings.formatMovie(context)
    "TV_SHORT", "TV SHORT" -> Strings.formatTvShort(context)
    "SPECIAL" -> Strings.formatSpecial(context)
    "MUSIC" -> Strings.formatMusic(context)
    "OVA" -> Strings.formatOva(context)
    "ONA" -> Strings.formatOna(context)
    else -> format.replace('_', ' ')
}

/** What a Continue row says it plays; with no source for what is left, that no source has it. */
@androidx.compose.runtime.Composable
internal fun ContinueItem.actionText(): String {
    val context = androidx.compose.ui.platform.LocalContext.current
    return if (unsourced) Strings.episodeUnsourced(context) else action(context)
}
