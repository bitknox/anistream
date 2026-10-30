// What the player model's tests share: an mpv that only records, and a model on it.

package tv.anistream.player

import android.view.Surface
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.MpvOption
import tv.anistream.engine.model.PlayerCommand
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerReport
import tv.anistream.engine.model.PlayerUi

/** An mpv that records each call and says what the test tells it to. */
class ScriptedMpv : PlayerBackend {
    val calls = mutableListOf<MpvCall>()
    var begun: String? = null
    /** The options mpv was started with besides the player's own. */
    var begunWith: Map<String, String> = emptyMap()
    var ended = false
    var attached = 0
    var detached = 0
    val sizes = mutableListOf<Pair<Int, Int>>()
    val answers = mutableMapOf<String, MpvValue>()

    /** Calls that never come back: mpv's core stuck behind a stalled decoder. */
    var stuck = false

    override var listener: ((MpvEvent) -> Unit)? = null

    /** The playlist entries mpv made for the loads, in order: one each, as mpv numbers them. */
    val entries = mutableListOf<Long>()

    /** The newest entry a `START_FILE` has named. */
    private var startedThrough = 0L

    /** The newest load's entry: what mpv starts when it takes several loads back to back. */
    val newestEntry: Long get() = entries.last()

    /**
     * A `START_FILE` with no entry is for the oldest load not yet started, as mpv starts loads
     * spaced out enough to start each; [MpvEvent.startFile] names one, for loads it coalesced.
     */
    fun emit(event: MpvEvent) {
        val named = if (event.id == MpvNative.Event.START_FILE && event.json == null) {
            entries.firstOrNull { it > startedThrough }?.let(MpvEvent::startFile) ?: event
        } else {
            event
        }
        named.entry?.let { startedThrough = maxOf(startedThrough, it) }
        listener?.invoke(named)
    }

    /** Set: a load's answer, the entry mpv made, waits for it, as a busy mpv thread keeps it. */
    var answering: kotlinx.coroutines.CompletableDeferred<Unit>? = null

    override suspend fun load(call: MpvCall.Load): Long? {
        perform(call)
        answering?.await()
        if (ended) return null
        return (entries.size + 1L).also { entries += it }
    }

    override suspend fun begin(hwdec: String, options: Map<String, String>) {
        begun = hwdec
        begunWith = options
    }

    override suspend fun perform(call: MpvCall): List<PlayerReport> {
        if (ended) return emptyList()
        if (stuck) kotlinx.coroutines.awaitCancellation()
        calls += call
        return when (call) {
            is MpvCall.Restate -> listOf(PlayerReport.Property(call.property, (answers[call.property] ?: MpvValue.List(emptyList())).json))
            // Only what it was told to answer, as mpv has no value for the rest.
            is MpvCall.Sample -> call.properties.mapNotNull { p -> answers[p]?.let { PlayerReport.Property(p, it.json) } }
            else -> emptyList()
        }
    }

    /** Whether mpv was asked to decode in software, and answered. */
    var software = false

    override suspend fun decodeInSoftware(timeoutMs: Long): Boolean {
        if (stuck) {
            kotlinx.coroutines.delay(timeoutMs)
            return false
        }
        software = true
        return true
    }

    override fun attach(surface: Surface) {
        attached++
    }

    override fun resize(width: Int, height: Int) {
        sizes += width to height
    }

    override fun detach() {
        detached++
    }

    override suspend fun end() {
        ended = true
    }
}

/** A model on scripted mpvs, and what it sends the engine. */
class Rig(private val test: TestScope, hwdec: String = "no", display: DisplayInfo? = null) {
    val sent = mutableListOf<Command>()
    val made = mutableListOf<ScriptedMpv>()

    /** The system's caption style the model reads: none, as with captions off, unless a test sets one. */
    var captionStyle: CaptionStyle? = null

    val model = PlayerModel(
        send = { sent += it },
        backend = { ScriptedMpv().also { made += it } },
        scope = test.backgroundScope,
        now = { test.testScheduler.currentTime },
        hwdec = hwdec,
        display = { display },
        systemCaptionStyle = { captionStyle },
    )

    val mpv: ScriptedMpv get() = made.last()

    /** Let the model's coroutines run what is due now. */
    fun settle() = test.runCurrent()

    fun ops(playback: ULong, vararg ops: PlayerOp) {
        model.apply(Event.PlayerOps(playback, ops.toList()))
        settle()
    }

    fun ui(playback: ULong, ui: PlayerUi) {
        model.apply(Event.PlayerUi(playback, ui))
        settle()
    }

    fun emit(event: MpvEvent) {
        mpv.emit(event)
        settle()
    }

    /** The reports sent, with their playbacks. */
    val reports: List<Pair<ULong, PlayerReport>>
        get() = sent.filterIsInstance<Command.PlayerReport>().map { it.playback to it.report }

    /** The commands sent to steer playback, aimed at a playback or at whatever plays. */
    val controls: List<PlayerCommand>
        get() = sent.mapNotNull {
            when (it) {
                is Command.Control -> it.command
                is Command.Player -> it.v1
                else -> null
            }
        }

    /** The playbacks the commands were aimed at, `null` for whatever plays. */
    val aims: List<ULong?>
        get() = buildList {
            for (command in sent) {
                when (command) {
                    is Command.Control -> add(command.playback)
                    is Command.Player -> add(null)
                    else -> {}
                }
            }
        }

    /** Open a player for [playback] and load a file into it, which mpv starts. */
    fun opened(playback: ULong = 1u, url: String = "/media/episode.mkv", options: List<MpvOption> = emptyList()) {
        ops(playback, PlayerOp.Open(false, null, null), PlayerOp.Load(url, options))
        emit(MpvEvent.of(MpvNative.Event.START_FILE))
        emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
    }

    /** mpv's playhead at [seconds]. */
    fun playhead(seconds: Double) = emit(MpvEvent.property("time-pos", MpvValue.Real(seconds)))
}
