// Casting to TVs with Chromecast built in: the Chromecast rows of the Play on TV picker, and the
// card of the cast running now.
//
// The engine's sender holds every rule, as it does for Play on TV: which Cast devices the picker
// lists, how a cast starts, falls back and ends, and what the card reads. This model keeps its
// latest answers on the main thread for the screens, hands the platform's `_googlecast._tcp`
// browse to it (NsdManager in the app, made-up records in the fixtures), and says when the card
// starts, fails, plays and ends.

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiCastBrowser
import tv.anistream.engine.ffi.FfiCastCard
import tv.anistream.engine.ffi.FfiCastRow
import tv.anistream.engine.ffi.FfiCastSession
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvException
import tv.anistream.engine.ffi.FfiVolumeControl
import tv.anistream.engine.ffi.tvPlayhead
import tv.anistream.engine.model.Msg

/**
 * A cast as the card reads it and drives it: the engine's [FfiCastSession], or the fixtures'
 * scripted TV.
 */
interface CastLink {
    val tvName: String

    /** The card as it stands. */
    fun card(): FfiCastCard

    /** The card once it may read differently; `null` once the engine has stopped. */
    suspend fun nextCard(): FfiCastCard?

    fun command(command: FfiTvCommand)

    /** Stop casting, and the position to play on from here when it is past the first seconds. */
    fun playHere(): Double?

    /** Up next's Play now. */
    fun playNext()

    /** Up next's Cancel: the episode does not roll on. */
    fun cancelNext()

    /** The Undo note's button: back to where an automatic skip began. */
    fun undoSkip()

    /** Cast the same episode to the same TV again, from its position; `false` when it cannot. */
    fun tryAgain(): Boolean

    fun stop()
}

/** The engine's cast session as a [CastLink]. */
class EngineCastLink(private val session: FfiCastSession) : CastLink {
    override val tvName: String = session.deviceName()

    override fun card(): FfiCastCard = session.card()

    override suspend fun nextCard(): FfiCastCard? = session.nextCard()

    override fun command(command: FfiTvCommand) = session.command(command)

    override fun playHere(): Double? = session.playHere()

    override fun playNext() = session.playNext()

    override fun cancelNext() = session.cancelNext()

    override fun undoSkip() = session.undoSkip()

    override fun tryAgain(): Boolean = session.tryAgain()

    override fun stop() = session.stop()
}

/** Where a cast is, as the card draws it. */
enum class CastStage {
    /** Connecting, opening the receiver, preparing the stream or waiting for the TV. */
    Starting,

    /** The TV plays the episode, paused or not. */
    Playing,

    /** Nothing played: every way of playing it ran out. The card says why until dismissed. */
    Failed,

    /** The TV dropped off mid-cast. The card says so until dismissed. */
    Lost,
}

/**
 * The cast card: the engine's card, when on [CastModel]'s clock it came for the playhead, what was
 * asked of the TV, and when, for the starting line's counter.
 */
data class CastShown(
    val cast: FfiCastCard,
    val at: Long,
    val tvName: String,
    val request: TvPlayRequest,
    val startedAt: Long,
) {
    val stage: CastStage
        get() {
            val notice = cast.notice
            val now = cast.card.now
            return when {
                notice is Msg.NoticeCastLost -> CastStage.Lost
                notice != null -> CastStage.Failed
                now == null || now.loading -> CastStage.Starting
                else -> CastStage.Playing
            }
        }

    /** The TV takes a volume level; otherwise its own volume is in charge and the card has Mute alone. */
    val level: Boolean get() = cast.volumeControl == FfiVolumeControl.LEVEL
}

/**
 * A cast that ended with something to say once the card has gone: someone else took the TV. The
 * notice offers to play on here, from where the TV was.
 */
data class CastEnded(val notice: Msg, val id: AnilistId, val episode: String)

/**
 * Casting's state, on the main thread.
 *
 * @param clock milliseconds, for the playhead to move on from the card's time and the starting
 *   line to count.
 */
class CastModel(
    private val scope: CoroutineScope,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
) {
    private val _rows = MutableStateFlow<List<FfiCastRow>>(emptyList())

    /** The picker's Chromecast rows, while it is open. */
    val rows: StateFlow<List<FfiCastRow>> = _rows.asStateFlow()

    private val _blocked = MutableStateFlow(false)

    /** Android 17's local network permission was turned down: the picker says so, with Open settings. */
    val blocked: StateFlow<Boolean> = _blocked.asStateFlow()

    private val _shown = MutableStateFlow<CastShown?>(null)

    /** The card of the cast running now, or of the one that failed or dropped off until dismissed. */
    val shown: StateFlow<CastShown?> = _shown.asStateFlow()

    private val _open = MutableStateFlow(false)

    /** The card's sheet is up. */
    val open: StateFlow<Boolean> = _open.asStateFlow()

    private val _ended = MutableSharedFlow<CastEnded>(extraBufferCapacity = 1)

    /** Casts that ended with a notice once their card had gone, for the snackbar. */
    val ended: SharedFlow<CastEnded> = _ended.asSharedFlow()

    private val _problem = MutableStateFlow<String?>(null)

    /** The engine's words for a cast that could not even be asked for, as the TV left the network. */
    val problem: StateFlow<String?> = _problem.asStateFlow()

    /**
     * Plays an episode on this device: Play here, once the cast has stopped. The app sends the
     * engine its play, which resumes where the TV left off.
     */
    var playHere: ((AnilistId, String) -> Unit)? = null

    /** Called once the TV plays what the player here was playing: the app closes its player. */
    var handedOff: (() -> Unit)? = null

    /**
     * How a row's cast starts: the engine's sender, unless a fixtures scene stands a scripted TV
     * in. `null` when the engine said no, with its words in [problem].
     */
    var starter: ((key: String, request: TvPlayRequest) -> CastLink?)? = null

    private var tv: TvRemoteModel? = null
    private var browser: FfiCastBrowser? = null
    private var browsing = false
    private var link: CastLink? = null
    private var cards: Job? = null

    /** The session of a cast that failed or dropped off, kept for Try again until the card is dismissed. */
    private var over: CastLink? = null

    /** Whether casting is there: the engine's sender came with a Cast browse to give it. */
    val available: Boolean get() = tv?.attachedSender != null && browser != null

    /**
     * Cast through [tv]'s sender, browsing with [browser] while the picker is up. The sender's
     * changes reach the rows through [tv], which watches them.
     */
    fun attach(tv: TvRemoteModel, browser: FfiCastBrowser) {
        this.tv = tv
        this.browser = browser
        tv.changed = ::refresh
        if (starter == null) {
            starter = { key, request ->
                val sender = tv.attachedSender
                try {
                    sender?.cast(key, request.id, request.episode, request.from)?.let(::EngineCastLink)
                } catch (e: FfiTvException) {
                    _problem.value = TvRemoteModel.words(e)
                    null
                }
            }
        }
        refresh()
    }

    internal fun refresh() {
        val sender = tv?.attachedSender ?: return
        _rows.value = if (browsing) sender.castRows() else emptyList()
    }

    /**
     * Look for TVs with Chromecast built in while the picker is up. The picker's own browse is
     * started first: it stops this one with it as the picker closes.
     */
    fun startBrowsing() {
        val sender = tv?.attachedSender ?: return
        val browser = browser ?: return
        browsing = true
        sender.browseCast(browser)
        refresh()
    }

    fun stopBrowsing() {
        browsing = false
        _rows.value = emptyList()
    }

    /** Browse again from the start, as after the local network was allowed. */
    fun browseAgain() {
        if (browsing) startBrowsing()
    }

    /**
     * What Android said of the local network permission: turned down, the picker says so in
     * place of its rows; allowed, both browses start again.
     */
    fun localNetwork(allowed: Boolean) {
        val was = _blocked.value
        _blocked.value = !allowed
        if (allowed && was) {
            tv?.browseAgain()
            browseAgain()
        }
    }

    /** Cast [request] to the Chromecast row [row]: the picker closes and the card opens on its starting state. */
    fun cast(row: FfiCastRow, request: TvPlayRequest) = cast(row.key, request)

    private fun cast(key: String, request: TvPlayRequest) {
        _problem.value = null
        val next = starter?.invoke(key, request) ?: return
        end()
        link = next
        val now = clock()
        _shown.value = CastShown(next.card(), now, next.tvName, request, now)
        _open.value = true
        tv?.sheetClosed()
        follow(next, request)
    }

    /** Take the cards [next] sends until the cast is over. */
    private fun follow(next: CastLink, request: TvPlayRequest) {
        cards = scope.launch {
            var handed = !request.fromPlayer
            while (isActive) {
                val card = next.nextCard() ?: break
                if (link !== next) break
                take(card)
                if (!handed && _shown.value?.stage == CastStage.Playing) {
                    handed = true
                    handedOff?.invoke()
                }
            }
        }
    }

    /**
     * The card the engine sent. The playhead moves on from where and when it said; a cast that is
     * over leaves its card up when it has something to say there, and goes, with a notice for the
     * snackbar, when someone else took the TV.
     */
    private fun take(card: FfiCastCard) {
        val held = _shown.value ?: return
        val same = held.cast.card.now?.position == card.card.now?.position && held.cast.card.now?.paused == card.card.now?.paused
        _shown.value = held.copy(cast = card, at = if (same) held.at else clock())
        if (!card.card.over) return
        val notice = card.notice
        when {
            notice is Msg.NoticeCastTakenOver -> {
                val now = card.card.now
                end()
                _ended.tryEmit(CastEnded(notice, now?.id ?: held.request.id, now?.episode ?: held.request.episode))
            }
            // Stopped by the viewer, here or from the notification.
            notice == null -> end()
            // Failed or lost: the card says so until it is dismissed.
            else -> {
                cards?.cancel()
                cards = null
                over = link
                link = null
            }
        }
    }

    /** Let the cast and its card go. */
    private fun end() {
        cards?.cancel()
        cards = null
        link = null
        over = null
        _shown.value = null
        _open.value = false
    }

    // The card

    fun openCard() {
        if (_shown.value != null) _open.value = true
    }

    fun closeCard() {
        _open.value = false
    }

    /** Send the TV a command from the card, the notification or the volume keys. */
    fun send(command: FfiTvCommand) {
        link?.command(command)
    }

    /** Mute the TV, or bring its sound back. */
    fun toggleMute() {
        val muted = _shown.value?.cast?.muted ?: false
        send(FfiTvCommand.Mute(!muted))
    }

    /** Move a TV's volume level to [level] out of 100, in the steps the engine sends. */
    fun setVolume(level: Double) {
        val volume = _shown.value?.cast?.card?.now?.volume ?: return
        val step = level - volume
        if (step != 0.0) send(FfiTvCommand.Volume(step))
    }

    /** A step of the volume keys, out of 100: nothing on a TV whose volume is fixed. */
    fun stepVolume(up: Boolean) {
        if (_shown.value?.level != true) return
        send(FfiTvCommand.Volume(if (up) VOLUME_STEP else -VOLUME_STEP))
    }

    /** Stop the TV and play on here, from where it was. */
    fun playHere() {
        val shown = _shown.value ?: return
        link?.playHere()
        val now = shown.cast.card.now
        end()
        playHere?.invoke(now?.id ?: shown.request.id, now?.episode ?: shown.request.episode)
    }

    /** Stop casting: the receiver quits on the TV, the card and the bar go. */
    fun stop() {
        link?.stop()
        end()
    }

    /** The card of a cast that failed or dropped off, dismissed. */
    fun dismiss() {
        if (link == null) end()
    }

    /** Up next's Play now: the next episode starts on the TV at once. */
    fun playNext() {
        link?.playNext()
    }

    /** Up next's Cancel: the episode does not roll on. */
    fun cancelNext() {
        link?.cancelNext()
    }

    /** Undo after an automatic skip. */
    fun undoSkip() {
        link?.undoSkip()
    }

    /**
     * Start the same episode on the same TV again from its position, as Try again on a failed
     * card asks. The card starts over.
     */
    fun tryAgain() {
        val shown = _shown.value ?: return
        val again = over ?: return
        if (!again.tryAgain()) return
        over = null
        link = again
        val now = clock()
        _shown.value = shown.copy(cast = again.card(), at = now, startedAt = now)
        follow(again, shown.request)
    }

    /**
     * Choose another TV for the episode playing here: the picker, with the episode at the TV's
     * position. The cast goes on until another TV takes it.
     */
    fun chooseAnother() {
        val shown = _shown.value ?: return
        val tv = tv ?: return
        val now = shown.cast.card.now
        _open.value = false
        tv.offer(
            TvPlayRequest(
                id = now?.id ?: shown.request.id,
                title = now?.title ?: shown.request.title,
                episode = now?.episode ?: shown.request.episode,
                from = playhead(),
            ),
        )
    }

    /** Where the TV's playhead is now: moved on from the card on this device's clock while it plays. */
    fun playhead(at: Long = clock()): Double? {
        val shown = _shown.value ?: return null
        val now = shown.cast.card.now ?: return null
        if (shown.stage != CastStage.Playing) return now.position
        return tvPlayhead(now, ((at - shown.at).coerceAtLeast(0)) / 1000.0)
    }

    /** Whole seconds since the cast was asked for, for the starting line's counter. */
    fun startingSeconds(at: Long = clock()): Long {
        val shown = _shown.value ?: return 0
        return ((at - shown.startedAt).coerceAtLeast(0)) / 1000
    }

    companion object {
        /** The volume keys' step, out of the TV's 100. */
        const val VOLUME_STEP = 5.0

        /** Past this many seconds the starting line counts. */
        const val COUNT_FROM_SECONDS = 4L

        /** What the TV plays, as the card shows it: what the engine says, else what was asked while it starts. */
        fun now(shown: CastShown): FfiNowCard = shown.cast.card.now ?: FfiNowCard(
            id = shown.request.id,
            title = shown.request.title,
            episode = shown.request.episode,
            episodeLine = "",
            loading = true,
            position = shown.request.from ?: 0.0,
            duration = null,
            paused = false,
            speed = 1.0,
            volume = null,
            audio = emptyList(),
            subtitles = emptyList(),
            subtitlesOff = false,
            skip = null,
        )
    }
}
