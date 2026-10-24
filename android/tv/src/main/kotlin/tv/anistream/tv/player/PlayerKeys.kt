// The player's D-pad rules that do not need a screen: the running seek count while the chrome is
// hidden, the scrub head's steps and its stops at the opening's and ending's edges, and the times
// the chrome writes.

package tv.anistream.tv.player

import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToLong
import tv.anistream.player.SeekSegment

/** How long the chrome stays with no key: 5 s, since 3 s is short across a room. */
const val CHROME_HIDE = 5_000L

/** How long after the last Left or Right the one seek goes. */
const val SEEK_SETTLE = 500L

/** How long a skip offer holds Center while the chrome is hidden. */
const val SKIP_FRESH = 8_000L

/**
 * Left and Right with the chrome hidden: each press adds 10 s to a running count, and one seek
 * goes once the presses stop.
 */
class SeekRun(private val step: Double = 10.0) {
    /** The seconds counted so far, back negative. */
    var total = 0.0
        private set

    /** Each press, so the screen restarts its settle wait. */
    var presses = 0
        private set

    fun press(forward: Boolean) {
        total += if (forward) step else -step
        presses += 1
    }

    /** The count to seek by, and the run begins again. */
    fun take(): Double = total.also {
        total = 0.0
        presses = 0
    }

    /** "+30 s", "−20 s". */
    val label: String get() = signedSeconds(total)
}

/** "+30 s" or "−20 s" (a real minus sign), and "0 s". */
fun signedSeconds(seconds: Double): String {
    val whole = seconds.roundToLong()
    return when {
        whole > 0 -> "+$whole s"
        whole < 0 -> "−${abs(whole)} s"
        else -> "0 s"
    }
}

/**
 * The scrub head on the focused seek bar: it moves without seeking, 10 s a press, 30 s once a key
 * has been held 1 s, 60 s after 3 s. Moving over the opening's or the ending's edge, it stops on
 * the edge once, so a few presses land right after the opening.
 */
class ScrubHead(start: Double, private val duration: Double, private val segments: List<SeekSegment>) {
    var at: Double = start.coerceIn(0.0, duration)
        private set

    private var stoppedOn: Double? = null

    /** Move one press, the key held for [heldMillis] so far (0 for a fresh press). */
    fun move(forward: Boolean, heldMillis: Long) {
        val step = stepFor(heldMillis)
        val target = (if (forward) at + step else at - step).coerceIn(0.0, duration)
        val edge = edges().firstOrNull { edge ->
            edge != stoppedOn && if (forward) edge > at && edge <= target else edge < at && edge >= target
        }
        if (edge != null) {
            at = edge
            stoppedOn = edge
        } else {
            at = target
            stoppedOn = null
        }
    }

    private fun edges(): List<Double> = segments
        .filter { it.kind != SeekSegment.Kind.Plain }
        .flatMap { listOf(it.start, it.end) }
        .filter { it > 0.0 && it < duration }
        .distinct()
        .sorted()

    companion object {
        fun stepFor(heldMillis: Long): Double = when {
            heldMillis >= 3_000 -> 60.0
            heldMillis >= 1_000 -> 30.0
            else -> 10.0
        }
    }
}

/** "10:29", or "1:02:03" past the hour. */
fun clockTime(seconds: Double): String {
    val total = seconds.coerceAtLeast(0.0).toLong()
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val secs = total % 60
    return if (hours > 0) {
        String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, secs)
    } else {
        String.format(Locale.ROOT, "%d:%02d", minutes, secs)
    }
}

/** "−13:31", the time left. */
fun remainingTime(position: Double, duration: Double): String = "−" + clockTime(duration - position)
