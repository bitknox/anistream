// What the ongoing download notification says, Android 16's Live Update: names and episode numbers
// only, never episode titles.

package tv.anistream.store

import android.content.Context
import kotlin.math.ceil
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings

/**
 * The download notification for the queue as it stands, as NativeLiveActivity draws it:
 *
 * - one episode: "Stardust Wayfarer" over "Episode 8 · 5 min left", with Pause and Cancel;
 * - several: "Downloading 3 episodes" over "Stardust Wayfarer E8 · 2.7 GB left", with Pause all and
 *   Cancel;
 * - paused: the same with "paused", and Resume (Resume all) in place of Pause.
 *
 * The bar is a segment per episode, as long as the episode is, those finished while the job ran
 * first and full, with a point between each two, and the tracker at the bytes done overall.
 */
data class LiveDownloads(
    val title: String,
    val text: String,
    /** Each episode's share of the bar, in queue order, out of [BAR] together. */
    val segments: List<Int>,
    /** Where the tracker is, out of [BAR]. */
    val progress: Int,
    /** Whether nothing says how far yet: no episode's size is known. */
    val indeterminate: Boolean,
    /** The status bar chip: the time left, "5 min", when the speed says. */
    val chip: String?,
    /** More than one episode left. */
    val several: Boolean,
    /** Everything left is paused: Resume rather than Pause. */
    val paused: Boolean,
) {
    companion object {
        /** The bar's length, which the segments share. */
        const val BAR = 1000

        /**
         * The notification for [rows], or `null` when nothing is left to download. [finished] are
         * the episodes that finished while the job ran, which keep their full segments.
         */
        fun of(context: Context, rows: List<DownloadRow>, rates: Map<Long, Double>, finished: Set<Long> = emptySet()): LiveDownloads? {
            val left = rows.filter { it.state == DownloadState.ACTIVE || it.state == DownloadState.QUEUED || it.state == DownloadState.PAUSED }
            if (left.isEmpty()) return null
            val bar = rows.filter { it in left || (it.state == DownloadState.DONE && it.id in finished) }
            val lead = left.firstOrNull { it.state == DownloadState.ACTIVE } ?: left.first()
            val paused = left.all { it.state == DownloadState.PAUSED }
            val bytesLeft = left.sumOf { (it.total.toLong() - it.downloaded.toLong()).coerceAtLeast(0) }
            val sizes = bar.map { if (it.state == DownloadState.DONE) maxOf(it.total.toLong(), it.downloaded.toLong()) else it.total.toLong() }
            val known = sizes.any { it > 0 }
            val seconds = if (paused) null else secondsLeft(left, rates, bytesLeft)
            val several = left.size > 1
            val title = when {
                !several -> lead.title
                paused -> context.resources.getQuantityString(R.plurals.downloads_live_paused, left.size, left.size)
                else -> context.resources.getQuantityString(R.plurals.downloads_notice_running, left.size, left.size)
            }
            val text = if (several) {
                listOfNotNull(
                    Strings.downloadsLiveLead(context, lead.title, lead.episode.toInt()),
                    if (known) Strings.downloadsLiveLeft(context, Wording.bytes(context, bytesLeft)) else null,
                ).joinToString(" · ") // i18n-skip
            } else if (!paused && seconds != null) {
                Strings.downloadsNoticeEpisodeLeft(context, lead.episode.toInt(), chipOf(context, seconds))
            } else {
                listOfNotNull(
                    Strings.episodeNumber(context, lead.episode.toInt()),
                    when {
                        paused -> Strings.downloadsLivePausedOne(context)
                        lead.state == DownloadState.QUEUED -> Strings.downloadsLiveWaiting(context)
                        else -> bytesOf(context, lead) ?: Strings.downloadsNoticeStarting(context)
                    },
                    if (paused) bytesOf(context, lead) else null,
                ).joinToString(" · ") // i18n-skip
            }
            val (segments, progress) = bar(bar, sizes)
            return LiveDownloads(
                title = title,
                text = text,
                segments = segments,
                progress = progress,
                indeterminate = !known,
                chip = seconds?.let { chipOf(context, it) },
                several = several,
                paused = paused,
            )
        }

        /** "412 MB of 1.1 GB", when the episode's size is known. */
        private fun bytesOf(context: Context, row: DownloadRow): String? =
            if (row.total > 0u) Strings.downloadsLiveProgress(context, Wording.bytes(context, row.downloaded.toLong()), Wording.bytes(context, row.total.toLong())) else null

        /** Seconds until everything left is down, at the speeds the running ones go at. */
        private fun secondsLeft(left: List<DownloadRow>, rates: Map<Long, Double>, bytesLeft: Long): Long? {
            val rate = left.filter { it.state == DownloadState.ACTIVE }.sumOf { rates[it.id] ?: 0.0 }
            if (rate <= 0 || bytesLeft <= 0) return null
            return ceil(bytesLeft / rate).toLong()
        }

        /** The time left as the chip says it: "5 min", "1 h 20 min", "3 h". */
        fun chipOf(context: Context, seconds: Long): String {
            val minutes = maxOf(1L, ceil(seconds / 60.0).toLong())
            if (minutes < 60) return Strings.timeMinutes(context, minutes.toInt())
            val hours = (minutes / 60).toInt()
            val rest = (minutes % 60).toInt()
            return if (rest > 0 && hours < 10) Strings.durationHoursMinutes(context, hours, rest) else Strings.durationHours(context, hours)
        }

        /**
         * The segments, each episode's share of [BAR] by its size (an even share while none is
         * known), and the tracker at the bytes done.
         */
        private fun bar(rows: List<DownloadRow>, sizes: List<Long>): Pair<List<Int>, Int> {
            val total = sizes.sum()
            if (total <= 0) return List(rows.size) { BAR / rows.size }.withRemainder() to 0
            val shares = sizes.map { (it * BAR / total).toInt() }.withRemainder()
            val done = rows.zip(sizes).sumOf { (row, size) ->
                if (row.state == DownloadState.DONE) size else row.downloaded.toLong().coerceAtMost(size)
            }
            return shares to (done * BAR / total).toInt().coerceIn(0, BAR)
        }

        /** The rounding left over goes to the last segment, so they fill the bar. */
        private fun List<Int>.withRemainder(): List<Int> =
            if (isEmpty()) this else dropLast(1) + (last() + (BAR - sum()))
    }
}

/**
 * What one run of the job has seen of the queue: the episodes it saw still to come, and those of
 * them that have finished since, each said once.
 */
class DownloadSession {
    private val waiting = mutableSetOf<Long>()
    private val done = mutableSetOf<Long>()

    /** The episodes finished while the job ran, which keep their full segment on the bar. */
    val finished: Set<Long> get() = done

    /** Take [rows] in: the episodes that finished since the last look, to say so. */
    fun saw(rows: List<DownloadRow>): List<DownloadRow> {
        val now = rows.filter { it.state == DownloadState.DONE && it.id in waiting && it.id !in done }
        done += now.map { it.id }
        waiting += rows.filter { it.state == DownloadState.ACTIVE || it.state == DownloadState.QUEUED || it.state == DownloadState.PAUSED }.map { it.id }
        return now
    }
}
