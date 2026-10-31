// libmpv through the JNI shim, libanistream_mpv.so (src/main/cpp).

package tv.anistream.player

import android.content.Context
import android.view.Surface

/**
 * libmpv through the JNI shim. One handle per mpv; every function but [setAppContext] takes it.
 * Values go in as mpv's strings, or as JSON for a node, and come out as JSON, which is how the
 * engine's `PlayerReport::Property` carries them.
 */
object MpvNative {
    init {
        System.loadLibrary("anistream_mpv")
    }

    /** mpv's event ids (`mpv_event_id`), the ones the player reads. */
    object Event {
        const val SHUTDOWN = 1
        const val LOG_MESSAGE = 2
        const val START_FILE = 6
        const val END_FILE = 7
        const val FILE_LOADED = 8
        const val VIDEO_RECONFIG = 17
        const val AUDIO_RECONFIG = 18
        const val SEEK = 20
        const val PLAYBACK_RESTART = 21
        const val PROPERTY_CHANGE = 22
    }

    /**
     * Called on the handle's event thread. [name] is the property for a property change, the
     * prefix for a log line and the event's name otherwise; [json] is the value, `{"reason": …}`
     * for an end of file, or `{"level": …, "text": …}` for a warning or worse.
     */
    fun interface Listener {
        fun onEvent(event: Int, name: String?, json: ByteArray?)
    }

    /** For FFmpeg's content:// protocol and MediaCodec. Once per process. */
    @JvmStatic external fun setAppContext(context: Context)

    @JvmStatic external fun create(listener: Listener): Long

    @JvmStatic external fun setOption(handle: Long, name: String, value: String): Int

    /** Asks for mpv's log at [logLevel] (to logcat, tag "mpv"), then mpv_initialize. */
    @JvmStatic external fun initialize(handle: Long, logLevel: String): Int

    @JvmStatic external fun command(handle: Long, args: Array<String>): Int

    /** A command with named arguments, as one JSON object with its `name`. */
    @JvmStatic external fun commandNode(handle: Long, json: ByteArray): Int

    /** [commandNode], answering `{"code": …, "result": …}`: mpv's result, as JSON, and its code. */
    @JvmStatic external fun commandNodeResult(handle: Long, json: ByteArray): ByteArray?

    @JvmStatic external fun setProperty(handle: Long, name: String, value: String): Int

    /** A property set to a node, from JSON. */
    @JvmStatic external fun setPropertyNode(handle: Long, name: String, json: ByteArray): Int

    @JvmStatic external fun getProperty(handle: Long, name: String): ByteArray?

    @JvmStatic external fun observe(handle: Long, name: String, id: Long): Int

    @JvmStatic external fun attachSurface(handle: Long, surface: Surface): Int

    /** Only after `vo=null`, so the VO has let the surface go. */
    @JvmStatic external fun detachSurface(handle: Long): Int

    /** `screenshot-raw <flags> rgba`: width and height as two little-endian ints, then the rows. */
    @JvmStatic external fun screenshot(handle: Long, flags: String): ByteArray?

    @JvmStatic external fun destroy(handle: Long)
}
