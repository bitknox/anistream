// Continue: what you were watching, from local history, as a short queue.

package tv.anistream.store

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.ResumePoint
import tv.anistream.i18n.Strings

/** One row of the queue: a title and the episode it plays next. */
data class ContinueItem(
    val entry: Entry,
    /** The episode the row plays. */
    val episode: UInt,
    /** Where that episode was left, when it was. */
    val resume: ResumePoint?,
    /** How many seconds ago that episode aired, when it is the newest out and aired this week. */
    val airedAgo: Long?,
    /** Nothing from the next episode on has a source, so there is nothing to play. */
    val unsourced: Boolean = false,
) {
    val id: AnilistId get() = entry.id

    /** Whether this is a new episode rather than one under way. */
    val isNew: Boolean get() = airedAgo != null

    /** Whether the episode is out: one left part-way is, and otherwise the title's broadcasts say. */
    val airing: EpisodeAiring get() = if (resume == null) entry.airing(episode) else EpisodeAiring.Aired

    /**
     * Whether the row has an episode to play: not when everything out is watched and the next
     * one is still to air, and not when no source lists what is left.
     */
    val playable: Boolean get() = airing.hasAired && !unsourced

    /**
     * What the row says it plays: "Continue E7 · 10:06", "Play E4", "Play E9 · out 2h ago", or
     * with nothing out to watch, "Caught up · E9 Saturday 18:30".
     */
    fun action(context: Context): String {
        val dubbed = entry.watchedIn == Translation.DUB
        return when {
            resume != null ->
                if (dubbed) Strings.continueRowResumeDubbed(context, episode.toInt(), resume.clock()) else Strings.continueRowResume(context, episode.toInt(), resume.clock())
            unsourced -> shortEpisode(context)
            !playable -> airing.airs()?.let { Strings.continueRowCaughtUp(context, episode.toInt(), it) } ?: Strings.continueRowCaughtUpPlain(context)
            airedAgo != null ->
                if (dubbed) Strings.continueRowStartAiredDubbed(context, episode.toInt(), Wording.ago(context, airedAgo))
                else Strings.continueRowStartAired(context, episode.toInt(), Wording.ago(context, airedAgo))
            else -> if (dubbed) Strings.continueRowStartDubbed(context, episode.toInt()) else Strings.continueRowStart(context, episode.toInt())
        }
    }

    /** "E7", or "E7 · Dubbed" for a title last watched dubbed: only a dub is marked, by the engine's rule. */
    fun shortEpisode(context: Context): String =
        if (entry.watchedIn == Translation.DUB) Strings.episodeShortDubbed(context, episode.toInt()) else Strings.episodeNumberShort(context, episode.toInt())

    /** "Episode 7", or "Episode 7 · Dubbed": the long form, for the lines that name the episode. */
    fun longEpisode(context: Context): String =
        if (entry.watchedIn == Translation.DUB) Strings.episodeLongDubbed(context, episode.toInt()) else Strings.episodeNumber(context, episode.toInt())

    /** How far into the episode, for the row's meter, when it was left part-way. */
    val fraction: Double? get() = resume?.fraction

    /** Whole minutes left in the episode, when its runtime is known: "14 min left". */
    val minutesLeft: Int?
        get() {
            val resume = resume ?: return null
            val fraction = resume.fraction?.takeIf { it > 0 } ?: return null
            val left = resume.position / fraction - resume.position
            return maxOf(1, Math.round(left / 60).toInt())
        }

    companion object {
        /** What counts as this week. */
        const val WEEK: Long = 7 * 86_400

        fun of(entry: Entry, rows: List<EpisodeRow>? = null): ContinueItem {
            val target = entry.playableNext(rows)
            val episode = target.episode
            val aired = entry.latestAired()
            val airedAgo = aired?.takeIf { entry.resume == null && it.episode == episode }
                ?.ago?.takeIf { it < WEEK }
            return ContinueItem(entry, episode, entry.resume, airedAgo, target.unsourced)
        }
    }
}

/** The queue, as Continue groups it. */
data class ContinueQueue(
    /** Titles under way: an episode left part-way, or the next one waiting. */
    val inProgress: List<ContinueItem>,
    /** Titles whose next episode is the newest out, and aired in the last week. */
    val newThisWeek: List<ContinueItem>,
) {
    val all: List<ContinueItem> get() = inProgress + newThisWeek
    val isEmpty: Boolean get() = inProgress.isEmpty() && newThisWeek.isEmpty()

    /** The row Continue leads with, as its hero: the first under way, else the newest out. */
    val lead: ContinueItem? get() = inProgress.firstOrNull() ?: newThisWeek.firstOrNull()

    /**
     * The first row with an episode out to play, in Continue's order: what the launcher's
     * Continue shortcut plays.
     */
    val nextUp: ContinueItem? get() = all.firstOrNull { it.playable }

    companion object {
        /** At most this many rows: Continue is a shortlist, not a second library. */
        const val ROWS = 5

        /** The queue of [ids], newest watch first as the engine orders them, rows from [titles]. */
        fun of(
            ids: List<AnilistId>,
            titles: Map<AnilistId, Entry>,
            episodes: Map<AnilistId, List<EpisodeRow>> = emptyMap(),
            rows: Int = ROWS,
        ): ContinueQueue {
            val items = ids.mapNotNull { titles[it] }.take(rows).map { ContinueItem.of(it, episodes[it.id]) }
            return ContinueQueue(items.filterNot { it.isNew }, items.filter { it.isNew })
        }
    }
}

/** The Continue list. */
class ContinueModel {
    val list = ListModel()

    private val _hidden = MutableStateFlow<Set<AnilistId>>(emptySet())

    /** Titles taken off the queue in this session, ahead of the engine's next answer. */
    val hidden: StateFlow<Set<AnilistId>> = _hidden.asStateFlow()

    /** The queue as it stands. */
    fun queue(titles: Map<AnilistId, Entry>, episodes: Map<AnilistId, List<EpisodeRow>> = emptyMap(), rows: Int = ContinueQueue.ROWS): ContinueQueue =
        ContinueQueue.of(list.state.value.ids.filterNot { it in _hidden.value }, titles, episodes, rows)

    internal fun apply(content: ContentList, titles: TitleCache) {
        // Continue's rows carry this device's watch history; no other list's may overwrite it.
        list.apply(content, titles, local = true)
        _hidden.value = emptySet()
    }

    internal fun hide(id: AnilistId) {
        _hidden.value = _hidden.value + id
    }
}
