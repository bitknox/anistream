// Scrubbing: the frame under the finger from a second mpv, as the spike measured it (AS8).

package tv.anistream.player

import android.content.Context
import android.graphics.Bitmap
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/**
 * A second mpv with no surface, no audio and no subtitles, which shows the frame at the scrubbing
 * finger: software decoding scaled to [width] pixels, keyframe seeks, and mpv's own
 * `screenshot-raw` of the frame it lands on. It decodes in software on purpose, leaving the
 * phone's few MediaCodec instances to the player. Opened on the first scrub of a file, kept for
 * the next, closed with the file; one seek at a time, the newest waiting while mpv works.
 */
class SeekPreview(private val context: Context, private val scope: CoroutineScope, private val width: Int = 320) {
    /** The frame at the last place asked for, once there is one. */
    var frame by mutableStateOf<Bitmap?>(null)
        private set

    private var mpv: MpvClient? = null
    private var source: MpvCall.Load? = null
    private var target: Double? = null
    private var busy = false
    private val restarts = Channel<Unit>(Channel.CONFLATED)

    /** Show the frame at [seconds] of [of]. */
    fun show(seconds: Double, of: MpvCall.Load) {
        if (mpv == null || source != of) open(of, seconds)
        target = seconds
        pump()
    }

    private fun open(of: MpvCall.Load, seconds: Double) {
        close()
        val mpv = MpvClient("preview")
        mpv.listener = { event -> if (event.id == MpvNative.Event.PLAYBACK_RESTART) restarts.trySend(Unit) }
        this.mpv = mpv
        source = of
        busy = true
        scope.launch {
            try {
                mpv.start(MpvSetup.baseOptions(context) + options(), emptyList(), logLevel = "error")
                val arguments = mutableMapOf<String, MpvValue>("url" to MpvValue.Text(of.url), "flags" to MpvValue.Text("replace")) // i18n-skip
                arguments["options"] = MpvValue.Map((of.options + ("start" to seconds.toString())).mapValues { MpvValue.Text(it.value) })
                mpv.command("loadfile", arguments)
                withTimeoutOrNull(OPEN_MS) { restarts.receive() }
                if (mpv === this@SeekPreview.mpv) mpv.screenshot("video")?.let { frame = it }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.w(PlayerModel.TAG, "the seek preview did not open: $e")
            }
            if (mpv !== this@SeekPreview.mpv) return@launch
            busy = false
            pump()
        }
    }

    /** Seek to the newest place asked for, when mpv is free. */
    private fun pump() {
        val mpv = mpv ?: return
        val seconds = target ?: return
        if (busy) return
        target = null
        busy = true
        scope.launch {
            try {
                while (restarts.tryReceive().isSuccess) Unit
                mpv.command("seek", seconds.toString(), "absolute+keyframes")
                withTimeoutOrNull(SEEK_MS) { restarts.receive() }
                if (mpv === this@SeekPreview.mpv) mpv.screenshot("video")?.let { frame = it }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
            }
            if (mpv !== this@SeekPreview.mpv) return@launch
            busy = false
            pump()
        }
    }

    /** Done with this file, or with previews: the second mpv shuts down. */
    fun close() {
        mpv?.let { gone ->
            gone.listener = null
            scope.launch { withContext(NonCancellable) { gone.shutdown() } }
        }
        mpv = null
        source = null
        target = null
        busy = false
        frame = null
    }

    private fun options(): List<Pair<String, String>> = listOf(
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
        "demuxer-readahead-secs" to "0",
        "demuxer-max-bytes" to "${4 * 1024 * 1024}",
        "demuxer-max-back-bytes" to "0",
        "sws-scaler" to "fast-bilinear",
        "vf" to "scale=w=$width:h=-2",
    )

    private companion object {
        const val OPEN_MS = 10_000L
        const val SEEK_MS = 4_000L
    }
}
