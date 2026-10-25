// The engine's hosted player protocol in mpv's terms: each PlayerOp as the one call on mpv it is,
// and mpv's events as the PlayerReports the engine reads. After Apple's HostedOps.swift.

package tv.anistream.player

import tv.anistream.engine.model.PlayerEnd
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerReport
import tv.anistream.engine.model.MpvValue as EngineValue

/** How `seek` lands: exactly where asked, or on the keyframe nearest, for scrubbing. */
enum class SeekMode(val mpv: String) {
    Absolute("absolute+exact"),
    Relative("relative+exact"),
    Keyframe("absolute+keyframes"),
}

/** One call on mpv, for one of the engine's ops. */
sealed interface MpvCall {
    /** `loadfile <url> replace` with per-file options. */
    data class Load(val url: String, val options: Map<String, String>) : MpvCall

    /** `sub-add <url> auto`: listed, not selected. */
    data class AddSubtitle(val url: String, val title: String?, val language: String?) : MpvCall

    data class Set(val property: String, val value: MpvValue) : MpvCall

    /** An input command as mpv spells it: `cycle pause`, `add volume 5`, `af add …`, `stop`. */
    data class Command(val args: List<String>) : MpvCall

    data class Seek(val seconds: Double, val mode: SeekMode) : MpvCall

    /** `glsl-shaders` as one list, so no path is split on a separator. */
    data class Shaders(val paths: List<String>) : MpvCall

    /** Read a property and report it as it stands: `chapter-list` for `RestateChapters`. */
    data class Restate(val property: String) : MpvCall
}

/** What the host does for one of the engine's ops. */
sealed interface HostedStep {
    /** Show the player and start mpv, at the viewer's speed and volume. */
    data class Open(val fullScreen: Boolean, val speed: Double?, val volume: Double?) : HostedStep

    data class Call(val call: MpvCall) : HostedStep

    /** Full screen on, off, or (`null`) the other way: the window's, which mpv knows nothing of. */
    data class FullScreen(val on: Boolean?) : HostedStep

    /** The viewer stopped: end the file, report that it ended by quitting and that the player closed. */
    data object Quit : HostedStep

    /** The playback is over: close the player, and say nothing more. */
    data object Close : HostedStep

    companion object {
        fun of(op: PlayerOp): HostedStep = when (op) {
            is PlayerOp.Open -> Open(op.fullscreen, op.speed, op.volume)
            // A later option of the same name wins, as it would on mpv's command line.
            is PlayerOp.Load -> Call(MpvCall.Load(op.url, op.options.associate { it.name to it.value }))
            is PlayerOp.AddSubtitle -> Call(MpvCall.AddSubtitle(op.url, op.title, op.language))
            is PlayerOp.Set ->
                if (op.property == "fullscreen" && op.value is EngineValue.Flag) {
                    FullScreen((op.value as EngineValue.Flag).v1)
                } else {
                    Call(MpvCall.Set(op.property, mpvValue(op.value)))
                }
            is PlayerOp.Cycle ->
                if (op.property == "fullscreen") FullScreen(null) else Call(MpvCall.Command(listOf("cycle", op.property)))
            PlayerOp.ToggleFullscreen -> FullScreen(null)
            is PlayerOp.Add -> Call(MpvCall.Command(listOf("add", op.property, op.delta.toString())))
            is PlayerOp.Seek -> Call(MpvCall.Seek(op.seconds, if (op.absolute) SeekMode.Absolute else SeekMode.Relative))
            is PlayerOp.Shaders -> Call(MpvCall.Shaders(op.paths))
            is PlayerOp.AudioFilter -> Call(MpvCall.Command(listOf("af", if (op.add) "add" else "remove", op.filter)))
            PlayerOp.RestateChapters -> Call(MpvCall.Restate("chapter-list"))
            PlayerOp.Stop -> Call(MpvCall.Command(listOf("stop")))
            PlayerOp.Quit -> Quit
            PlayerOp.Close -> Close
        }
    }
}

/** The engine's value as mpv takes it: a node arrives as JSON text. */
fun mpvValue(value: EngineValue): MpvValue = when (value) {
    is EngineValue.Flag -> MpvValue.Flag(value.v1)
    is EngineValue.Int -> MpvValue.Int64(value.v1)
    is EngineValue.Double -> MpvValue.Real(value.v1)
    is EngineValue.String -> MpvValue.Text(value.v1)
    is EngineValue.Node -> MpvValue.parse(value.v1) ?: MpvValue.None
}

/** The properties the engine reads, which the player observes from the start. */
val reportedProperties = listOf(
    "time-pos", "duration", "pause", "speed", "volume", "chapter-list", "video-params",
    "current-tracks/video", "track-list", "aid", "sid", "sub-delay", "paused-for-cache",
    "demuxer-cache-state", "frame-drop-count", "seeking",
)

/** The report for one of mpv's events, when the engine reads it: a property, a file loaded or ended. */
fun reportOf(event: MpvEvent): PlayerReport? = when (event.id) {
    MpvNative.Event.PROPERTY_CHANGE ->
        if (event.name in reportedProperties) PlayerReport.Property(event.name!!, event.json ?: "null") else null
    MpvNative.Event.FILE_LOADED -> PlayerReport.FileLoaded
    MpvNative.Event.END_FILE -> PlayerReport.Ended(playerEnd(event.value["reason"].string))
    else -> null
}

/** mpv's `end-file` reason, by its name. */
fun playerEnd(reason: String?): PlayerEnd = when (reason) {
    "eof" -> PlayerEnd.EOF
    "quit" -> PlayerEnd.QUIT
    "stop" -> PlayerEnd.STOP
    "redirect" -> PlayerEnd.REDIRECT
    "error" -> PlayerEnd.ERROR
    else -> PlayerEnd.UNKNOWN
}

/** What libplacebo says when it gives up on a shader hook, as the engine's `HookDisabled` takes it. */
fun disabledHook(event: MpvEvent): String? {
    if (event.id != MpvNative.Event.LOG_MESSAGE) return null
    val text = event.value["text"].string ?: return null
    return if ("hook" in text && "disabling" in text) text.trim() else null
}
