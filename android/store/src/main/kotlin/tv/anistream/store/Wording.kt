// What the model's types do in Rust beside their fields, which the bindings carry no further than
// the fields: each a port of the method it is named after, kept to the same rule, so that a screen
// says what the terminal says.

package tv.anistream.store

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import tv.anistream.engine.ffi.FfiDirs
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.ResumePoint

/** `12:30`, or `1:02:03` past the hour: the position as a viewer thinks of it. `ResumePoint::clock`. */
fun ResumePoint.clock(): String {
    val total = maxOf(position, 0.0).toLong()
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val seconds = total % 60
    return if (hours > 0) "%d:%02d:%02d".format(hours, minutes, seconds) else "%d:%02d".format(minutes, seconds)
}

/** The newest episode known to be out, and how many seconds ago when that is known too. */
data class LatestAired(val episode: UInt, val ago: Long?)

/**
 * The newest episode known to be out. `Entry::latest_aired`.
 *
 * Holding episode N + 1 as next proves N is out, so the larger claim wins over a schedule row that
 * lags; the "ago" is carried only when it belongs to the episode named.
 */
fun Entry.latestAired(): LatestAired? {
    val proven = nextEpisode?.takeIf { it >= 2u }?.minus(1u)
    val aired = lastAired
    return when {
        aired != null && proven != null && proven > aired.episode -> LatestAired(proven, null)
        aired != null -> LatestAired(aired.episode, aired.ago)
        else -> proven?.let { LatestAired(it, null) }
    }
}

/**
 * Whether [episode] has aired, by the title's broadcasts: out when it is no later than the newest
 * one known to be out; not out when it is the next broadcast or after and that is still ahead; out
 * otherwise, as every episode of a finished title is.
 */
fun Entry.airing(episode: UInt): EpisodeAiring {
    val aired = latestAired()
    if (aired != null && episode <= aired.episode) return EpisodeAiring.Aired
    val next = nextEpisode
    if (next == null || episode < next) return EpisodeAiring.Aired
    if (episode == next) {
        val left = nextAiringIn
        // The countdown ran out since the schedule came: it is out.
        if (left != null && left <= 0) return EpisodeAiring.Aired
        return EpisodeAiring.Upcoming(left)
    }
    return EpisodeAiring.Upcoming(null)
}

/** Whether an episode can play yet, by its title's broadcasts. */
sealed interface EpisodeAiring {
    /** It is out, and plays. */
    data object Aired : EpisodeAiring

    /** Not out yet: it airs in [inSeconds] when the schedule dates it, as it does the next one. */
    data class Upcoming(val inSeconds: Long?) : EpisodeAiring

    val hasAired: Boolean get() = this == Aired

    /** When it airs, as the screens say it: "Saturday 18:30", or `null` when out or undated. */
    fun airs(now: Instant = AppClock.now(), zone: ZoneId = AppClock.zone): String? {
        val seconds = (this as? Upcoming)?.inSeconds ?: return null
        return AirTime.words(now.plusSeconds(seconds), now, zone)
    }
}

/**
 * A broadcast's time as the screens say it, in English on a 24-hour clock as the boards set it:
 * "Saturday 18:30" within the week, "22 October 18:30" further off.
 */
object AirTime {
    val locale: Locale = Locale.UK
    private val time = DateTimeFormatter.ofPattern("HH:mm", locale)
    private val date = DateTimeFormatter.ofPattern("d MMMM", locale)

    fun words(at: Instant, now: Instant = AppClock.now(), zone: ZoneId = AppClock.zone): String {
        val local = at.atZone(zone)
        val days = (at.epochSecond - now.epochSecond) / 86_400.0
        val day = if (days < 6) local.dayOfWeek.getDisplayName(TextStyle.FULL, locale) else date.format(local)
        return "$day ${time.format(local)}"
    }
}

/**
 * The words for counts and spans of time: `anistream_model::wording`, so a screen says a wait or a
 * count as the terminal does.
 */
object Wording {
    /** A countdown in the largest unit that still reads at a glance: `6d 17h`, `2h 5m`, `4m`, `now`. */
    fun countdown(seconds: Long): String {
        if (seconds <= 0) return "now"
        val minutes = seconds / 60
        val hours = minutes / 60
        val days = hours / 24
        if (days >= 1) {
            val hoursLeft = hours % 24
            return if (hoursLeft > 0 && days < 7) "${days}d ${hoursLeft}h" else "${days}d"
        }
        if (hours >= 1) {
            val minutesLeft = minutes % 60
            return if (minutesLeft > 0) "${hours}h ${minutesLeft}m" else "${hours}h"
        }
        return "${minutes}m"
    }

    /** How long ago: `2d ago`, or `just now` under a minute. */
    fun ago(seconds: Long): String = if (seconds < 60) "just now" else "${countdown(seconds)} ago"

    /** A count and its noun, singular at one: `1 episode`, `3 episodes`. */
    fun plural(count: Int, noun: String): String = if (count == 1) "1 $noun" else "$count ${noun}s"

    /** Names in a sentence: "Kumo", "Kumo and Natsukaze", "Kumo, Hoshi and Natsukaze". */
    fun list(names: List<String>): String = when (names.size) {
        0 -> ""
        1 -> names[0]
        else -> names.dropLast(1).joinToString(", ") + " and " + names.last()
    }

    /**
     * A size as people read it: `9.4 GB`, `140 MB`, `0 B`, in decimal units, one decimal below
     * ten and none above. `human_bytes`.
     */
    fun bytes(bytes: Long): String {
        for ((unit, scale) in listOf("GB" to 1_000_000_000L, "MB" to 1_000_000L, "kB" to 1_000L, "B" to 1L)) {
            if (bytes >= scale) {
                val value = bytes.toDouble() / scale
                return if (value < 10 && scale > 1) String.format(Locale.ROOT, "%.1f %s", value, unit)
                else "${Math.round(value)} $unit"
            }
        }
        return "0 B"
    }
}

/** The list's name in lower case: "watching". `LibrarySegment::label`. */
val LibrarySegment.label: String get() = name.lowercase(Locale.ROOT)

/** The engine's directories under one root, `config`, `data` and `cache`, as a test lays them out. */
fun ffiDirs(root: java.io.File): FfiDirs = FfiDirs(
    config = root.resolve("config").path,
    data = root.resolve("data").path,
    cache = root.resolve("cache").path,
)
