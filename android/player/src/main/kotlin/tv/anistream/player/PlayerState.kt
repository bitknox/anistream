// What the player shows, as values: the engine's PlayerUi kept until it says otherwise, and what
// mpv says of the file. After Apple's PlayerState.swift.

package tv.anistream.player

import tv.anistream.i18n.Strings
import android.content.Context
import java.util.Locale
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.MenuChoices
import tv.anistream.engine.model.MenuEpisode
import tv.anistream.engine.model.MenuSource
import tv.anistream.engine.model.NextEpisode
import tv.anistream.engine.model.NoticeKind
import tv.anistream.store.text
import tv.anistream.engine.model.SkipKind
import tv.anistream.engine.model.TimelineMark

/** The rules the player's cards keep time by. Times are milliseconds unless they say seconds. */
object PlayerTiming {
    /** Up Next's countdown, from where the ending starts, when the engine gives none. */
    const val UP_NEXT_COUNTDOWN = 10.0

    /** Without ending data, Up Next shows this many seconds before the end of anything over two minutes. */
    const val UP_NEXT_WITHOUT_ENDING = 30.0

    /** Episodes rolled on in a row with nobody touching anything before "Still watching?". */
    const val STILL_WATCHING_AFTER = 3

    /** The skip button dims after this long (T4). */
    const val SKIP_DIMS = 8_000L

    /** The undo note stays for this many seconds of playback, and at most [UNDO_PAUSED] of wall time. */
    const val UNDO_PLAYBACK = 6.0
    const val UNDO_PAUSED = 15_000L

    /** The resume note stays for this many seconds of playback. */
    const val RESUME_PLAYBACK = 6.0

    /** Waiting for data mid-episode shows the buffering line along the video's top edge from here. */
    const val BUFFERING_AFTER = 1_000L

    /** The starting screen admits the wait from here. */
    const val STARTING_WAITS = 3_000L

    /** The controls hide after this long without input (T4). */
    const val CONTROLS_HIDE = 3_000L

    /** An alert on the video stays this long. */
    const val ALERT_STAYS = 4_000L

    /** A start that has shown nothing this long offers another source, and to stop. */
    const val STARTING_OFFERS = 20_000L

    /** How long a player closed before its first frame waits for the playback's verdict. */
    const val VERDICT_WAIT = 3_000L

    /** How long the first file waits for a frame before mpv is opened again without hardware decoding. */
    const val FIRST_FRAME_WATCHDOG = 12_000L

    /** How long a Load waits for the surface before it opens anyway, without hardware decoding. */
    const val SURFACE_WAIT = 4_000L
}

/** The playback Now Playing shows, as the engine staged it. */
data class StagedPlayback(val id: AnilistId, val title: String, val episode: String, val episodeTitle: String?)

/** The starting screen: what is being found or opened, since when. */
data class StartingScreen(
    val playback: ULong,
    val id: AnilistId?,
    val title: String,
    val episode: String,
    val episodeTitle: String?,
    val coverUrl: String?,
    val bannerUrl: String?,
    val since: Long,
) {
    /** "Starting episode 7", and from 3 s on what the wait is on, "Finding a source · 4 s", or the engine's note. */
    fun line(context: Context, now: Long, note: String?): String {
        val waited = now - since
        if (waited < PlayerTiming.STARTING_WAITS) return Strings.playerStartingText(context, episode)
        val what = note?.takeIf { it.isNotEmpty() }?.replaceFirstChar { it.uppercase() } ?: Strings.playerFindingSourcePlain(context)
        return Strings.playerWaitNote(context, what, (waited / 1000).toInt())
    }

    /** Whether it has waited long enough to offer another source, and to stop. */
    fun offersChoice(now: Long): Boolean = now - since >= PlayerTiming.STARTING_OFFERS
}

/**
 * A playback that never played, for "Couldn't play episode 7", and why when the engine said:
 * "No sources configured", the first clause of its alert in plain words.
 */
data class PlaybackFailure(
    val id: AnilistId?,
    val episode: String,
    val reason: String? = null,
    /** What the engine said, as a message, so the reason reads in the person's language. */
    val msg: tv.anistream.engine.model.Msg? = null,
) {
    /** The reason in the person's language when the engine sent a message, else as it came. */
    fun reasonText(context: Context): String? =
        msg?.let { reason(it.text(context)) } ?: reason

    companion object {
        /**
         * The engine's alert as a failure says it: its first clause, capitalised, with no address
         * in it. "no sources configured: see the Providers screen" is "No sources configured", and
         * "mpv: loading failed" is "Loading failed", as a lone word before the colon only names who
         * said it.
         */
        fun reason(alert: String): String? {
            var text = alert.trim()
            val named = text.substringBefore(": ", "")
            if (named.isNotEmpty() && ' ' !in named && text.length > named.length + 2) text = text.substringAfter(": ")
            // The clause before the address is cut, as an address would take a colon after it as its own.
            val clause = text.substringBefore(": ").substringBefore(" \u2014 ").trim().trimEnd('.').trim()
            return MpvLog.redacting(clause).takeIf { it.isNotEmpty() }?.replaceFirstChar { it.uppercase() }
        }
    }
}

/** The episode on screen, for the header and the media session. */
data class EpisodeOnScreen(
    val number: String,
    val label: String,
    val title: String?,
    val show: String,
    val translation: Translation,
) {
    /** "Episode 7 · The Lantern Road", or the label alone. */
    val line: String get() = if (title.isNullOrEmpty()) label else "$label · $title"
}

/** What the menus list, whole. */
data class PlayerMenus(
    val episodes: List<MenuEpisode>,
    val sources: List<MenuSource>,
    val translation: Translation,
    val upscaling: MenuChoices,
    val toneMapping: MenuChoices?,
    val nightMode: Boolean,
)

data class SkipOffer(val kind: SkipKind, val to: Double, val since: Long) {
    fun label(context: Context): String = if (kind == SkipKind.OPENING) Strings.playerSkipOpening(context) else Strings.playerSkipEnding(context)
}

data class UndoOffer(
    val kind: SkipKind,
    val undoTo: Double,
    /** Where the playhead was when the note appeared. */
    val from: Double?,
    val since: Long,
) {
    /** "Skipped opening", then Undo. */
    fun label(context: Context): String = if (kind == SkipKind.OPENING) Strings.playerSkippedOpening(context) else Strings.playerSkippedEnding(context)
}

data class UpNextCard(
    val next: NextEpisode,
    val auto: Boolean,
    val endingAt: Double?,
    /** How long the countdown runs, as the engine says: `0` is none, and the card waits. */
    val countdown: Double = PlayerTiming.UP_NEXT_COUNTDOWN,
    /** How many episodes in a row with no input from the viewer play before "Still watching?". */
    val stillWatchingAfter: Int = PlayerTiming.STILL_WATCHING_AFTER,
    /** The viewer cancelled it, for the rest of the episode. */
    val cancelled: Boolean = false,
    /** PlayNext has been sent, once. */
    val played: Boolean = false,
    /**
     * Where the countdown runs from since a seek landed in or past it: its full length again from
     * there, where the ending alone would have it already over.
     */
    val restartedAt: Double? = null,
    /** The viewer sought while it was up: what it plays next does not count toward "Still watching?". */
    val sought: Boolean = false,
) {
    /** Where the card shows and its countdown starts: the ending, or near the end of the file. */
    fun showsAt(duration: Double?): Double? {
        endingAt?.let { return it }
        if (duration == null || duration <= 0) return null
        return if (duration > 120) {
            duration - PlayerTiming.UP_NEXT_WITHOUT_ENDING
        } else {
            maxOf(0.0, duration - if (countdown > 0) countdown else PlayerTiming.UP_NEXT_COUNTDOWN)
        }
    }

    /** Seconds left on the countdown at [position], while it runs: never with no countdown. */
    fun remaining(position: Double, duration: Double?): Double? {
        if (!auto || cancelled || countdown <= 0) return null
        val shows = showsAt(duration) ?: return null
        val start = maxOf(shows, restartedAt ?: shows)
        if (position < start) return null
        return maxOf(0.0, countdown - (position - start))
    }
}

data class StallCard(val seconds: Long, val since: Long) {
    /** Seconds held: what the engine said, counting up from then. */
    fun held(now: Long): Long = seconds + (now - since) / 1000
}

data class ResumeNote(val position: Double) {
    /** "Resumed at 12:04". */
    fun label(context: Context): String = Strings.playerResumedAt(context, PlayerClock.label(position))
}

data class PlayerAlert(val text: String, val kind: NoticeKind, val since: Long)

/** A chapter of the file, as mpv lists it. */
data class Chapter(val title: String, val time: Double) {
    /** The chapter names the engine reads as an opening or an ending, as its own rule does. */
    val segment: SeekSegment.Kind
        get() {
            val name = title.trim().lowercase()
            return when {
                name in setOf("op", "opening", "intro") || name.startsWith("ncop") -> SeekSegment.Kind.Opening
                name in setOf("ed", "ending", "outro") || name.startsWith("nced") -> SeekSegment.Kind.Ending
                else -> SeekSegment.Kind.Plain
            }
        }

    companion object {
        /** A `chapter-list` entry. */
        fun of(entry: MpvValue) = Chapter(entry["title"].string.orEmpty(), entry["time"].double ?: 0.0)
    }
}

/** A track of the file, as mpv lists it. */
data class Track(
    val kind: Kind,
    val id: Long,
    val language: String?,
    val title: String?,
    val selected: Boolean,
    val external: Boolean,
) {
    enum class Kind(val mpv: String) { Video("video"), Audio("audio"), Subtitle("sub") }

    /** "English · Signs & Songs": the language by name, its title after it. */
    fun name(context: Context): String {
            val language = language?.let { Locale.forLanguageTag(it).getDisplayLanguage(Locale.getDefault()).takeIf { name -> name.isNotEmpty() && name != it } ?: it }
            return when {
                language != null && title != null -> "$language · $title" // i18n-skip
                language != null -> language
                title != null -> title
                else -> Strings.playerTrackN(context, id.toInt())
            }
        }

    companion object {
        /** A `track-list` entry, or `null` for one that is not a track. */
        fun of(entry: MpvValue): Track? {
            val kind = Kind.entries.firstOrNull { it.mpv == entry["type"].string } ?: return null
            val id = entry["id"].long ?: return null
            return Track(
                kind, id, entry["lang"].string, entry["title"].string,
                entry["selected"].bool ?: false, entry["external"].bool ?: false,
            )
        }
    }
}

/** A stretch of the seek line: openings and endings are drawn in haze. */
data class SeekSegment(val kind: Kind, val start: Double, val end: Double) {
    enum class Kind { Plain, Opening, Ending }
}

/** The seek line's stretches from the engine's marks: openings and endings, everything else plain. */
fun markedSegments(marks: List<TimelineMark>, duration: Double?): List<SeekSegment> {
    if (duration == null || duration <= 0) return emptyList()
    val segments = mutableListOf<SeekSegment>()
    var at = 0.0
    for (mark in marks.sortedBy { it.start }) {
        val start = maxOf(mark.start, at)
        val end = minOf(mark.end, duration)
        if (end <= start) continue
        if (start > at) segments += SeekSegment(SeekSegment.Kind.Plain, at, start)
        val kind = if (mark.kind == SkipKind.OPENING) SeekSegment.Kind.Opening else SeekSegment.Kind.Ending
        segments += SeekSegment(kind, start, end)
        at = end
    }
    if (at < duration) segments += SeekSegment(SeekSegment.Kind.Plain, at, duration)
    return segments
}

/** The seek line's stretches: openings and endings from the chapters, everything else plain. */
fun seekSegments(chapters: List<Chapter>, duration: Double?): List<SeekSegment> {
    if (duration == null || duration <= 0) return emptyList()
    val sorted = chapters.filter { it.time < duration }.sortedBy { it.time }
    val segments = mutableListOf<SeekSegment>()
    var at = 0.0
    sorted.forEachIndexed { index, chapter ->
        val kind = chapter.segment
        if (kind == SeekSegment.Kind.Plain) return@forEachIndexed
        val end = if (index + 1 < sorted.size) sorted[index + 1].time else duration
        if (chapter.time > at) segments += SeekSegment(SeekSegment.Kind.Plain, at, chapter.time)
        segments += SeekSegment(kind, chapter.time, end)
        at = end
    }
    if (at < duration) segments += SeekSegment(SeekSegment.Kind.Plain, at, duration)
    return segments
}

/** Times as the player writes them. */
object PlayerClock {
    /** "12:04", "1:02:09". */
    fun label(seconds: Double): String {
        val total = if (seconds.isFinite()) maxOf(0.0, seconds).toLong() else 0L
        val hours = total / 3600
        val minutes = total / 60 % 60
        val secs = total % 60
        return if (hours > 0) {
            String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, secs)
        } else {
            String.format(Locale.ROOT, "%d:%02d", minutes, secs)
        }
    }

    /** "−2:29". */
    fun remaining(seconds: Double): String = "−" + label(seconds)
}
