package tv.anistream.spike.player

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.ComponentName
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.util.Rational
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.widget.FrameLayout
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import org.json.JSONArray
import org.json.JSONObject
import tv.anistream.player.SeekPreviews
import tv.anistream.player.Shaders
import java.io.File
import java.util.concurrent.Executors

/**
 * The spike's one screen: a SurfaceView mpv draws into, driven by intent extras so scripts can
 * run it with `adb shell am start` (see README.md):
 *
 *   url       a URL, `lavfi`, or a file name in the app's files/media
 *   shaders   off | natural | sharp: the chain `choose()` gives for the source and the display
 *   chain     shader files to pin instead, comma-separated
 *   hwdec, vo, gpu-context, opts ("name=value;…")   mpv's, at the first start only
 *   cmd       mpv commands, `|`-separated, words split on spaces
 *   set       properties, "name=value;…"
 *   seconds   log a SPIKE_REPORT after this many seconds, with the CPU used from second 2
 *   report    log a SPIKE_REPORT now
 *   pip       enter picture in picture
 *   previews  make this many seek previews from a second mpv and log SPIKE_PREVIEWS (AS8)
 *
 * This activity is recreated on rotation and resize, so mpv, which outlives it, gets a new
 * surface each time (AS9). [StableActivity] handles those changes itself instead (AS2).
 */
@UnstableApi
open class SpikeActivity : Activity() {
    private lateinit var surface: SurfaceView
    private var controller: ListenableFuture<MediaController>? = null
    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val extras = intent
        SpikePlayer.start(
            this,
            hwdec = extras.getStringExtra("hwdec") ?: "mediacodec,mediacodec-copy",
            vo = extras.getStringExtra("vo") ?: "gpu-next",
            gpuContext = extras.getStringExtra("gpu-context") ?: "android",
            extra = pairs(extras.getStringExtra("opts")),
        )
        SpikePlayer.display = display()
        surface = SurfaceView(this)
        surface.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                Log.i(SpikePlayer.TAG, "SPIKE_SURFACE created")
                SpikePlayer.attach(holder.surface)
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                Log.i(SpikePlayer.TAG, "SPIKE_SURFACE changed ${width}x$height")
                SpikePlayer.mpv.resize(width, height)
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                Log.i(SpikePlayer.TAG, "SPIKE_SURFACE destroyed")
                SpikePlayer.detach()
            }
        })
        setContentView(FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            addView(surface, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        })
        window.insetsController?.let {
            it.hide(WindowInsets.Type.systemBars())
            it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        setPictureInPictureParams(pipParams())
        Log.i(SpikePlayer.TAG, "SPIKE_ACTIVITY created ${javaClass.simpleName} recreated=${savedInstanceState != null}")
        // A recreated activity keeps playing what the last one played.
        if (savedInstanceState == null) handle(extras)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handle(intent)
    }

    override fun onStart() {
        super.onStart()
        // Binding a controller starts the session's service; Media3 keeps it in the foreground
        // with its notification while mpv plays.
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        controller = MediaController.Builder(this, token).buildAsync()
    }

    override fun onStop() {
        controller?.let { MediaController.releaseFuture(it) }
        controller = null
        super.onStop()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        SpikePlayer.display = display()
        Log.i(SpikePlayer.TAG, "SPIKE_CONFIG ${newConfig.orientation} ${newConfig.screenWidthDp}x${newConfig.screenHeightDp}dp")
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        Log.i(SpikePlayer.TAG, "SPIKE_PIP $isInPictureInPictureMode")
    }

    override fun onDestroy() {
        Log.i(SpikePlayer.TAG, "SPIKE_ACTIVITY destroyed ${javaClass.simpleName} changing=$isChangingConfigurations")
        worker.shutdown()
        super.onDestroy()
    }

    private fun handle(extras: Intent) {
        extras.getStringExtra("shaders")?.let { id ->
            SpikePlayer.look = Shaders.Look.entries.first { it.name.equals(id, ignoreCase = true) }
            SpikePlayer.pinnedChain = null
            SpikePlayer.applyShaders()
        }
        extras.getStringExtra("chain")?.let {
            SpikePlayer.pinnedChain = it.split(',').filter(String::isNotBlank)
            SpikePlayer.applyShaders()
        }
        extras.getStringExtra("url")?.let { SpikePlayer.load(resolve(it)) }
        extras.getStringExtra("cmd")?.split('|')?.forEach { line ->
            SpikePlayer.mpv.command(*line.trim().split(' ').filter(String::isNotEmpty).toTypedArray())
        }
        for ((name, value) in pairs(extras.getStringExtra("set"))) SpikePlayer.mpv.set(name, value)
        if (extras.hasExtra("report")) worker.execute { log("SPIKE_REPORT", SpikePlayer.report("now")) }
        val seconds = extras.getIntExtra("seconds", 0)
        if (seconds > 2) {
            var cpu0 = 0L
            var wall0 = 0L
            main.postDelayed({ cpu0 = SpikePlayer.cpuMs(); wall0 = SystemClock.elapsedRealtime() }, 2_000)
            main.postDelayed({
                val cpu = SpikePlayer.cpuMs() - cpu0 to SystemClock.elapsedRealtime() - wall0
                worker.execute { log("SPIKE_REPORT", SpikePlayer.report("after-$seconds-s", cpu)) }
            }, seconds * 1000L)
        }
        if (extras.getBooleanExtra("pip", false)) enterPictureInPictureMode(pipParams())
        extras.getStringExtra("previews")?.toIntOrNull()?.let { n -> worker.execute { previews(n) } }
    }

    /** AS8: what a second mpv costs, making [count] previews back to back while the player plays. */
    private fun previews(count: Int) {
        val url = SpikePlayer.url ?: return
        val duration = SpikePlayer.mpv.get("duration")?.toDoubleOrNull() ?: return
        val result = JSONObject()
        // The player alone, for five seconds.
        var cpu = SpikePlayer.cpuMs()
        var wall = SystemClock.elapsedRealtime()
        Thread.sleep(5_000)
        result.put("player-cpu-percent", 100.0 * (SpikePlayer.cpuMs() - cpu) / (SystemClock.elapsedRealtime() - wall))
        result.put("rss-kb-before", SpikePlayer.rssKb())
        val openStart = SystemClock.elapsedRealtime()
        val previews = SeekPreviews(this)
        val opened = previews.open(url)
        result.put("open-ms", SystemClock.elapsedRealtime() - openStart)
        result.put("opened", opened)
        result.put("rss-kb-opened", SpikePlayer.rssKb())
        val times = JSONArray()
        cpu = SpikePlayer.cpuMs()
        wall = SystemClock.elapsedRealtime()
        val shots = File(filesDir, "shots").apply { mkdirs() }
        var size = ""
        for (i in 1..count) {
            val (bitmap, ms) = previews.frame(duration * i / (count + 1))
            times.put(ms)
            if (bitmap != null) {
                size = "${bitmap.width}x${bitmap.height}"
                if (i <= 3) File(shots, "preview-$i.png").outputStream().use {
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
                }
                bitmap.recycle()
            }
        }
        val elapsed = SystemClock.elapsedRealtime() - wall
        result.put("with-previews-cpu-percent", 100.0 * (SpikePlayer.cpuMs() - cpu) / elapsed)
        result.put("previews-per-second", count * 1000.0 / elapsed)
        result.put("frame-ms", times)
        result.put("frame-size", size)
        result.put("rss-kb-after-frames", SpikePlayer.rssKb())
        previews.close()
        Thread.sleep(1_000)
        result.put("rss-kb-closed", SpikePlayer.rssKb())
        log("SPIKE_PREVIEWS", result)
    }

    private fun log(tag: String, json: JSONObject) {
        // logcat cuts lines near 4 KB, so long reports go out in pieces that scripts join.
        val text = json.toString()
        text.chunked(3000).forEachIndexed { i, part ->
            Log.i(SpikePlayer.TAG, "$tag ${i + 1}/${(text.length + 2999) / 3000} $part")
        }
    }

    private fun resolve(url: String): String = when {
        url == "lavfi" -> "av://lavfi:testsrc2=size=1920x1080:rate=24"
        "://" in url || url.startsWith("/") -> url
        else -> File(filesDir, "media/$url").path
    }

    private fun display(): Shaders.Display {
        val mode = display?.mode ?: return Shaders.Display(0, 0)
        val (w, h) = mode.physicalWidth to mode.physicalHeight
        return Shaders.Display(maxOf(w, h), minOf(w, h))
    }

    private fun pipParams(): PictureInPictureParams = PictureInPictureParams.Builder()
        .setAspectRatio(Rational(16, 9))
        .setAutoEnterEnabled(true)
        .setSeamlessResizeEnabled(true)
        .build()

    private fun pairs(spec: String?): List<Pair<String, String>> = spec.orEmpty().split(';')
        .filter { '=' in it }
        .map { it.substringBefore('=').trim() to it.substringAfter('=').trim() }
}

/** The same screen, keeping itself through rotation and resize rather than being recreated. */
@UnstableApi
class StableActivity : SpikeActivity()
