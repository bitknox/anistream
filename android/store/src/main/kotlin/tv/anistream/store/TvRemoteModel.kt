// Play on TV: the TVs on the network and the ones paired, pairing with a code or a scanned QR
// link, and the now-playing card of the TV this phone is driving.
//
// The engine's sender (`FfiTvSender`) holds every rule: which rows the picker lists and in what
// order, what a row says under its name, what a failed pairing tells the viewer, what the card
// reads and when a session is over. This model only keeps its latest answers on the main thread
// for the screens, and hands the platform's DNS-SD browser to it (NsdManager in the app, or the
// fixtures' made-up TVs).

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiPairedTv
import tv.anistream.engine.ffi.FfiRemoteCard
import tv.anistream.engine.ffi.FfiLinkPhase
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvException
import tv.anistream.engine.ffi.FfiTvLink
import tv.anistream.engine.ffi.FfiTvPairing
import tv.anistream.engine.ffi.FfiTvRow
import tv.anistream.engine.ffi.FfiTvRowKind
import tv.anistream.engine.ffi.FfiTvSender
import tv.anistream.engine.ffi.tvPlayhead

/**
 * What Play on TV plays: the episode, what this phone calls the title, and where this phone was
 * in it, which the TV starts from. [fromPlayer]: asked from the player, which closes once the TV
 * plays it.
 */
data class TvPlayRequest(
    val id: AnilistId,
    val title: String,
    val episode: String,
    val from: Double? = null,
    val fromPlayer: Boolean = false,
)

/** A pairing under way: the TV, whether a code is to be typed, and how the last try went. */
data class TvPairingStep(
    val tvName: String,
    /** The TV shows six digits to type here. False for a scanned link, which carried them. */
    val needsCode: Boolean,
    /** A try is out, or the TV is being asked for its code. */
    val working: Boolean,
    /** The engine's words for the last try that failed. */
    val problem: String? = null,
)

/**
 * What the TV shots open on, the fixtures' `tv` extra:
 *
 *     paired   one TV paired already (Bedroom) and one not (Living room), nothing open
 *     picker   the same, with the picker open
 *     code     the picker pairing with Living room, its code to type
 *     remote   Living room paired and playing an episode, its card open
 */
enum class TvScene {
    PAIRED, PICKER, CODE, REMOTE;

    companion object {
        fun named(name: String?): TvScene? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

/** The card as the TV last said it, and when on [TvRemoteModel]'s clock, for the playhead. */
data class TvCard(val card: FfiRemoteCard, val at: Long)

/**
 * Play on TV's state, on the main thread.
 *
 * @param clock milliseconds, for the playhead to move on from the card's time.
 */
class TvRemoteModel(
    private val scope: CoroutineScope,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
) {
    /** What the TV sheet shows. */
    enum class Sheet {
        /** The TVs to choose from, and pairing with one. */
        Picker,

        /** The now-playing card of the TV connected. */
        Remote,
    }

    private val _sheet = MutableStateFlow<Sheet?>(null)
    val sheet: StateFlow<Sheet?> = _sheet.asStateFlow()

    private val _request = MutableStateFlow<TvPlayRequest?>(null)

    /** What choosing a TV plays. `null` when the picker was opened to pair or connect only. */
    val request: StateFlow<TvPlayRequest?> = _request.asStateFlow()

    private val _rows = MutableStateFlow<List<FfiTvRow>>(emptyList())
    val rows: StateFlow<List<FfiTvRow>> = _rows.asStateFlow()

    private val _paired = MutableStateFlow<List<FfiPairedTv>>(emptyList())
    val paired: StateFlow<List<FfiPairedTv>> = _paired.asStateFlow()

    private val _pairing = MutableStateFlow<TvPairingStep?>(null)
    val pairing: StateFlow<TvPairingStep?> = _pairing.asStateFlow()

    private val _connected = MutableStateFlow<String?>(null)

    /** The TV this phone drives, by fingerprint, while a link to it is open. */
    val connected: StateFlow<String?> = _connected.asStateFlow()

    private val _card = MutableStateFlow<TvCard?>(null)

    /** The card as the TV last said it. */
    val card: StateFlow<TvCard?> = _card.asStateFlow()

    private val _problem = MutableStateFlow<String?>(null)

    /** Something the picker or the card has to say: a command that did not get through. */
    val problem: StateFlow<String?> = _problem.asStateFlow()

    /** This phone's fingerprint in short, as a TV's settings list it. */
    var shortFingerprint: String = ""
        private set

    /** Called once the TV plays what the player here was playing: the app closes its player. */
    var handedOff: (() -> Unit)? = null

    /** Called the first time Play on TV is used, before any browse: the app asks for the local network. */
    var firstUse: (() -> Unit)? = null

    private var sender: FfiTvSender? = null
    private var browser: FfiTvBrowser? = null
    private var steps: FfiTvPairing? = null
    private var link: FfiTvLink? = null
    private var watcher: Job? = null
    private var cards: Job? = null
    private var browsers = 0

    /** Whether Play on TV is there: the engine was built with the remote and gave a sender. */
    val available: Boolean get() = sender != null

    internal val attachedSender: FfiTvSender? get() = sender

    /** The browse Play on TV was attached with: the fixtures' fake TVs stage their scenes on it. */
    val attachedBrowser: FfiTvBrowser? get() = browser

    /** Take the engine's sender, browsing with [browser] while the picker is up. */
    fun attach(sender: FfiTvSender, browser: FfiTvBrowser) {
        this.sender = sender
        this.browser = browser
        shortFingerprint = sender.shortFingerprint()
        refresh()
        waiting?.let {
            waiting = null
            begin(it)
        }
        watcher?.cancel()
        watcher = scope.launch {
            while (isActive) {
                sender.changed()
                refresh()
            }
        }
    }

    internal fun refresh() {
        val sender = sender ?: return
        _rows.value = sender.rows()
        _paired.value = sender.paired()
    }

    // Play on TV

    /**
     * Play on TV, from a title, an episode or the player: straight to the TV driven already,
     * otherwise the picker to choose one.
     */
    fun offer(request: TvPlayRequest) {
        if (!available) return
        _problem.value = null
        _request.value = request
        val connected = _connected.value
        if (connected != null && _card.value?.card?.phase == FfiLinkPhase.CONNECTED) play(connected) else openSheet(Sheet.Picker)
    }

    /** The picker, to pair or connect, with nothing to play. */
    fun openPicker() {
        if (!available) return
        _request.value = null
        _problem.value = null
        openSheet(Sheet.Picker)
    }

    /** The card of the TV driven, or the picker when there is none. */
    fun openRemote() {
        openSheet(if (_connected.value == null) Sheet.Picker else Sheet.Remote)
    }

    private var used = false

    private fun openSheet(sheet: Sheet) {
        if (!used) {
            used = true
            firstUse?.invoke()
        }
        _sheet.value = sheet
    }

    /** The sheet closed. A pairing under way is let go; the link stays, for the card to come back to. */
    fun sheetClosed() {
        _sheet.value = null
        _request.value = null
        cancelPairing()
    }

    /** Look for TVs while a picker is up. Counted, as Settings and the sheet can both want it. */
    fun startBrowsing() {
        val sender = sender ?: return
        val browser = browser ?: return
        browsers += 1
        if (browsers != 1) return
        sender.browse(browser)
        refresh()
    }

    fun stopBrowsing() {
        val sender = sender ?: return
        if (browsers == 0) return
        browsers -= 1
        if (browsers != 0) return
        sender.stopBrowsing()
        refresh()
    }

    /** Browse again from the start, as after the local network was allowed. */
    fun browseAgain() {
        val sender = sender ?: return
        val browser = browser ?: return
        if (browsers == 0) return
        sender.stopBrowsing()
        sender.browse(browser)
    }

    /** A row chosen: a paired TV is connected to and plays what was asked; a new one starts pairing. */
    fun choose(row: FfiTvRow) {
        when (row.kind) {
            FfiTvRowKind.PAIRED, FfiTvRowKind.PAIRED_AWAY -> row.fingerprint?.let(::use)
            FfiTvRowKind.NEW -> begin(row.key)
            FfiTvRowKind.NEEDS_UPDATE -> Unit
        }
    }

    /**
     * Pair from [target]: a row's key, a scanned `anistream-remote://pair` link or an address
     * typed in. A TV asked for a code shows one; a link that carried the code pairs at once.
     */
    fun begin(target: String) {
        val sender = sender ?: return
        cancelPairing()
        openSheet(Sheet.Picker)
        val step = TvPairingStep(tvName = "", needsCode = true, working = true)
        _pairing.value = step
        scope.launch {
            try {
                val steps = sender.pair(target)
                if (_pairing.value == null) return@launch
                this@TvRemoteModel.steps = steps
                _pairing.value = TvPairingStep(steps.tvName(), steps.needsCode(), working = true)
                if (steps.needsCode()) {
                    steps.requestCode()
                    _pairing.update { it?.copy(working = false) }
                } else {
                    finish(steps, null)
                }
            } catch (e: FfiTvException) {
                fail(e)
            }
        }
    }

    /**
     * A link the app was opened with: the QR code on a TV's Pair a phone screen, read by the
     * system's camera. False when it is not one.
     */
    fun open(link: String): Boolean {
        if (!link.startsWith("$SCHEME://")) return false
        if (available) begin(link) else waiting = link
        return true
    }

    /** A link the app was opened with before the engine was up, followed once it is. */
    private var waiting: String? = null

    /** The six digits typed. */
    fun submit(code: String) {
        val steps = steps ?: return
        if (_pairing.value?.working != false) return
        _pairing.update { it?.copy(working = true, problem = null) }
        scope.launch {
            try {
                finish(steps, code)
            } catch (e: FfiTvException) {
                fail(e)
            }
        }
    }

    /** Ask the TV for a code again, as after one ran out. */
    fun requestCodeAgain() {
        val steps = steps ?: return
        _pairing.update { it?.copy(working = true, problem = null) }
        scope.launch {
            try {
                steps.requestCode()
                _pairing.update { it?.copy(working = false) }
            } catch (e: FfiTvException) {
                fail(e)
            }
        }
    }

    fun cancelPairing() {
        _pairing.value = null
        steps = null
    }

    private suspend fun finish(steps: FfiTvPairing, code: String?) {
        val tv = steps.pair(code)
        if (_pairing.value == null) return
        cancelPairing()
        refresh()
        use(tv.fingerprint)
    }

    private fun fail(error: FfiTvException) {
        if (_pairing.value == null) return
        _pairing.update { it?.copy(working = false, problem = words(error)) }
    }

    // The link

    /** Connect to a paired TV, then play what was asked on it, or show its card. */
    internal fun use(fingerprint: String) {
        connect(fingerprint)
        if (_request.value != null) play(fingerprint) else openSheet(Sheet.Remote)
    }

    /** Hold a link to the TV, reading its card until it closes. */
    fun connect(fingerprint: String) {
        val sender = sender ?: return
        if (_connected.value == fingerprint && link != null) return
        disconnect()
        val link = sender.link(fingerprint) ?: return
        this.link = link
        _connected.value = fingerprint
        _card.value = TvCard(link.card(), clock())
        cards = scope.launch {
            while (isActive) {
                val card = link.nextCard() ?: break
                if (this@TvRemoteModel.link !== link) break
                _card.value = TvCard(card, clock())
            }
        }
    }

    private fun play(fingerprint: String) {
        val request = _request.value ?: return
        val link = link ?: return
        if (_connected.value != fingerprint) return
        _request.value = null
        openSheet(Sheet.Remote)
        scope.launch {
            try {
                link.play(request.id, request.title, request.episode, request.from)
                if (request.fromPlayer) handedOff?.invoke()
            } catch (e: FfiTvException) {
                _problem.value = words(e)
            }
        }
    }

    /** Stop driving the TV. What it plays plays on. */
    fun disconnect() {
        cards?.cancel()
        cards = null
        link?.hangUp()
        link?.destroy()
        link = null
        _connected.value = null
        _card.value = null
    }

    /** Send the TV a command from the card, the notification or the volume keys. */
    fun send(command: FfiTvCommand) {
        val link = link ?: return
        _problem.value = null
        scope.launch {
            try {
                link.send(command)
            } catch (e: FfiTvException) {
                _problem.value = words(e)
            }
        }
    }

    /** Try the TV again now, as the network or the app comes back, rather than after the wait. */
    fun retryNow() {
        link?.retryNow()
    }

    /** Forget a paired TV, on both sides when it can be reached. */
    fun forget(fingerprint: String) {
        val sender = sender ?: return
        if (_connected.value == fingerprint) {
            disconnect()
            if (_sheet.value == Sheet.Remote) _sheet.value = null
        }
        _paired.update { tvs -> tvs.filterNot { it.fingerprint == fingerprint } }
        scope.launch {
            sender.forget(fingerprint)
            refresh()
        }
    }

    /** Where the TV's playhead is now: moved on from the card on this phone's clock while it plays. */
    fun playhead(at: Long = clock()): Double? {
        val card = _card.value ?: return null
        val now = card.card.now ?: return null
        return tvPlayhead(now, ((at - card.at).coerceAtLeast(0)) / 1000.0)
    }

    /** Whether [scene] is on screen, as a shot waits for it. */
    fun reached(scene: TvScene): Boolean = when (scene) {
        TvScene.PAIRED -> _paired.value.isNotEmpty()
        TvScene.PICKER -> _sheet.value == Sheet.Picker && _rows.value.size == 2
        TvScene.CODE -> _pairing.value.let { it != null && it.needsCode && !it.working && it.tvName.isNotEmpty() }
        TvScene.REMOTE -> _card.value?.card?.now?.let { !it.loading && it.subtitles.isNotEmpty() } == true
    }

    /** The paired TV this phone drives, for the card's header and Settings. */
    fun connectedTv(): FfiPairedTv? = _connected.value?.let { fp -> _paired.value.firstOrNull { it.fingerprint == fp } }

    companion object {
        /** The scheme of the link a TV's QR code carries. */
        const val SCHEME = "anistream-remote"

        /** What a TV lists this device as: the kind of device, never the name it was given. */
        const val DEVICE_NAME = "Android phone"

        /** The volume keys' and buttons' step, out of the TV's 100. */
        const val VOLUME_STEP = 5.0

        /** The engine's words for a failure. */
        fun words(error: FfiTvException): String = when (error) {
            is FfiTvException.Failed -> error.reason
        }
    }
}
