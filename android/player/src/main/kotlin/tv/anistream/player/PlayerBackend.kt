// What the player model drives: one mpv, or a script of one in the tests.

package tv.anistream.player

import android.content.Context
import android.view.Surface
import tv.anistream.engine.model.PlayerReport

/**
 * One mpv for one player. Calls that wait on mpv suspend, off the main thread; the model never
 * waits on mpv from it. The surface calls return at once, but for [detach], which holds the
 * surface's callback until mpv has let the surface go.
 */
interface PlayerBackend {
    /** mpv's events, on the main thread. Set before [begin], so nothing is missed. */
    var listener: ((MpvEvent) -> Unit)?

    /** Start mpv, decoding with [hwdec] and with [options] besides, observing what the engine reads. */
    suspend fun begin(hwdec: String, options: Map<String, String>)

    /** One call; a `Restate` or a `Sample` answers with the reports it reads. */
    suspend fun perform(call: MpvCall): List<PlayerReport>

    /**
     * Decode in software from now on, if mpv answers within [timeoutMs]: false when it does not,
     * its core stuck behind a hardware decoder that stalled.
     */
    suspend fun decodeInSoftware(timeoutMs: Long): Boolean

    /** Draw into [surface] from now on. */
    fun attach(surface: Surface)

    /** The surface is [width] × [height] pixels now. */
    fun resize(width: Int, height: Int)

    /** The surface is going: stop drawing into it before this returns. */
    fun detach()

    /** Shut mpv down. Calls from here on do nothing. */
    suspend fun end()
}

/** The app's mpv, behind [PlayerBackend]: GLES `gpu-next` into the player's SurfaceView. */
class MpvBackend(private val context: Context, private val extra: List<Pair<String, String>> = emptyList()) : PlayerBackend {
    private val mpv = MpvClient("player")

    override var listener: ((MpvEvent) -> Unit)?
        get() = mpv.listener
        set(value) {
            mpv.listener = value
        }

    override suspend fun begin(hwdec: String, options: Map<String, String>) {
        MpvNative.setAppContext(context.applicationContext)
        mpv.start(MpvSetup.playerOptions(context, hwdec) + options.toList() + extra, reportedProperties, logLevel = "warn")
    }

    override suspend fun perform(call: MpvCall): List<PlayerReport> {
        when (call) {
            is MpvCall.Load -> {
                val arguments = mutableMapOf<String, MpvValue>("url" to MpvValue.Text(call.url), "flags" to MpvValue.Text("replace"))
                if (call.options.isNotEmpty()) {
                    arguments["options"] = MpvValue.Map(call.options.mapValues { MpvValue.Text(it.value) })
                }
                mpv.command("loadfile", arguments)
            }
            is MpvCall.AddSubtitle -> {
                val arguments = mutableMapOf<String, MpvValue>("url" to MpvValue.Text(call.url), "flags" to MpvValue.Text("auto"))
                call.title?.let { arguments["title"] = MpvValue.Text(it) }
                call.language?.let { arguments["lang"] = MpvValue.Text(it) }
                mpv.command("sub-add", arguments)
            }
            is MpvCall.Set -> mpv.set(call.property, call.value)
            is MpvCall.Command -> mpv.command(*call.args.toTypedArray())
            is MpvCall.Seek -> mpv.command("seek", call.seconds.toString(), call.mode.mpv)
            is MpvCall.Shaders -> mpv.set("glsl-shaders", MpvValue.List(call.paths.map(MpvValue::Text)))
            is MpvCall.Restate -> return listOf(PlayerReport.Property(call.property, mpv.get(call.property).json))
            // A property mpv has no value for now is left out, and the engine leaves its row out.
            is MpvCall.Sample -> return call.properties.mapNotNull { property ->
                mpv.get(property).takeIf { it != MpvValue.None }?.let { PlayerReport.Property(property, it.json) }
            }
        }
        return emptyList()
    }

    override suspend fun decodeInSoftware(timeoutMs: Long): Boolean = mpv.setWithin("hwdec", "no", timeoutMs)

    override fun attach(surface: Surface) = mpv.attach(surface, "gpu-next")

    override fun resize(width: Int, height: Int) = mpv.resize(width, height)

    override fun detach() = mpv.detach()

    override suspend fun end() = mpv.shutdown()
}
