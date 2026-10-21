package tv.anistream.player

import android.content.Context
import android.graphics.Bitmap
import android.os.SystemClock
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * Seek-bar previews from a second mpv, as the Apple app makes them: no surface (`vo=null`), no
 * audio or subtitles, software decoding scaled down to [width], keyframe seeks, and mpv's own
 * `screenshot-raw video` of the frame it lands on. One preview at a time; call off the main
 * thread.
 */
class SeekPreviews(context: Context, private val width: Int = 320) {
    private val mpv = MpvClient("preview")
    private val events = LinkedBlockingQueue<MpvEvent>()

    init {
        mpv.listeners += { events.offer(it) }
        val options = MpvSetup.baseOptions(context) + listOf(
            "vo" to "null",
            "ao" to "null",
            "aid" to "no",
            "sid" to "no",
            "pause" to "yes",
            "keep-open" to "always",
            "hwdec" to "no",
            "vd-lavc-threads" to "2",
            "vd-lavc-fast" to "yes",
            "vd-lavc-skiploopfilter" to "all",
            "hr-seek" to "no",
            "cache" to "no",
            "demuxer-readahead-secs" to "0",
            "demuxer-max-bytes" to "${4 * 1024 * 1024}",
            "demuxer-max-back-bytes" to "0",
            "sws-scaler" to "fast-bilinear",
            "vf" to "scale=w=$width:h=-2",
        )
        mpv.start(options, emptyList(), logLevel = "warn")
    }

    /** Opens [url]; true once mpv has loaded it. */
    fun open(url: String, timeoutMs: Long = 10_000): Boolean {
        events.clear()
        mpv.command("loadfile", url, "replace")
        return awaitEvent(MpvNative.Event.FILE_LOADED, timeoutMs) &&
            awaitEvent(MpvNative.Event.PLAYBACK_RESTART, timeoutMs)
    }

    /** The frame at the keyframe nearest [seconds], and how long it took. */
    fun frame(seconds: Double, timeoutMs: Long = 5_000): Pair<Bitmap?, Long> {
        val start = SystemClock.elapsedRealtime()
        events.clear()
        mpv.command("seek", seconds.toString(), "absolute+keyframes")
        val landed = awaitEvent(MpvNative.Event.PLAYBACK_RESTART, timeoutMs)
        val bitmap = if (landed) mpv.screenshot("video") else null
        return bitmap to SystemClock.elapsedRealtime() - start
    }

    fun close() = mpv.shutdown()

    private fun awaitEvent(id: Int, timeoutMs: Long): Boolean {
        val deadline = SystemClock.elapsedRealtime() + timeoutMs
        while (true) {
            val left = deadline - SystemClock.elapsedRealtime()
            if (left <= 0) return false
            val event = events.poll(left, TimeUnit.MILLISECONDS) ?: return false
            if (event.id == id) return true
        }
    }
}
