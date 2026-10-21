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
    var ended = false
    var attached = 0
    var detached = 0
    val sizes = mutableListOf<Pair<Int, Int>>()
    val answers = mutableMapOf<String, MpvValue>()

    /** Calls that never come back: mpv's core stuck behind a stalled decoder. */
    var stuck = false

    override var listener: ((MpvEvent) -> Unit)? = null

    fun emit(event: MpvEvent) {
        listener?.invoke(event)
    }

    override suspend fun begin(hwdec: String) {
        begun = hwdec
    }

    override suspend fun perform(call: MpvCall): PlayerReport? {
        if (ended) return null
        if (stuck) kotlinx.coroutines.awaitCancellation()
        calls += call
        if (call is MpvCall.Restate) return PlayerReport.Property(call.property, (answers[call.property] ?: MpvValue.List(emptyList())).json)
        return null
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
class Rig(private val test: TestScope, hwdec: String = "no") {
    val sent = mutableListOf<Command>()
    val made = mutableListOf<ScriptedMpv>()
    val model = PlayerModel(
        send = { sent += it },
        backend = { ScriptedMpv().also { made += it } },
        scope = test.backgroundScope,
        now = { test.testScheduler.currentTime },
        hwdec = hwdec,
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
