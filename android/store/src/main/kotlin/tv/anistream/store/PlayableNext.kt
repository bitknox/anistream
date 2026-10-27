// The episode a Play or Resume button plays: the engine's next one unless no source lists it. AniList
// says it is out and nothing can play it, so the button goes to the next episode that something can.

package tv.anistream.store

import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow

/** The episode a title's main button plays. */
data class PlayableNext(
    val episode: UInt,
    /** Every episode from the next one on is one no source lists: there is nothing to play. */
    val unsourced: Boolean,
)

/**
 * The next episode to play, skipping those no source lists, in [rows] when the table is loaded. A
 * part-watched episode is resumed as it is. With every later episode unsourced too the answer is
 * the engine's own next one, flagged [PlayableNext.unsourced].
 */
fun Entry.playableNext(rows: List<EpisodeRow>?): PlayableNext {
    val next = progress?.next ?: 1u
    if (resume != null || rows == null) return PlayableNext(next, false)
    val numbered = rows.mapNotNull { row -> row.number.toUIntOrNull()?.let { it to row } }
    if (numbered.none { (number, row) -> number == next && row.unsourced }) return PlayableNext(next, false)
    val later = numbered.filter { (number, row) -> number > next && !row.unsourced }.minOfOrNull { it.first }
    return later?.let { PlayableNext(it, false) } ?: PlayableNext(next, true)
}
