// What the match sheet and the title page say of a candidate, a match and the question, for the
// phone, the tablet and the TV alike.

package tv.anistream.store

import android.content.Context
import java.text.NumberFormat
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.CurrentMatch
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.MatchCandidate
import tv.anistream.engine.model.Msg
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings

object MatchWords {
    private const val SEPARATOR = " · " // i18n-skip

    private fun episodes(context: Context, count: Int) =
        context.resources.getQuantityString(R.plurals.episodes_count, count, count)

    /** "TV · 2024 · 12 episodes", from what the source lists. */
    fun facts(context: Context, candidate: MatchCandidate): String = listOfNotNull(
        candidate.format?.let { SearchWords.format(context, it) },
        candidate.year?.toString(),
        candidate.episodes?.let { episodes(context, it.toInt()) },
    ).joinToString(SEPARATOR)

    /** How close it is, "71% match", or why it was ruled out. */
    fun closeness(context: Context, candidate: MatchCandidate): String {
        candidate.rejected?.let { return it.text(context) }
        val percent = NumberFormat.getPercentInstance().apply { maximumFractionDigits = 0 }.format(candidate.similarity)
        return Strings.matchSimilarity(context, percent)
    }

    /** What AniList says of the show: "On AniList: TV · 2024 · 12 episodes". */
    fun anilist(context: Context, entry: Entry): String {
        val format = when (entry.format) {
            null -> null
            "TV" -> Strings.formatTv(context)
            "MOVIE" -> Strings.formatMovie(context)
            "TV_SHORT", "TV SHORT" -> Strings.formatTvShort(context)
            "SPECIAL" -> Strings.formatSpecial(context)
            "MUSIC" -> Strings.formatMusic(context)
            "OVA" -> Strings.formatOva(context)
            "ONA" -> Strings.formatOna(context)
            else -> entry.format?.replace('_', ' ')
        }
        val facts = listOfNotNull(
            format,
            entry.year?.toString(),
            entry.episodes?.takeIf { it > 0u }?.let { episodes(context, it.toInt()) },
        ).joinToString(SEPARATOR)
        return if (facts.isEmpty()) "" else Strings.matchAnilistFacts(context, facts)
    }

    /** The page's line while the match is open: how many titles could be it. */
    fun uncertain(context: Context, asked: MatchAsked): String =
        if (asked.possible == 0) Strings.matchUncertainNone(context, asked.source)
        else context.resources.getQuantityString(R.plurals.match_uncertain, asked.possible, asked.possible, asked.source)

    /** What the current match says of how it was made. */
    fun chosen(context: Context, match: CurrentMatch): String =
        if (match.chosenByYou) Strings.matchChosenByYou(context) else Strings.matchChosenAutomatically(context)
}

/** What the list area holds. */
sealed interface MatchShown {
    data class Candidates(val providerId: String, val source: String, val candidates: List<MatchCandidate>) : MatchShown
    data class Searching(val source: String, val query: String) : MatchShown
    data class Empty(val source: String, val query: String) : MatchShown
    data class Failed(val source: String, val reason: Msg) : MatchShown
    data object Nothing : MatchShown
}

/** What the list area shows for [id]: a search by hand if one is running, else the question. */
fun matchShown(state: MatchState, id: AnilistId, sourceName: String): MatchShown = when (val search = state.searches[id]) {
    is MatchSearch.Searching -> MatchShown.Searching(sourceName, search.query)
    is MatchSearch.Found -> MatchShown.Candidates(search.providerId, search.source, search.candidates)
    is MatchSearch.Empty -> MatchShown.Empty(search.source, search.query)
    is MatchSearch.Failed -> MatchShown.Failed(sourceName, search.reason)
    null -> state.asked[id]?.let { MatchShown.Candidates(it.providerId, it.source, it.candidates) } ?: MatchShown.Nothing
}
