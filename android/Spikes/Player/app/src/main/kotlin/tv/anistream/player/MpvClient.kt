package tv.anistream.player

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** One of mpv's events, on the main thread. [json] is a property's value, as mpv_node JSON. */
data class MpvEvent(val id: Int, val name: String?, val json: String?)

/**
 * Owns one mpv handle.
 *
 * - Every call runs on the client's own thread, so nothing on the main thread waits on mpv: a
 *   screenshot, a VO reconfigure or a shader compile can take a while (Apple's MpvClient found
 *   the same).
 * - Events come from the shim's event thread and are posted to the main thread for [listeners].
 * - [shutdown] frees the handle; calls after it do nothing.
 */
class MpvClient(private val name: String) {
    private val thread: ExecutorService = Executors.newSingleThreadExecutor { r ->
        Thread(r, "mpv-$name")
    }
    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L
    private var nextObserver = 1L
    val listeners = CopyOnWriteArrayList<(MpvEvent) -> Unit>()

    /** The last value of each observed property, as JSON. */
    val properties = java.util.concurrent.ConcurrentHashMap<String, String>()

    /** Creates and initialises mpv with [options] (set in order), observing [observed]. */
    fun start(options: List<Pair<String, String>>, observed: List<String>, logLevel: String = "info") {
        call {
            check(handle == 0L) { "mpv $name has already started" }
            val h = MpvNative.create { id, prop, data ->
                val json = data?.toString(Charsets.UTF_8)
                if (id == MpvNative.Event.PROPERTY_CHANGE && prop != null && json != null) {
                    properties[prop] = json
                }
                val event = MpvEvent(id, prop, json)
                main.post { listeners.forEach { it(event) } }
            }
            check(h != 0L) { "mpv_create failed" }
            for ((key, value) in options) MpvNative.setOption(h, key, value)
            val r = MpvNative.initialize(h, logLevel)
            check(r >= 0) { "mpv_initialize: $r" }
            for (property in observed) MpvNative.observe(h, property, nextObserver++)
            handle = h
        }.get()
    }

    fun command(vararg args: String) {
        call { if (handle != 0L) MpvNative.command(handle, arrayOf(*args)) }
    }

    /** Waits for the command, for callers off the main thread. */
    fun commandSync(vararg args: String): Int =
        call { if (handle != 0L) MpvNative.command(handle, arrayOf(*args)) else -1 }.get()

    fun set(property: String, value: String) {
        call { if (handle != 0L) MpvNative.setProperty(handle, property, value) }
    }

    fun setSync(property: String, value: String): Int =
        call { if (handle != 0L) MpvNative.setProperty(handle, property, value) else -1 }.get()

    /** A property as JSON, or null. Blocks: not for the main thread in the app proper. */
    fun get(property: String): String? =
        call { if (handle != 0L) MpvNative.getProperty(handle, property) else null }
            .get(10, TimeUnit.SECONDS)?.toString(Charsets.UTF_8)

    /**
     * mpv-android's BaseMPVView.surfaceCreated: the surface as `wid`, then the VO back on. mpv
     * keeps playing while there is no surface, with `vo=null`.
     */
    fun attach(surface: Surface, vo: String) {
        call {
            if (handle == 0L) return@call
            MpvNative.attachSurface(handle, surface)
            MpvNative.setOption(handle, "force-window", "yes")
            MpvNative.setProperty(handle, "vo", vo)
        }
    }

    /** surfaceChanged: mpv's android context cannot ask the surface its size. */
    fun resize(width: Int, height: Int) {
        set("android-surface-size", "${width}x$height")
    }

    /** surfaceDestroyed: must finish before the callback returns, while the surface lives. */
    fun detach() {
        call {
            if (handle == 0L) return@call
            MpvNative.setProperty(handle, "vo", "null")
            MpvNative.setProperty(handle, "force-window", "no")
            MpvNative.detachSurface(handle)
        }.get(10, TimeUnit.SECONDS)
    }

    /** mpv's own render of the video (`video`) or of the window (`window`), as a Bitmap. */
    fun screenshot(flags: String = "video"): Bitmap? {
        val raw = call { if (handle != 0L) MpvNative.screenshot(handle, flags) else null }
            .get(10, TimeUnit.SECONDS) ?: return null
        val header = ByteBuffer.wrap(raw, 0, 8).order(ByteOrder.LITTLE_ENDIAN)
        val width = header.int
        val height = header.int
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(raw, 8, raw.size - 8))
        return bitmap
    }

    fun shutdown() {
        call {
            if (handle == 0L) return@call
            MpvNative.destroy(handle)
            handle = 0L
        }.get(15, TimeUnit.SECONDS)
        thread.shutdown()
    }

    private fun <T> call(block: () -> T) = thread.submit<T> {
        try {
            block()
        } catch (e: Exception) {
            Log.e("mpv", "mpv $name: ${e.message}", e)
            throw e
        }
    }
}
