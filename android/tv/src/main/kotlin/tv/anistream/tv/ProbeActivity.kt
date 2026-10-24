// A player probe beside the TV app: the player on a stream it is given, with what
// the box makes of it on screen and in logcat, to see what a TV box can play and how.

package tv.anistream.tv

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.widget.FrameLayout
import android.widget.TextView
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import tv.anistream.player.MpvClient
import tv.anistream.player.MpvSetup
import tv.anistream.player.MpvValue

/**
 * Plays the `url` extra:
 * `adb shell am start -n tv.anistream.tv/.ProbeActivity --es url <url or path> [--es vo gpu]`
 *
 * - Centre pauses and plays, left and right seek ten seconds.
 * - Up cycles the upscaler, down the hardware decoding, to see what each costs in dropped frames.
 */
class ProbeActivity : Activity() {
    private val scope = MainScope()
    private val mpv = MpvClient("probe")
    private lateinit var stats: TextView
    /** Done once mpv has started and has the surface, which MediaCodec's zero-copy output needs before a file loads. */
    private val ready = CompletableDeferred<Unit>()
    private var holder: SurfaceHolder? = null
    private var vo = "gpu-next"
    private var scaler = 0
    private var hwdec = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val surface = SurfaceView(this)
        stats = TextView(this).apply {
            setTextColor(Color.WHITE)
            setShadowLayer(4f, 0f, 0f, Color.BLACK)
            textSize = 14f
            typeface = android.graphics.Typeface.MONOSPACE
            setPadding(48, 32, 48, 32)
        }
        setContentView(FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            addView(surface)
            addView(stats, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.START))
        })
        vo = intent.getStringExtra("vo") ?: vo
        // The surface can come before mpv has started, when an attach would do nothing, so mpv
        // takes it once it is up.
        surface.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                this@ProbeActivity.holder = holder
                if (ready.isCompleted) mpv.attach(holder.surface, vo)
            }
            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) = mpv.resize(width, height)
            override fun surfaceDestroyed(holder: SurfaceHolder) {
                this@ProbeActivity.holder = null
                mpv.detach()
            }
        })
        scope.launch {
            mpv.start(MpvSetup.playerOptions(this@ProbeActivity, HWDEC[0]), observed = emptyList())
            holder?.let {
                mpv.attach(it.surface, vo)
                mpv.resize(it.surfaceFrame.width(), it.surfaceFrame.height())
            }
            ready.complete(Unit)
            play(intent)
            while (isActive) {
                report()
                delay(1000)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        scope.launch {
            ready.await()
            play(intent)
        }
    }

    private suspend fun play(intent: Intent) {
        val url = intent.getStringExtra("url")
        if (url == null) {
            stats.text = "No stream. Start with --es url <url>."
            return
        }
        Log.i(TAG, "playing $url")
        mpv.command("loadfile", url)
    }

    /** What is playing and how, on screen and as one logcat line. */
    private suspend fun report() {
        val values = PROPERTIES.map { it to show(mpv.get(it)) }
        val line = values.joinToString("  ") { (name, value) -> "$name=$value" }
        val shown = "abi=${android.os.Build.SUPPORTED_ABIS.first()}  vo=$vo  scale=${SCALERS[scaler]}\n" +
            line.replace("  ", "\n")
        stats.text = shown
        Log.i(TAG, "vo=$vo scale=${SCALERS[scaler]} $line")
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        when (keyCode) {
            KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE ->
                scope.launch { mpv.command("cycle", "pause") }
            KeyEvent.KEYCODE_DPAD_LEFT -> scope.launch { mpv.command("seek", "-10") }
            KeyEvent.KEYCODE_DPAD_RIGHT -> scope.launch { mpv.command("seek", "10") }
            KeyEvent.KEYCODE_DPAD_UP -> {
                scaler = (scaler + 1) % SCALERS.size
                scope.launch { mpv.set("scale", MpvValue.Text(SCALERS[scaler])) }
            }
            KeyEvent.KEYCODE_DPAD_DOWN -> {
                hwdec = (hwdec + 1) % HWDEC.size
                scope.launch { mpv.set("hwdec", MpvValue.Text(HWDEC[hwdec])) }
            }
            else -> return super.onKeyDown(keyCode, event)
        }
        return true
    }

    override fun onDestroy() {
        scope.launch { mpv.shutdown() }.invokeOnCompletion { scope.cancel() }
        super.onDestroy()
    }

    private fun show(value: MpvValue): String = when (value) {
        is MpvValue.Text -> value.value
        is MpvValue.Int64 -> value.value.toString()
        is MpvValue.Real -> "%.2f".format(value.value)
        is MpvValue.Flag -> if (value.value) "yes" else "no"
        MpvValue.None -> "-"
        else -> value.json
    }

    private companion object {
        const val TAG = "anistream-probe"
        val PROPERTIES = listOf(
            "hwdec-current", "video-codec", "width", "height", "container-fps", "estimated-vf-fps",
            "frame-drop-count", "decoder-frame-drop-count", "vo-delayed-frame-count",
            "current-tracks/sub/codec", "avsync", "cache-buffering-state",
        )
        val SCALERS = listOf("bilinear", "spline36", "ewa_lanczossharp")
        val HWDEC = listOf(MpvSetup.defaultHwdec, "mediacodec-copy", "no")
    }
}
