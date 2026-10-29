// What the two widgets show, worked out from the record the app keeps: plain values, so the
// states (signed out, caught up, stale, nothing today) are tested without drawing anything.

package tv.anistream.widgets

import tv.anistream.i18n.Strings
import android.content.Context
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.time.temporal.ChronoUnit
import java.util.Locale
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.episodeMark
import tv.anistream.engine.model.WidgetAiring
import tv.anistream.engine.model.WidgetData
import tv.anistream.ui.airsWords

/**
 * The record as the widgets read it: the engine's `WidgetData`. What a record of the first shape
 * does not say is left unknown (`null`), so each state falls back to what it can tell; the second
 * says who is signed in, what airs next, the week's broadcasts and each title's time left.
 */
data class Record(
    val generatedAt: Instant,
    /** Whether a tracker is signed in. Unknown in a first-shape record. */
    val signedIn: Boolean? = null,
    val continuing: List<Continuing> = emptyList(),
    /** The next episode due from the list, for "caught up". Unknown in a first-shape record. */
    val nextAiring: Next? = null,
    /** The broadcasts from the day the record was made on, aired or not, in the order they air. */
    val airing: List<Airing> = emptyList(),
    /** How many days from the record's own [airing] covers: one in the first shape, seven in the second. */
    val days: Int = 1,
) {
    data class Continuing(
        val id: AnilistId,
        val title: String,
        /** The episode Continue starts. */
        val episode: UInt,
        val episodes: UInt? = null,
        /** How far into [episode] it was left, nought to one, when it was started. */
        val fraction: Double? = null,
        /** Whether [episode] was left part-way. Unknown in a first-shape record, where [fraction] tells. */
        val resume: Boolean? = null,
        /** Minutes left of [episode], rounded up. Unknown in a first-shape record. */
        val minutesLeft: UInt? = null,
        /** The poster. */
        val coverUrl: String? = null,
        /** The wide art, AniList's banner, where the title has one. */
        val bannerUrl: String? = null,
        /** The cover's colour, `#5d9be4`, for the plate drawn before the art is there. */
        val coverColor: String? = null,
        /** The engine's mark for [episode]: "Dub" for a show watched dubbed, nothing for a sub. */
        val mark: String? = null,
    )

    data class Airing(
        val id: AnilistId,
        val title: String,
        val episode: UInt,
        val airsAt: Instant,
        /** Whether the show is in the list. */
        val onList: Boolean,
        /** Whether the episode has been watched. Unknown in a first-shape record. */
        val watched: Boolean? = null,
        /** The engine's mark for a broadcast of the dub alone: "Dub". A sub's is not marked. */
        val translation: String? = null,
    )

    data class Next(val id: AnilistId, val title: String, val episode: UInt, val airsAt: Instant)

    companion object {
        /** The engine's record, as the widgets read it, of either shape. */
        fun of(data: WidgetData): Record {
            val second = data.version >= 2u
            fun airing(row: WidgetAiring) = Airing(
                id = row.id,
                title = row.title,
                episode = row.episode,
                airsAt = Instant.ofEpochSecond(row.airsAt),
                onList = row.onList,
                watched = row.watched.takeIf { second },
                translation = episodeMark(row.translation),
            )
            return Record(
                generatedAt = Instant.ofEpochSecond(data.generatedAt),
                signedIn = data.signedIn.takeIf { second },
                continuing = data.continueWatching.map {
                    Continuing(
                        id = it.id,
                        title = it.title,
                        episode = it.episode,
                        episodes = it.episodes,
                        fraction = it.fraction,
                        resume = it.resume.takeIf { second },
                        minutesLeft = it.minutesLeft,
                        coverUrl = it.coverUrl,
                        bannerUrl = it.bannerUrl,
                        coverColor = it.coverColor,
                        mark = it.mark,
                    )
                },
                nextAiring = data.nextAiring?.let { Next(it.id, it.title, it.episode, Instant.ofEpochSecond(it.airsAt)) },
                airing = (data.airingToday + data.nextDays).map(::airing).sortedBy { it.airsAt },
                days = if (second) 7 else 1,
            )
        }
    }
}

/** What Continue watching shows. */
sealed interface ContinueView {
    /** No tracker and nothing played here, or no record yet: a tap opens Settings › Accounts. */
    data object SignedOut : ContinueView

    /** Signed in with nothing to continue, and what airs next from the list when that is known. */
    data class CaughtUp(val next: Record.Next?) : ContinueView

    /**
     * The titles to continue, most recent first. [updated] says when the record is from, in place
     * of progress, once it is more than a day old.
     */
    data class Shows(val shows: List<Show>, val updated: String?) : ContinueView

    data class Show(
        val id: AnilistId,
        val title: String,
        val episode: UInt,
        /**
         * "E7 · 14 min left", "E7 of 24", or "Start E4" for an episode not begun, each with the
         * engine's mark after the episode for a dub: "E7 · Dub · 14 min left".
         */
        val line: String,
        /** The engine's mark, "Dub", or nothing for a sub. */
        val mark: String? = null,
        /** How far into the episode, for the bar; `null` when it was not begun. */
        val progress: Float?,
        /** The art for the wide places: the banner, or the poster where there is none. */
        val wideUrl: String?,
        /** The poster, for the rows at 4 × 3. */
        val posterUrl: String?,
        /** The plate's colour while there is no art: the cover's, as ARGB. */
        val plate: Long?,
    )
}

/** What Airing today shows. */
sealed interface AiringView {
    /** No record for today: the app has not run since the day began, or not at all yet. */
    data object NoSchedule : AiringView

    /** Nothing airs today, and what airs next from the list when that is known. */
    data class NothingToday(val next: Record.Next?) : AiringView

    /** Today's broadcasts, in the order they air. [marks] is false when there is no list to mark. */
    data class Today(val weekday: String, val rows: List<Row>, val marks: Boolean) : AiringView

    data class Row(
        val id: AnilistId,
        val time: String,
        val title: String,
        val episode: UInt,
        val onList: Boolean,
        /** Whether it has aired. */
        val aired: Boolean,
        /** Aired, in the list, and not watched: "E9 out" with the water dot. */
        val out: Boolean,
        /** Aired and watched. */
        val watched: Boolean,
        val translation: String?,
    ) {
        /** What the wide layout's right-hand column says: "E9 out", "E9 · Dub", "E4". */
        fun state(context: Context): String = when {
            out -> Strings.calendarEpisodeOut(context, episode.toInt())
            translation != null -> Strings.episodeNumberShort(context, episode.toInt()) + " · " + translation // i18n-skip
            else -> Strings.episodeNumberShort(context, episode.toInt())
        }
    }
}

/** How times are said: the device's own clock format, in its language. */
data class Words(val context: Context, val twentyFourHour: Boolean = true, val locale: Locale = Locale.UK) {
    fun time(at: Instant, zone: ZoneId): String =
        DateTimeFormatter.ofPattern(if (twentyFourHour) "HH:mm" else "h:mm a", locale).format(at.atZone(zone))

    /** "today 16:30", "tomorrow 16:30", "Thursday 16:30", as the title pages say when. */
    fun airs(at: Instant, now: Instant, zone: ZoneId): String = airsWords(context, at, now, zone, twentyFourHour, locale)

    /** When a record was made, for "Updated …": "yesterday", "Sunday", or "12 Oct" further back. */
    fun day(at: Instant, now: Instant, zone: ZoneId): String {
        val day = LocalDate.ofInstant(at, zone)
        val days = ChronoUnit.DAYS.between(day, LocalDate.ofInstant(now, zone))
        return when {
            days <= 0L -> Strings.widgetDayToday(context)
            days == 1L -> Strings.widgetDayYesterday(context)
            days < 7L -> day.dayOfWeek.getDisplayName(TextStyle.FULL, locale)
            else -> DateTimeFormatter.ofPattern(android.text.format.DateFormat.getBestDateTimePattern(locale, "dMMM"), locale).format(day)
        }
    }
}

object WidgetModel {
    /** How old a record may be before Continue says when it is from: a day. */
    const val CONTINUE_FRESH_SECS = 24L * 3_600

    /** The most titles Continue watching draws: the first, and two more at 4 × 3. */
    const val CONTINUE_SHOWS = 3

    fun continuing(record: Record?, now: Instant, zone: ZoneId, words: Words): ContinueView {
        record ?: return ContinueView.SignedOut
        if (record.continuing.isEmpty()) {
            // A first-shape record does not say whether a tracker is signed in. A show in the list
            // airing today says there is a list, so caught up; otherwise there is nothing to tell
            // signed out from, and signed out is what asks for the step that fills the widget.
            val signedIn = record.signedIn ?: record.airing.any { it.onList }
            if (!signedIn) return ContinueView.SignedOut
            return ContinueView.CaughtUp(next(record, now))
        }
        val stale = now.epochSecond - record.generatedAt.epochSecond > CONTINUE_FRESH_SECS
        return ContinueView.Shows(
            shows = record.continuing.take(CONTINUE_SHOWS).map { show(words.context, it) },
            updated = if (stale) Strings.widgetUpdated(words.context, words.day(record.generatedAt, now, zone)) else null,
        )
    }

    /**
     * What airs next from the list, still to come: the record's own word for it, or the soonest of
     * the list's broadcasts it holds once that has aired.
     */
    private fun next(record: Record, now: Instant): Record.Next? =
        record.nextAiring?.takeIf { it.airsAt.isAfter(now) }
            ?: record.airing.filter { it.onList && it.airsAt.isAfter(now) }.minByOrNull { it.airsAt }
                ?.let { Record.Next(it.id, it.title, it.episode, it.airsAt) }

    private fun show(context: Context, row: Record.Continuing): ContinueView.Show {
        val begun = row.resume ?: ((row.fraction ?: 0.0) > 0.0)
        val episode = listOfNotNull(Strings.episodeNumberShort(context, row.episode.toInt()), row.mark).joinToString(" · ") // i18n-skip
        val line = when {
            !begun -> Strings.widgetLineStart(context, episode)
            row.minutesLeft != null -> Strings.widgetLineLeft(context, episode, row.minutesLeft!!.toInt())
            row.episodes != null -> listOfNotNull(Strings.widgetLineOf(context, row.episode.toInt(), row.episodes!!.toInt()), row.mark).joinToString(" · ") // i18n-skip
            else -> episode
        }
        return ContinueView.Show(
            id = row.id,
            title = row.title,
            episode = row.episode,
            line = line,
            mark = row.mark,
            progress = if (begun) (row.fraction ?: 0.0).toFloat().coerceIn(0f, 1f) else null,
            wideUrl = row.bannerUrl ?: row.coverUrl,
            posterUrl = row.coverUrl,
            plate = row.coverColor?.let(::argb),
        )
    }

    fun airing(record: Record?, now: Instant, zone: ZoneId, words: Words): AiringView {
        record ?: return AiringView.NoSchedule
        val today = LocalDate.ofInstant(now, zone)
        val start = today.atStartOfDay(zone).toInstant()
        val end = today.plusDays(1).atStartOfDay(zone).toInstant()
        val rows = record.airing.filter { !it.airsAt.isBefore(start) && it.airsAt.isBefore(end) }.sortedBy { it.airsAt }
        if (rows.isEmpty()) {
            // The record holds the days from the one it was made on: past them it says nothing of
            // today, and within them nothing in it today means nothing airs.
            val made = LocalDate.ofInstant(record.generatedAt, zone)
            val covered = !today.isBefore(made) && today.isBefore(made.plusDays(record.days.toLong()))
            return if (covered) AiringView.NothingToday(next(record, now)) else AiringView.NoSchedule
        }
        val marks = record.signedIn ?: rows.any { it.onList }
        return AiringView.Today(
            weekday = today.dayOfWeek.getDisplayName(TextStyle.FULL, words.locale),
            rows = rows.map { row ->
                val aired = !row.airsAt.isAfter(now)
                AiringView.Row(
                    id = row.id,
                    time = words.time(row.airsAt, zone),
                    title = row.title,
                    episode = row.episode,
                    onList = row.onList && marks,
                    aired = aired,
                    out = aired && row.onList && marks && row.watched == false,
                    watched = aired && row.watched == true,
                    translation = row.translation,
                )
            },
            marks = marks,
        )
    }

    /** `#5d9be4` as opaque ARGB, or `null` when it is not a colour. */
    internal fun argb(hex: String): Long? =
        hex.removePrefix("#").takeIf { it.length == 6 }?.toLongOrNull(16)?.let { 0xFF000000L or it }

    /**
     * The [count] rows a layout has room for: the ones still to come, first, and at [withOut] the
     * one latest out and unwatched before them; the room left goes to the latest that aired.
     */
    fun pick(rows: List<AiringView.Row>, count: Int, withOut: Boolean): List<AiringView.Row> {
        val upcoming = rows.filter { !it.aired }
        val out = if (withOut) rows.lastOrNull { it.out } else null
        val chosen = (listOfNotNull(out) + upcoming).take(count).toMutableSet()
        val room = count - chosen.size
        if (room > 0) chosen += rows.filter { it.aired && it !in chosen }.takeLast(room)
        return rows.filter { it in chosen }
    }
}
