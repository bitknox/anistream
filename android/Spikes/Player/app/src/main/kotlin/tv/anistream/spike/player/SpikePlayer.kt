package tv.anistream.spike.player

import android.content.Context
import android.os.Process
import android.os.SystemClock
import android.util.Log
import androidx.media3.common.util.UnstableApi
import org.json.JSONObject
import org.json.JSONTokener
import tv.anistream.player.MpvClient
import tv.anistream.player.MpvNative
import tv.anistream.player.MpvPlayer
import tv.anistream.player.MpvSetup
import tv.anistream.player.Shaders
import java.io.File

/**
 * The spike's one mpv, kept for the life of the process rather than the activity's, so a
 * rotation or a fold that recreates the activity only hands mpv a new surface (AS9). The media
 * session's player wraps the same mpv.
 */
@UnstableApi
object SpikePlayer {
    const val TAG = "spike"

    lateinit var mpv: MpvClient
        private set
    lateinit var player: MpvPlayer
        private set
    lateinit var shaderDir: File
        private set
    var vo = "gpu-next"
        private set
    var url: String? = null
        private set
    var look = Shaders.Look.Off
    var pinnedChain: List<String>? = null
    var display = Shaders.Display(0, 0)
    var started = false
        private set
    private var attached = false
    private var pending: String? = null
    private var loads = 0
    private var firstFrameAt = 0L
    private var loadAt = 0L

    /** Starts mpv once. Later calls change nothing: options only apply at start. */
    fun start(context: Context, hwdec: String, vo: String, gpuContext: String, extra: List<Pair<String, String>>) {
        if (started) return
        started = true
        this.vo = vo
        MpvNative.setAppContext(context.applicationContext)
        shaderDir = Shaders.install(context)
        mpv = MpvClient("player")
        player = MpvPlayer(mpv) { url?.substringAfterLast('/') ?: "Player spike" }
        mpv.listeners += ::onEvent
        mpv.listeners += player::onMpvEvent
        val options = MpvSetup.baseOptions(context) + listOf(
            // Set when the surface arrives; until then mpv has nothing to draw into.
            "vo" to "null",
            "force-window" to "no",
            "gpu-context" to gpuContext,
            "opengl-es" to "yes",
            "hwdec" to hwdec,
            "hwdec-codecs" to "h264,hevc,mpeg4,mpeg2video,vp8,vp9,av1",
            "ao" to "audiotrack,opensles",
            "audio-set-media-role" to "yes",
            // HDR is tone-mapped to SDR until the phone is checked (AS4, the plan's call 2).
            "target-colorspace-hint" to "no",
        ) + extra
        mpv.start(
            options,
            MpvSetup.reportedProperties + listOf("hwdec-current", "video-out-params", "estimated-vf-fps"),
        )
    }

    /**
     * Loads [url] once there is a surface. Opening a file before the VO has one wedges mpv with
     * MediaCodec ("Both surface and native_window are NULL"); mpv-android's BaseMPVView waits for
     * surfaceCreated for the same reason.
     */
    fun load(url: String) {
        this.url = url
        if (!attached) {
            pending = url
            return
        }
        loads++
        loadAt = SystemClock.elapsedRealtime()
        firstFrameAt = 0
        mpv.command("loadfile", url, "replace")
    }

    fun attach(surface: android.view.Surface) {
        mpv.attach(surface, vo)
        attached = true
        pending?.let { pending = null; load(it) }
    }

    fun detach() {
        attached = false
        mpv.detach()
    }

    private fun onEvent(event: tv.anistream.player.MpvEvent) {
        when (event.id) {
            MpvNative.Event.START_FILE -> Log.i(TAG, "SPIKE_START_FILE loads=$loads")
            MpvNative.Event.FILE_LOADED -> Log.i(TAG, "SPIKE_FILE_LOADED")
            MpvNative.Event.END_FILE -> Log.i(TAG, "SPIKE_END_FILE ${event.json}")
            MpvNative.Event.PLAYBACK_RESTART -> if (firstFrameAt == 0L) {
                firstFrameAt = SystemClock.elapsedRealtime()
                Log.i(TAG, "SPIKE_FIRST_FRAME ${firstFrameAt - loadAt} ms")
            }
            MpvNative.Event.PROPERTY_CHANGE -> if (event.name == "video-params") applyShaders()
        }
    }

    /** The chain `choose()` gives for the source and display, as `glsl-shaders`. */
    fun applyShaders() {
        val params = mpv.properties["video-params"]?.let { runCatching { JSONObject(it) }.getOrNull() }
        val chain = pinnedChain?.let { Shaders.Chain(it, "pinned", null) } ?: run {
            if (params == null) return
            val source = Shaders.Source(
                params.optInt("dw", params.optInt("w")),
                params.optInt("dh", params.optInt("h")),
                Shaders.hdrOf(params.optString("gamma"), params.optString("colormatrix"), false),
            )
            Shaders.choose(look, source, display)
        }
        Log.i(TAG, "SPIKE_SHADERS look=${look.name} chain=${chain.label} files=${chain.files}")
        // An empty string would be a list of one empty path.
        if (chain.files.isEmpty()) mpv.command("change-list", "glsl-shaders", "clr", "")
        else mpv.set("glsl-shaders", Shaders.paths(shaderDir, chain))
    }

    /** What the spike measures, as one JSON line in logcat: `SPIKE_REPORT {…}`. */
    fun report(tag: String, cpu: Pair<Long, Long>? = null): JSONObject {
        val report = JSONObject()
        report.put("tag", tag)
        for (name in listOf(
            "hwdec-current", "video-params", "video-out-params", "current-vo", "current-gpu-context",
            "estimated-vf-fps", "container-fps", "display-fps", "frame-drop-count",
            "decoder-frame-drop-count", "vo-delayed-frame-count", "time-pos", "duration", "pause",
            "glsl-shaders", "current-tracks/sub", "sub-text", "vo-passes", "android-surface-size",
            "osd-dimensions",
        )) {
            val json = runCatching { mpv.get(name) }.getOrNull() ?: continue
            report.put(name, runCatching { JSONTokener(json).nextValue() }.getOrDefault(json))
        }
        report.put("loads", loads)
        report.put("first-frame-ms", if (firstFrameAt > 0) firstFrameAt - loadAt else -1)
        report.put("rss-kb", rssKb())
        cpu?.let { (cpuMs, wallMs) -> report.put("cpu-percent", 100.0 * cpuMs / wallMs) }
        return report
    }

    fun rssKb(): Long = File("/proc/self/status").readLines()
        .firstOrNull { it.startsWith("VmRSS:") }?.filter(Char::isDigit)?.toLongOrNull() ?: -1

    fun cpuMs(): Long = Process.getElapsedCpuTime()
}
