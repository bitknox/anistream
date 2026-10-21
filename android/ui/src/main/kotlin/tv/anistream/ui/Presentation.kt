// How a title's fields read on screen.

package tv.anistream.ui

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import uniffi.anistream_model.Entry
import tv.anistream.store.CalendarModel

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
