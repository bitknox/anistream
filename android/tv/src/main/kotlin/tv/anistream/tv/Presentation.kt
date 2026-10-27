// How a title's fields read on the TV: what the amber action plays and says, the hero's line and
// the title page's metadata line. The phone has its own in :ui; these follow the TV brief's words.

package tv.anistream.tv

import java.time.Instant
import java.time.format.TextStyle
import java.util.Locale
import tv.anistream.engine.core.Translation
import tv.anistream.engine.ffi.episodeLine
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
    fun label(now: Instant = AppClock.now()): String = when {
        resume != null -> "Resume ${episodeLine("episode $episode", translation)}"
        playable -> "Play ${episodeLine("episode $episode", translation)}"
        else -> outDay(now)?.let { "Play episode $episode (out $it)" } ?: "Play episode $episode (not out yet)"
    }

    private fun outDay(now: Instant): String? {
        val at = airsAt?.let(Instant::ofEpochSecond)
            ?: (airing as? EpisodeAiring.Upcoming)?.inSeconds?.let(now::plusSeconds)
            ?: return null
        val zone = AppClock.zone
        val days = (at.epochSecond - now.epochSecond) / 86_400.0
        val local = at.atZone(zone)
        return when {
            local.toLocalDate() == now.atZone(zone).toLocalDate() -> "today"
            days < 6 -> local.dayOfWeek.getDisplayName(TextStyle.FULL, AirTime.locale)
            else -> "${local.dayOfMonth} ${local.month.getDisplayName(TextStyle.FULL, AirTime.locale)}"
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
fun ContinueItem.heroLine(rows: List<EpisodeRow>?): String {
    val title = rows?.firstOrNull { it.number == episode.toString() }?.title?.takeIf { it.isNotBlank() }
    val left = minutesLeft?.let { "$it min left" }
    return listOfNotNull(longEpisode, title, left).joinToString(" · ")
}

/** What the hero's amber action says: "Resume episode 7", "Start episode 8". */
val ContinueItem.heroAction: String
    get() = if (resume != null) "Resume ${longEpisode.replaceFirstChar { it.lowercase() }}" else "Start ${longEpisode.replaceFirstChar { it.lowercase() }}"

/**
 * The title page's metadata line, set in capitals: "TV · SUMMER 2026 · 24 EPS · 79 % ·
 * ATELIER HOTARU".
 */
val Entry.metadataLine: String
    get() {
        val parts = mutableListOf<String>()
        format?.let { parts += it.replace('_', ' ') }
        val airedIn = listOfNotNull(season?.name, year?.toString()).joinToString(" ")
        if (airedIn.isNotEmpty()) parts += airedIn
        episodes?.takeIf { it > 0u && format != "MOVIE" }?.let { parts += if (it == 1u) "1 EP" else "$it EPS" }
        score?.let { parts += "$it %" }
        studio?.takeIf { it.isNotBlank() }?.let { parts += it }
        return parts.joinToString(" · ").uppercase(Locale.ROOT)
    }

/** A poster's second line on Seasonal and Search: "24 eps · 79 %". */
val Entry.posterLine: String?
    get() = listOfNotNull(
        episodes?.takeIf { it > 0u }?.let { if (it == 1u) "1 ep" else "$it eps" },
        score?.let { "$it %" },
    ).joinToString(" · ").ifEmpty { null }

/** An episode card's line: "24 min", "14 min left", or "Watched". */
fun EpisodeRow.cardLine(resume: ResumePoint?, playsNext: Boolean): String? {
    if (completed) return "Watched"
    val minutes = durationSecs?.toLong()?.let { maxOf(1L, Math.round(it / 60.0)) }
    if (playsNext && resume != null && minutes != null) {
        val left = maxOf(1L, Math.round((minutes * 60 - resume.position) / 60))
        return "$left min left"
    }
    return minutes?.let { "$it min" }
}

/** "E7 · The Lantern Road", or "E7" alone. */
val EpisodeRow.cardTitle: String
    get() = title?.takeIf { it.isNotBlank() }?.let { "E$number · $it" } ?: "E$number"

/** Synopses from AniList carry their spoilers marked; the TV shows only the text before them. */
fun synopsisWithoutSpoilers(text: String): String =
    text.replace(Regex("~!.*?!~", RegexOption.DOT_MATCHES_ALL), "").replace(Regex("\\s+\\n"), "\n").trim()

/** What a Continue card says it plays; with no source for what is left, that no source has it. */
@androidx.compose.runtime.Composable
fun ContinueItem.actionText(): String =
    if (unsourced) androidx.compose.ui.res.stringResource(tv.anistream.i18n.R.string.episode_unsourced) else action
