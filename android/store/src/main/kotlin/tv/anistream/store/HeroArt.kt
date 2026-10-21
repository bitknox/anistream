// A title's wide art: the banner, else a still from an episode already seen, else nothing wide,
// and the screens set the cover beside the title instead. The player's starting screen follows
// the same rule.

package tv.anistream.store

import uniffi.anistream_model.Entry
import uniffi.anistream_model.EpisodeRow

sealed interface HeroArt {
    data class Banner(override val url: String) : HeroArt
    data class Still(override val url: String) : HeroArt

    /**
     * No banner, and the episodes whose stills might stand in are on their way: the cover's
     * colour as a plate, without the cover, so nothing shows twice and nothing jumps.
     */
    data object Waiting : HeroArt

    /** Nothing wide to show. */
    data object Compact : HeroArt

    /** The URL to draw, when there is one. */
    val url: String? get() = null

    companion object {
        /** The art for [entry], with its episode table as far as it is known: `null` while it loads. */
        fun of(entry: Entry, episodes: List<EpisodeRow>?): HeroArt {
            entry.bannerUrl?.let { return Banner(it) }
            episodes ?: return Waiting
            return seenStill(episodes, entry.progress?.done ?: 0u)?.let(::Still) ?: Compact
        }

        /**
         * The still of the latest episode watched that has one, else episode 1's: never one from
         * an episode not yet seen past the first. Watched is the table's own word, or within what
         * the tracker counts as done.
         */
        fun seenStill(rows: List<EpisodeRow>, done: UInt): String? {
            val watched = rows.filter { row ->
                row.completed || (row.number.toDoubleOrNull()?.let { it <= done.toDouble() } == true)
            }
            watched.filter { it.thumbnail != null }
                .maxByOrNull { it.number.toDoubleOrNull() ?: 0.0 }
                ?.thumbnail?.let { return it }
            val first = rows.firstOrNull { it.number.toDoubleOrNull() == 1.0 } ?: rows.firstOrNull()
            return first?.thumbnail
        }
    }
}
