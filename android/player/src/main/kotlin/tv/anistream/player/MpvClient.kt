// One mpv, driven through the JNI shim on a thread of its own.

package tv.anistream.player

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.RejectedExecutionException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/** One of mpv's events. [json] is a property's value as mpv_node JSON, or the event's data. */
data class MpvEvent(val id: Int, val name: String?, val json: String?) {
    /** The value, read. */
    val value: MpvValue get() = json?.let(MpvValue::parse) ?: MpvValue.None

    companion object {
        fun property(name: String, value: MpvValue) = MpvEvent(MpvNative.Event.PROPERTY_CHANGE, name, value.json)
        fun of(id: Int) = MpvEvent(id, null, null)
        fun endFile(reason: String) = MpvEvent(MpvNative.Event.END_FILE, null, """{"reason":"$reason"}""")
    }
}

/** mpv said no: its error code, and what was asked. */
class MpvException(val code: Int, call: String) : Exception("$call: mpv error $code")

/**
 * Owns one mpv handle.
 *
 * - Every call runs on the client's own thread and is awaited, so nothing on the main thread waits
 *   on mpv: a screenshot, a VO reconfigure or a shader compile can take a while, and a hardware
 *   decoder that stalls wedges mpv's core (the spike's AS5).
 * - Events come from the shim's event thread and are posted to the main thread for [listener].
 * - [shutdown] frees the handle; calls after it do nothing.
 */
class MpvClient(private val name: String) {
    private val executor = Executors.newSingleThreadExecutor { r -> Thread(r, "mpv-$name") }
    private val thread = executor.asCoroutineDispatcher()
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var handle = 0L
    private var nextObserver = 1L

    /** Each event, on the main thread. Set before [start], so nothing is missed. */
    @Volatile var listener: ((MpvEvent) -> Unit)? = null

    /** Creates and initialises mpv with [options], set in order, observing [observed]. */
    suspend fun start(options: List<Pair<String, String>>, observed: List<String>, logLevel: String = "warn") = call {
        check(handle == 0L) { "mpv $name has already started" }
        val h = MpvNative.create { id, prop, data ->
            val event = MpvEvent(id, prop, data?.toString(Charsets.UTF_8))
            main.post { listener?.invoke(event) }
        }
        check(h != 0L) { "mpv_create failed" }
        for ((key, value) in options) MpvNative.setOption(h, key, value)
        val r = MpvNative.initialize(h, logLevel)
        if (r < 0) {
            MpvNative.destroy(h)
            throw MpvException(r, "mpv_initialize")
        }
        for (property in observed) MpvNative.observe(h, property, nextObserver++)
        handle = h
    }

    suspend fun command(vararg args: String) = call {
        live()?.let { checked(MpvNative.command(it, arrayOf(*args)), args.joinToString(" ")) }
    }

    /** A command with named arguments: `loadfile` with its `options` map, `sub-add` with a title. */
    suspend fun command(name: String, arguments: Map<String, MpvValue>) = call {
        val node = MpvValue.Map(linkedMapOf<String, MpvValue>("name" to MpvValue.Text(name)) + arguments)
        live()?.let { checked(MpvNative.commandNode(it, node.json.toByteArray()), name) }
    }

    /** Sets a property: a scalar as mpv's string, a list or a map as a node. */
    suspend fun set(property: String, value: MpvValue) = call {
        val h = live() ?: return@call
        val r = when (value) {
            is MpvValue.List, is MpvValue.Map -> MpvNative.setPropertyNode(h, property, value.json.toByteArray())
            MpvValue.None -> return@call
            is MpvValue.Flag -> MpvNative.setProperty(h, property, if (value.value) "yes" else "no")
            is MpvValue.Text -> MpvNative.setProperty(h, property, value.value)
            is MpvValue.Int64 -> MpvNative.setProperty(h, property, value.value.toString())
            is MpvValue.Real -> MpvNative.setProperty(h, property, value.value.toString())
        }
        checked(r, "set $property")
    }

    /**
     * Sets [property] if mpv's thread takes it within [timeoutMs], and says whether it did. The
     * caller is let go at the time even when the thread is stuck behind a stalled decoder, which a
     * plain call, awaited to its end, would never be.
     */
    suspend fun setWithin(property: String, value: String, timeoutMs: Long): Boolean {
        val done = CompletableDeferred<Boolean>()
        try {
            executor.execute {
                val h = handle
                done.complete(h != 0L && MpvNative.setProperty(h, property, value) >= 0)
            }
        } catch (_: RejectedExecutionException) {
            return false
        }
        return withTimeoutOrNull(timeoutMs) { done.await() } ?: false
    }

    /** A property as it stands, or [MpvValue.None] when mpv has none. */
    suspend fun get(property: String): MpvValue = call {
        val h = live() ?: return@call MpvValue.None
        MpvNative.getProperty(h, property)?.toString(Charsets.UTF_8)?.let(MpvValue::parse) ?: MpvValue.None
    }

    /**
     * mpv-android's BaseMPVView.surfaceCreated: the surface as `wid`, then the VO back on. mpv
     * keeps playing while there is no surface, with `vo=null`. Not awaited: the surface callback
     * returns at once.
     */
    fun attach(surface: Surface, vo: String) {
        executor.execute {
            val h = handle.takeIf { it != 0L } ?: return@execute
            MpvNative.attachSurface(h, surface)
            MpvNative.setOption(h, "force-window", "yes")
            MpvNative.setProperty(h, "vo", vo)
        }
    }

    /** surfaceChanged: mpv's android context cannot ask the surface its size, so it is told each time. */
    fun resize(width: Int, height: Int) {
        executor.execute {
            val h = handle.takeIf { it != 0L } ?: return@execute
            MpvNative.setProperty(h, "android-surface-size", "${width}x$height")
        }
    }

    /**
     * surfaceDestroyed: the VO off and the surface let go, finished before the callback returns,
     * while the surface still lives. Blocks the caller for as long as mpv's thread is busy.
     */
    fun detach() {
        runCatching {
            executor.submit {
                val h = handle.takeIf { it != 0L } ?: return@submit
                MpvNative.setProperty(h, "vo", "null")
                MpvNative.setProperty(h, "force-window", "no")
                MpvNative.detachSurface(h)
            }.get(5, TimeUnit.SECONDS)
        }.onFailure { Log.w(TAG, "mpv $name: the surface went before mpv let it go: $it") }
    }

    /** mpv's own render of the video (`video`), as a Bitmap: a seek preview's frame. */
    suspend fun screenshot(flags: String = "video"): Bitmap? {
        val raw = call { live()?.let { MpvNative.screenshot(it, flags) } } ?: return null
        val header = ByteBuffer.wrap(raw, 0, 8).order(ByteOrder.LITTLE_ENDIAN)
        val width = header.int
        val height = header.int
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(raw, 8, raw.size - 8))
        return bitmap
    }

    /** Quits mpv and frees the handle. Calls from here on do nothing. Safe to call twice. */
    suspend fun shutdown() {
        runCatching {
            call {
                val h = handle
                handle = 0L
                if (h != 0L) MpvNative.destroy(h)
            }
        }
        listener = null
        executor.shutdown()
    }

    private fun live(): Long? = handle.takeIf { it != 0L }

    private fun checked(r: Int, what: String) {
        if (r < 0) throw MpvException(r, what)
    }

    private suspend fun <T> call(block: () -> T): T = withContext(thread) { block() }

    companion object {
        const val TAG = "mpv"
    }
}
