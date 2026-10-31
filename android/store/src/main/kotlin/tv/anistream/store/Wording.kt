// What the model's types do in Rust beside their fields, which the bindings carry no further than
// the fields: each a port of the method it is named after, kept to the same rule, so that a screen
// says what the terminal says.

package tv.anistream.store

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import android.content.Context
import tv.anistream.i18n.Strings
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
 * A broadcast's time as the screens say it, in the person's language on the system's clock:
 * "Saturday 18:30" within the week, "22 October 18:30" further off, and "Saturday 6:30 pm" where
 * the system reads 12 hours.
 */
object AirTime {
    /** The language the screens are in now. */
    val locale: Locale get() = Locale.getDefault()

    /** Whether the system reads a 24-hour clock: the app sets it from Android's own setting. */
    @Volatile
    var twentyFourHour: Boolean = true

    /** The pattern the locale writes [skeleton] with, or [fallback] where Android's isn't there (a plain JVM test). */
    fun bestPattern(skeleton: String, fallback: String): String =
        runCatching { android.text.format.DateFormat.getBestDateTimePattern(locale, skeleton) }.getOrNull()?.takeIf { it.isNotBlank() } ?: fallback

    private val time: DateTimeFormatter
        get() = if (twentyFourHour) {
            DateTimeFormatter.ofPattern(bestPattern("Hm", "HH:mm"), locale)
        } else {
            DateTimeFormatter.ofPattern(bestPattern("hm", "h:mm a"), locale)
        }
    private val date: DateTimeFormatter get() = DateTimeFormatter.ofPattern(bestPattern("dMMMM", "d MMMM"), locale)

    /** The time of day alone, on the system's clock: "18:30" or "6:30 pm". */
    fun clock(at: Instant, zone: ZoneId = AppClock.zone): String = time.format(at.atZone(zone))

    /** [local]'s time of day, on the system's clock. */
    fun clock(local: java.time.temporal.TemporalAccessor): String = time.format(local)

    /**
     * A time coming up this week, as Notifications' "Coming up" says it: "Today 22:00",
     * "Tomorrow 16:30", "Thursday 16:30".
     */
    fun coming(context: Context, at: Instant, now: Instant = AppClock.now(), zone: ZoneId = AppClock.zone): String {
        val local = at.atZone(zone)
        val today = now.atZone(zone).toLocalDate()
        val day = when (local.toLocalDate()) {
            today -> Strings.dayToday(context)
            today.plusDays(1) -> Strings.dayTomorrow(context)
            else -> local.dayOfWeek.getDisplayName(TextStyle.FULL, locale)
        }
        return "$day ${time.format(local)}"
    }

    fun words(at: Instant, now: Instant = AppClock.now(), zone: ZoneId = AppClock.zone): String {
        val local = at.atZone(zone)
        val days = (at.epochSecond - now.epochSecond) / 86_400.0
        val day = if (days < 6) local.dayOfWeek.getDisplayName(TextStyle.FULL, locale) else date.format(local)
        return "$day ${time.format(local)}"
    }
}

/**
 * The words for counts and spans of time: `anistream_model::wording`, so a screen says a wait or a
 * count as the terminal does, in the catalogue's units.
 */
object Wording {
    /** A countdown in the largest unit that still reads at a glance: `6d 17h`, `2h 5m`, `4m`, `now`. */
    fun countdown(context: Context, seconds: Long): String {
        if (seconds <= 0) return Strings.timeNowInline(context)
        val minutes = seconds / 60
        val hours = minutes / 60
        val days = hours / 24
        if (days >= 1) {
            val hoursLeft = (hours % 24).toInt()
            return if (hoursLeft > 0 && days < 7) Strings.durationDaysHours(context, days.toInt(), hoursLeft) else Strings.durationDays(context, days.toInt())
        }
        if (hours >= 1) {
            val minutesLeft = (minutes % 60).toInt()
            return if (minutesLeft > 0) Strings.durationHoursMinutes(context, hours.toInt(), minutesLeft) else Strings.durationHours(context, hours.toInt())
        }
        return Strings.durationMinutes(context, minutes.toInt())
    }

    /** How long ago: `2d ago`, or `just now` under a minute. */
    fun ago(context: Context, seconds: Long): String =
        if (seconds < 60) Strings.timeJustNowInline(context) else Strings.timeAgo(context, countdown(context, seconds))

    /** "Hidden 3d ago" under a hidden title; null when the engine did not keep the time. */
    fun hiddenLine(context: Context, hidden: tv.anistream.engine.model.HiddenTitle): String? {
        if (hidden.hiddenAt <= 0) return null
        val seconds = (System.currentTimeMillis() / 1000 - hidden.hiddenAt).coerceAtLeast(0)
        return Strings.settingsHiddenOn(context, ago(context, seconds))
    }

    /** Names in a sentence: "Kumo", "Kumo and Natsukaze", "Kumo, Hoshi and Natsukaze". */
    fun list(names: List<String>): String = when (names.size) {
        0 -> ""
        1 -> names[0]
        else -> android.icu.text.ListFormatter.getInstance(AirTime.locale).format(names)
    }

    /**
     * A size as people read it: `9.4 GB`, `140 MB`, `0 B`, in decimal units, one decimal below
     * ten and none above, the number in the person's format. `human_bytes`.
     */
    fun bytes(context: Context, bytes: Long): String {
        val scale = listOf(1_000_000_000L, 1_000_000L, 1_000L).firstOrNull { bytes >= it } ?: 1L
        val value = bytes.toDouble() / scale
        val decimals = if (value < 10 && scale > 1) 1 else 0
        val number = java.text.NumberFormat.getInstance(AirTime.locale).apply {
            minimumFractionDigits = decimals
            maximumFractionDigits = decimals
        }.format(if (bytes <= 0) 0.0 else value)
        return when (scale) {
            1_000_000_000L -> Strings.sizeGigabyte(context, number)
            1_000_000L -> Strings.sizeMegabyte(context, number)
            1_000L -> Strings.sizeKilobyte(context, number)
            else -> Strings.sizeByte(context, number)
        }
    }
}

/** The list's name: "Watching". `LibrarySegment::label`. */
fun LibrarySegment.label(context: Context): String = when (this) {
    LibrarySegment.WATCHING -> Strings.libraryStatusWatching(context)
    LibrarySegment.PLANNING -> Strings.libraryStatusPlanning(context)
    LibrarySegment.COMPLETED -> Strings.libraryStatusCompleted(context)
    LibrarySegment.PAUSED -> Strings.libraryStatusPaused(context)
    LibrarySegment.DROPPED -> Strings.libraryStatusDropped(context)
}

/** The list's name inside a sentence: "watching", as in "Nothing on your watching list". */
fun LibrarySegment.inSentence(context: Context): String = label(context).lowercase(AirTime.locale)

/** The engine's directories under one root, `config`, `data` and `cache`, as a test lays them out. */
fun ffiDirs(root: java.io.File): FfiDirs = FfiDirs(
    config = root.resolve("config").path,
    data = root.resolve("data").path,
    cache = root.resolve("cache").path,
)
