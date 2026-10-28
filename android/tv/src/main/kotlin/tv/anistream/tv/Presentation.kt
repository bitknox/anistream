// How a title's fields read on the TV: what the amber action plays and says, the hero's line and
// the title page's metadata line. The phone has its own in :ui; these follow the TV brief's words.

package tv.anistream.tv

import tv.anistream.i18n.R
import tv.anistream.store.percentText
import tv.anistream.i18n.Strings
import android.content.Context
import java.time.Instant
import java.time.format.TextStyle
import java.util.Locale
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.ResumePoint
import tv.anistream.store.AirTime
import tv.anistream.store.AppClock
import tv.anistream.store.ContinueItem
import tv.anistream.store.EpisodeAiring
import tv.anistream.store.airing
import tv.anistream.store.playableNext

/** The episode a title's amber action plays, and how it says so. */
data class PlayAction(
    val episode: UInt,
    val resume: ResumePoint?,
    val airing: EpisodeAiring,
    val airsAt: Long? = null,
    val translation: Translation? = null,
    /** Every episode from the next on is one no source lists: nothing to play. */
    val unsourced: Boolean = false,
) {
    /** Whether it plays now: never an episode still to air, nor one no source lists. */
    val playable: Boolean get() = airing.hasAired && !unsourced

    /**
     * "Resume episode 7", "Play episode 1", or for one not out yet "Play episode 10 (out Thursday)",
     * which stays disabled until it is out.
     */
    fun label(context: Context, now: Instant = AppClock.now()): String {
        val dubbed = translation == Translation.DUB
        val number = episode.toInt()
        return when {
            resume != null -> if (dubbed) Strings.tvActionResumeDubbed(context, number) else Strings.tvActionResume(context, number)
            playable -> if (dubbed) Strings.titleButtonPlayDubbed(context, number) else Strings.titleButtonPlay(context, number)
            else -> outDay(context, now)?.let { Strings.tvActionOut(context, number, it) } ?: Strings.tvActionNotOut(context, number)
        }
    }

    private fun outDay(context: Context, now: Instant): String? {
        val at = airsAt?.let(Instant::ofEpochSecond)
            ?: (airing as? EpisodeAiring.Upcoming)?.inSeconds?.let(now::plusSeconds)
            ?: return null
        val zone = AppClock.zone
        val days = (at.epochSecond - now.epochSecond) / 86_400.0
        val local = at.atZone(zone)
        return when {
            local.toLocalDate() == now.atZone(zone).toLocalDate() -> Strings.tvOutToday(context)
            days < 6 -> local.dayOfWeek.getDisplayName(TextStyle.FULL, AirTime.locale)
            else -> java.time.format.DateTimeFormatter.ofPattern(android.text.format.DateFormat.getBestDateTimePattern(AirTime.locale, "dMMMM"), AirTime.locale).format(local)
        }
    }

    companion object {
        /**
         * The action for [entry]: the engine's next episode, or in [rows], once the table is
         * loaded, the first after it that some source lists.
         */
        fun of(entry: Entry, now: Long = AppClock.now().epochSecond, rows: List<EpisodeRow>? = null): PlayAction {
            val target = entry.playableNext(rows)
            val episode = target.episode
            val resume = entry.resume
            val dub = entry.watchedIn
            if (resume != null) return PlayAction(episode, resume, EpisodeAiring.Aired, translation = dub)
            if (target.unsourced) return PlayAction(episode, null, entry.airing(episode), translation = dub, unsourced = true)
            val scheduled = entry.detail?.schedule?.firstOrNull { it.episode == episode }
            if (scheduled != null) {
                return if (scheduled.airsAt > now) {
                    PlayAction(episode, null, EpisodeAiring.Upcoming(scheduled.airsAt - now), scheduled.airsAt, dub)
                } else {
                    PlayAction(episode, null, EpisodeAiring.Aired, translation = dub)
                }
            }
            return PlayAction(episode, null, entry.airing(episode), translation = dub)
        }
    }
}

/** The hero's line under the title: "Episode 7 · The Lantern Road · 14 min left". */
fun ContinueItem.heroLine(context: Context, rows: List<EpisodeRow>?): String {
    val title = rows?.firstOrNull { it.number == episode.toString() }?.title?.takeIf { it.isNotBlank() }
    val left = minutesLeft?.let { Strings.timeMinutesLeft(context, it.toInt()) }
    return listOfNotNull(longEpisode(context), title, left).joinToString(" · ") // i18n-skip
}

/** What the hero's amber action says: "Resume episode 7", "Start episode 8". */
fun ContinueItem.heroAction(context: Context): String {
    val episode = longEpisode(context).replaceFirstChar { it.lowercase() }
    return if (resume != null) Strings.tvHeroResume(context, episode) else Strings.tvHeroStart(context, episode)
}

/**
 * The title page's metadata line: "TV · Summer 2026 · 24 episodes · 79% · Atelier Hotaru". Sentence
 * case, as the phone sets it.
 */
fun Entry.metadataLine(context: Context): String {
    val parts = mutableListOf<String>()
    formatLabel(context, format)?.let { parts += it }
    val airedIn = when {
        season != null && year != null -> Strings.seasonWithYear(context, seasonName(context, season!!), year.toString())
        season != null -> seasonName(context, season!!)
        else -> year?.toString()
    }
    airedIn?.let { parts += it }
    episodes?.takeIf { it > 0u && format != "MOVIE" }?.let { parts += context.resources.getQuantityString(R.plurals.episodes_count, it.toInt(), it.toInt()) }
    score?.let { parts += percentText(it.toInt()) }
    studio?.takeIf { it.isNotBlank() }?.let { parts += it }
    return parts.joinToString(" · ") // i18n-skip
}

/** A poster's second line on Seasonal and Search: "24 episodes · 79%". */
fun Entry.posterLine(context: Context): String? = listOfNotNull(
    episodes?.takeIf { it > 0u }?.let { context.resources.getQuantityString(R.plurals.episodes_count, it.toInt(), it.toInt()) },
    score?.let { percentText(it.toInt()) },
).joinToString(" · ").ifEmpty { null } // i18n-skip

/** An episode card's line: "24 min", "14 min left", or "Watched". */
fun EpisodeRow.cardLine(context: Context, resume: ResumePoint?, playsNext: Boolean): String? {
    if (completed) return Strings.episodeWatched(context)
    val minutes = durationSecs?.toLong()?.let { maxOf(1L, Math.round(it / 60.0)) }
    if (playsNext && resume != null && minutes != null) {
        val left = maxOf(1L, Math.round((minutes * 60 - resume.position) / 60))
        return Strings.timeMinutesLeft(context, left.toInt())
    }
    return minutes?.let { Strings.timeMinutes(context, it.toInt()) }
}

/** "E7 · The Lantern Road", or "E7" alone. */
fun EpisodeRow.cardTitle(context: Context): String =
    title?.takeIf { it.isNotBlank() }?.let { Strings.episodeLineDetail(context, number.toInt(), it) } ?: Strings.episodeNumberShort(context, number.toInt())

/** Synopses from AniList carry their spoilers marked; the TV shows only the text before them. */
fun synopsisWithoutSpoilers(text: String): String =
    text.replace(Regex("~!.*?!~", RegexOption.DOT_MATCHES_ALL), "").replace(Regex("\\s+\\n"), "\n").trim()

/** What a Continue card says it plays; with no source for what is left, that no source has it. */
@androidx.compose.runtime.Composable
fun ContinueItem.actionText(): String =
    if (unsourced) androidx.compose.ui.res.stringResource(tv.anistream.i18n.R.string.episode_unsourced) else action(androidx.compose.ui.platform.LocalContext.current)

/** A season's name: "Autumn". */
fun seasonName(context: Context, season: tv.anistream.engine.model.Season): String = when (season) {
    tv.anistream.engine.model.Season.WINTER -> Strings.seasonWinter(context)
    tv.anistream.engine.model.Season.SPRING -> Strings.seasonSpring(context)
    tv.anistream.engine.model.Season.SUMMER -> Strings.seasonSummer(context)
    tv.anistream.engine.model.Season.FALL -> Strings.seasonAutumn(context)
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
