// Continue: what you were watching, from local history, as a short queue.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.ResumePoint

/** One row of the queue: a title and the episode it plays next. */
data class ContinueItem(
    val entry: Entry,
    /** The episode the row plays. */
    val episode: UInt,
    /** Where that episode was left, when it was. */
    val resume: ResumePoint?,
    /** How many seconds ago that episode aired, when it is the newest out and aired this week. */
    val airedAgo: Long?,
) {
    val id: AnilistId get() = entry.id

    /** Whether this is a new episode rather than one under way. */
    val isNew: Boolean get() = airedAgo != null

    /** Whether the episode is out: one left part-way is, and otherwise the title's broadcasts say. */
    val airing: EpisodeAiring get() = if (resume == null) entry.airing(episode) else EpisodeAiring.Aired

    /** Whether the row has an episode to play: not when everything out is watched. */
    val playable: Boolean get() = airing.hasAired

    /**
     * What the row says it plays: "Resume E7 · 10:06", "Start E4", "Start E9 · out 2h ago", or
     * with nothing out to watch, "Caught up · E9 Saturday 18:30".
     */
    val action: String
        get() = when {
            resume != null -> "Resume E$episode · ${resume.clock()}"
            !playable -> "Caught up · E$episode " + (airing.airs() ?: "not out yet")
            airedAgo != null -> "Start E$episode · out ${Wording.ago(airedAgo)}"
            else -> "Start E$episode"
        }

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

        fun of(entry: Entry): ContinueItem {
            val episode = entry.progress?.next ?: 1u
            val aired = entry.latestAired()
            val airedAgo = aired?.takeIf { entry.resume == null && it.episode == episode }
                ?.ago?.takeIf { it < WEEK }
            return ContinueItem(entry, episode, entry.resume, airedAgo)
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

    companion object {
        /** At most this many rows: Continue is a shortlist, not a second library. */
        const val ROWS = 5

        /** The queue of [ids], newest watch first as the engine orders them, rows from [titles]. */
        fun of(ids: List<AnilistId>, titles: Map<AnilistId, Entry>): ContinueQueue {
            val items = ids.mapNotNull { titles[it] }.take(ROWS).map(ContinueItem::of)
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
    fun queue(titles: Map<AnilistId, Entry>): ContinueQueue =
        ContinueQueue.of(list.state.value.ids.filterNot { it in _hidden.value }, titles)

    internal fun apply(content: ContentList, titles: TitleCache) {
        list.apply(content, titles)
        _hidden.value = emptySet()
    }

    internal fun hide(id: AnilistId) {
        _hidden.value = _hidden.value + id
    }
}
