// The player the app hosts for the engine: its ops carried out on mpv, mpv's events reported back,
// and what the player's screens show. After Apple's PlayerModel.swift, on main's hosted.rs.

package tv.anistream.player

import android.util.Log
import android.view.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Ending
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.PlayerChoice
import tv.anistream.engine.model.PlayerCommand
import tv.anistream.engine.model.PlayerEnd
import tv.anistream.engine.model.PlayerOp
import tv.anistream.engine.model.PlayerReport
import tv.anistream.engine.model.PlayerUi
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.TrackKind

/** The screen the player is on: its size in pixels, its density and refresh rate. */
data class DisplayInfo(val width: Int, val height: Int, val scale: Double, val refreshHz: Double?)

/** What the device runs on, for the engine's upscaling. */
data class PowerState(val onBattery: Boolean, val lowPower: Boolean)

/**
 * The app's one player, for the life of the process: the activity only lends it a surface, so a
 * rotation, a fold or Picture in Picture never touches mpv (the spike's AS9). Every call is made
 * on the main thread.
 *
 * - The engine's `PlayerOps` run on mpv in order, one op to one call, on a coroutine of their own
 *   that awaits each: nothing waits on mpv from the main thread.
 * - mpv's events become the engine's `PlayerReport`s, for the playback whose file mpv has: a `Load`
 *   belongs to its playback from mpv's `start-file` on, and what comes before it to the playback
 *   before.
 * - `Quit`, and the viewer closing the player, report `Ended { Quit }` and then `Closed`; `Close`
 *   says nothing. Either way mpv shuts down, and every op after it for those playbacks is refused,
 *   until the engine opens a player again.
 * - The engine's `PlayerUi` is kept here until it says otherwise, with the cards' timing: the Up
 *   Next countdown runs on the playhead from `ending_at`, and sends `PlayNext` at zero.
 * - A file that loads but shows no picture while mpv is not waiting for data is a hardware decoder
 *   that stalled: mpv decodes in software from then on (the spike's AS5).
 *
 * @param send hands the engine a command.
 * @param backend makes the mpv for each player opened.
 * @param scope the main thread's, for as long as the process.
 * @param now the clock the cards keep time by, in milliseconds.
 */
class PlayerModel(
    private val send: (Command) -> Unit,
    private val backend: () -> PlayerBackend,
    private val scope: CoroutineScope,
    private val now: () -> Long = { android.os.SystemClock.elapsedRealtime() },
    private val power: () -> PowerState = { PowerState(onBattery = false, lowPower = false) },
    /** The hardware decoding mpv starts with: none in the emulator (AS5). */
    private val hwdec: String = MpvSetup.defaultHwdec,
    /**
     * The screen the player is on, as the window has it before any surface: said as the player
     * opens, so the engine's first upscaling chain is chosen for it, and no compute shaders on
     * GLES, rather than stepped down from a guess a moment later.
     */
    private val display: () -> DisplayInfo? = { null },
    /** The system's caption style as it is now, or null while captions are off; a made-up one in the tests. */
    private val systemCaptionStyle: () -> CaptionStyle? = { null },
) {
    // Wiring

    private var mpv: PlayerBackend? = null
    private var work: Channel<Work>? = null
    private var worker: Job? = null
    private var ticker: Job? = null
    private var watchdog: Job? = null

    /** What the session player and the notification follow: called when what they show changes. */
    var onSessionChange: (() -> Unit)? = null

    private sealed interface Work {
        data class Call(val call: MpvCall) : Work
    }

    // The player

    /** Whether the player is up: from the playback staged, or the engine's Open, until it closes. */
    var isPresented by mutableStateOf(false)
        private set

    /** Whether there is an mpv now, to draw into the surface. */
    var hasMpv by mutableStateOf(false)
        private set

    /** The window fills the screen: the engine's Open and its cycles. A phone's player always does. */
    var fullScreen by mutableStateOf(false)
        private set

    /** The newest playback the engine has spoken of, whose file the player shows or is opening. */
    var playback by mutableStateOf<ULong?>(null)
        private set

    /** The playback whose Open made this player. */
    private var opened: ULong? = null

    /** Loads sent to mpv whose files have not started, oldest first. */
    private val loads = ArrayDeque<ULong>()

    /** The playback whose file mpv has now. */
    private var current: ULong? = null

    /** What each playback's Load opened, for the seek preview and for opening it again. */
    private val sources = mutableMapOf<ULong, MpvCall.Load>()

    /** Where previews are read from instead of the stream, when the engine says so. */
    var thumbnails by mutableStateOf<String?>(null)
        private set

    /** Every playback up to this one is over: its ops are refused. */
    private var closedThrough: ULong? = null

    /**
     * The viewer closed the player: every op is refused until the engine stages a playback again,
     * so one it was still finding cannot bring the player back up.
     */
    private var closedByViewer = false

    /**
     * A playback has been staged that has said nothing yet: the one on screen, if any, is older,
     * and stopping is for the staged one, which only a Stop for whatever plays reaches.
     */
    private var stagedAhead = false

    /** The playbacks the engine has been told this player closed for. */
    private val closedSaid = mutableSetOf<ULong>()

    /** The hardware decoding the mpv now open started with, or was moved to. */
    var decoding by mutableStateOf(hwdec)
        private set

    /** The last value set of each property, and the shaders, to set again on an mpv opened in its place. */
    private val settings = linkedMapOf<String, MpvCall>()

    // The surface

    private var surface: Surface? = null
    private var surfaceSize: Pair<Int, Int>? = null
    private val surfaced = MutableStateFlow(false)

    /**
     * Bumped when the surface has to be made again: an mpv whose decoder stalled holds the one
     * there is, and a new SurfaceView gives the next mpv its own.
     */
    var surfaceGeneration by mutableIntStateOf(0)
        private set

    // What the engine says

    var staged by mutableStateOf<StagedPlayback?>(null)
        private set
    var starting by mutableStateOf<StartingScreen?>(null)
        private set
    var episode by mutableStateOf<EpisodeOnScreen?>(null)
        private set
    var menus by mutableStateOf<PlayerMenus?>(null)
        private set
    var skip by mutableStateOf<SkipOffer?>(null)
        private set
    var undo by mutableStateOf<UndoOffer?>(null)
        private set
    var upNext by mutableStateOf<UpNextCard?>(null)
        private set
    var stall by mutableStateOf<StallCard?>(null)
        private set
    var resume by mutableStateOf<ResumeNote?>(null)
        private set
    var alert by mutableStateOf<PlayerAlert?>(null)
        private set
    var upscaler by mutableStateOf<String?>(null)
        private set
    var hdr by mutableStateOf<String?>(null)
        private set

    /** The playback never played: "Couldn't play episode 7", with Choose source and Close. */
    var failure by mutableStateOf<PlaybackFailure?>(null)
        private set

    /** The viewer asked for the episode's releases; [sourceChoices] is null until they come. */
    var choosingSource by mutableStateOf(false)
        private set
    var sourceChoices by mutableStateOf<List<SourceCandidate>?>(null)
        private set

    /** The player closed before a frame: waiting for the engine to say whether it failed. */
    private var verdictPending = false

    /** "Still watching?": the episodes rolled on in a row with nobody there, which pauses. */
    var stillWatching by mutableStateOf(false)
        private set

    /** Skips the viewer has taken, for the haptic that answers each. */
    var skipsTaken by mutableIntStateOf(0)
        private set

    /** After how many episodes in a row with nobody there it asks, as the engine last said: `0` is never. */
    var stillWatchingAfter by mutableIntStateOf(PlayerTiming.STILL_WATCHING_AFTER)
        private set

    /** Keep waiting was answered: no stall card until this stall ends. */
    private var stallWaived = false

    // What mpv says

    /** The playhead, to a quarter of a second: what the screens read. */
    var position by mutableStateOf(0.0)
        private set

    /** The playhead as mpv last said it. */
    var playhead = 0.0
        private set
    var duration by mutableStateOf<Double?>(null)
        private set
    var paused by mutableStateOf(false)
        private set
    var speed by mutableStateOf(1.0)
        private set
    var volume by mutableStateOf(100.0)
        private set
    var chapters by mutableStateOf<List<Chapter>>(emptyList())
        private set
    var tracks by mutableStateOf<List<Track>>(emptyList())
        private set
    var audioTrack by mutableStateOf<Long?>(null)
        private set
    var subtitleTrack by mutableStateOf<Long?>(null)
        private set

    /** mpv is waiting for data, since then. */
    var bufferingSince by mutableStateOf<Long?>(null)
        private set

    /** The file playing has put a picture up. */
    var hasPicture by mutableStateOf(false)
        private set

    /** The video's aspect, width over height, once mpv knows it. */
    var aspect by mutableStateOf<Double?>(null)
        private set

    /** The picture's height in lines and its pixel format, for the Info panel: 1080, "yuv420p10". */
    var videoHeight by mutableStateOf<Int?>(null)
        private set
    var pixelFormat by mutableStateOf<String?>(null)
        private set

    /** The video track's codec as mpv names it ("hevc"), for the Info panel. */
    var videoCodec by mutableStateOf<String?>(null)
        private set

    /** Frames mpv dropped in this file, for the Info panel. */
    var droppedFrames by mutableStateOf(0L)
        private set

    // The viewer

    /** Now, a few times a second while the player is up: what the cards' timing reads. */
    var clock by mutableLongStateOf(now())
        private set

    /** The last tap, drag or key: the controls hide a while after it. */
    var lastInput by mutableLongStateOf(now())
        private set

    /** What keeps the controls up: scrubbing, a sheet open, TalkBack. */
    var holds by mutableStateOf<Set<String>>(emptySet())
        private set

    /** Scrubbing, at this time: the seek line follows the finger, not mpv. */
    var scrubbing by mutableStateOf<Double?>(null)
        private set
    private var scrubSent: Double? = null

    /** Episodes rolled on in a row since the viewer last did anything. */
    var rolledOn = 0
        private set
    private var rollPending = false

    /**
     * TalkBack or Switch Access is on: the controls stay up, Up Next waits for the viewer rather
     * than counting down, and the skip stays at full strength (T4, T5).
     */
    var assisted by mutableStateOf(false)

    /** The title's art, from the starting screen, for the media session: the cover, and the banner. */
    var coverUrl by mutableStateOf<String?>(null)
        private set
    var bannerUrl by mutableStateOf<String?>(null)
        private set

    /**
     * The activity is in Picture in Picture, or on its way into it: the window shows the video
     * alone, and the cards, the controls and the sheets are not drawn.
     */
    var inPip by mutableStateOf(false)

    /** The app is in the background, playing audio only. */
    var background = false
        private set

    // The engine's events

    /** One of the engine's events: the player's ops and interface, and the playback around them. */
    fun apply(event: Event) {
        when (event) {
            is Event.PlayerOps -> for (op in event.ops) apply(op, event.playback)
            is Event.PlayerUi -> apply(event.ui, event.playback)
            is Event.PlaybackStaged -> {
                closedByViewer = false
                stagedAhead = true
                failure = null
                choosingSource = false
                sourceChoices = null
                verdictPending = false
                staged = StagedPlayback(event.id, event.title, event.episode, event.episodeTitle)
                present()
                changed()
            }
            is Event.PlaybackEnded -> when {
                // Not when the viewer sought into the ending: someone was there.
                event.rollingOn -> rollPending = upNext?.sought != true
                mpv == null && event.ending == Ending.Failed && isPresented && startingEpisode != null -> {
                    // It never played: no stream was found, or none showed a frame. Say so where the
                    // viewer is looking, with another source to choose, rather than vanish.
                    verdictPending = false
                    failure = PlaybackFailure(staged?.id, startingEpisode!!)
                    note("couldn't play episode ${failure?.episode}")
                }
                mpv == null -> {
                    // Nothing was opened for it, or it played elsewhere, or it is simply over.
                    verdictPending = false
                    dismiss()
                }
            }
            is Event.Sources -> if (choosingSource) sourceChoices = event.candidates
            // The playback staged found nothing; the one before plays on.
            Event.PlaybackKept -> starting = null
            else -> {}
        }
    }

    internal fun apply(op: PlayerOp, playback: ULong) {
        if (!accepts(playback)) {
            note("#$playback refused ${describe(op)}: ${if (closedByViewer) "the viewer closed the player" else "that playback is over"}")
            sayClosed(playback)
            return
        }
        enter(playback)
        if (loud(op)) note("#$playback ${describe(op)}")
        if (op is PlayerOp.Seek) restartCountdown(if (op.absolute) op.seconds else playhead + op.seconds)
        when (val step = HostedStep.of(op)) {
            is HostedStep.Open -> open(playback, step.fullScreen, step.speed, step.volume)
            is HostedStep.Call -> {
                if (mpv == null) {
                    note("#$playback refused ${describe(op)}: no player is open")
                    return
                }
                val call = step.call
                if (call is MpvCall.Load) {
                    loads.addLast(playback)
                    sources[playback] = call
                    hasPicture = false
                }
                if (call is MpvCall.Set) settings[call.property] = call
                if (call is MpvCall.Shaders) settings["glsl-shaders"] = call
                enqueue(call)
            }
            is HostedStep.FullScreen -> fullScreen = step.on ?: !fullScreen
            HostedStep.Quit -> close(reporting = true)
            // A file that never showed a frame may be a playback that failed: the starting screen
            // stays up for the engine's verdict, which follows the close.
            HostedStep.Close -> close(reporting = false, awaitingVerdict = !hasPicture)
        }
    }

    internal fun apply(ui: PlayerUi, playback: ULong) {
        if (!accepts(playback)) {
            sayClosed(playback)
            return
        }
        enter(playback)
        val now = now()
        when (ui) {
            is PlayerUi.Starting -> {
                note("#$playback starting episode ${ui.episode}")
                starting = StartingScreen(playback, ui.id, ui.title, ui.episode, ui.episodeTitle, ui.coverUrl, ui.bannerUrl, now)
                ui.coverUrl?.let { coverUrl = it }
                ui.bannerUrl?.let { bannerUrl = it }
                present()
                changed()
            }
            is PlayerUi.Episode -> {
                episode = EpisodeOnScreen(ui.number, ui.label, ui.title, ui.show, ui.translation)
                changed()
            }
            is PlayerUi.Menus -> menus = PlayerMenus(ui.episodes, ui.sources, ui.translation, ui.upscaling, ui.toneMapping, ui.nightMode)
            is PlayerUi.Skip -> skip = SkipOffer(ui.kind, ui.to, now)
            PlayerUi.SkipCleared -> skip = null
            is PlayerUi.AutoSkipped -> undo = UndoOffer(ui.kind, ui.undoTo, from = null, since = now)
            is PlayerUi.UpNext -> {
                note("#$playback up next: episode ${ui.next.number}, auto ${ui.auto}, from ${ui.endingAt ?: "near the end"}, ${ui.countdown} s")
                stillWatchingAfter = ui.stillWatchingAfter.toInt()
                val before = upNext
                upNext = UpNextCard(
                    ui.next, ui.auto, ui.endingAt, ui.countdown.toDouble(), ui.stillWatchingAfter.toInt(),
                    cancelled = before?.next?.number == ui.next.number && before.cancelled,
                    played = before?.next?.number == ui.next.number && before.played,
                    restartedAt = before?.takeIf { it.next.number == ui.next.number }?.restartedAt,
                    sought = before?.next?.number == ui.next.number && before.sought,
                )
            }
            PlayerUi.UpNextCleared -> upNext = null
            is PlayerUi.Stall -> {
                note("#$playback stalled ${ui.seconds} s${if (stallWaived) ", waived" else ""}")
                if (!stallWaived) stall = StallCard(ui.seconds.toLong(), now)
            }
            PlayerUi.StallCleared -> {
                stall = null
                stallWaived = false
            }
            is PlayerUi.Resumed -> resume = ResumeNote(ui.position)
            is PlayerUi.Picture -> {
                upscaler = ui.upscaler
                hdr = ui.hdr
            }
            is PlayerUi.Thumbnails -> thumbnails = ui.url
            is PlayerUi.Alert -> alert = PlayerAlert(ui.text, ui.kind, now)
        }
    }

    /**
     * Whether ops and interface for [playback] are still taken: not once it is over, nor any at all
     * once the viewer has closed the player, until a playback is staged again.
     */
    private fun accepts(playback: ULong): Boolean =
        !closedByViewer && (closedThrough?.let { playback > it } ?: true)

    /**
     * Tell the engine this player is closed for [playback], once: what it says to a player that is
     * gone is refused, and a playback that opened one here, as the next episode found after the
     * viewer closed it can, ends as closed rather than waiting on it.
     */
    private fun sayClosed(playback: ULong) {
        if (!closedSaid.add(playback)) return
        send(Command.PlayerReport(playback, PlayerReport.Closed))
    }

    /** The engine speaks of [playback]: a newer one starts with none of the last one's cards. */
    private fun enter(playback: ULong) {
        if (this.playback?.let { playback > it } == false) return
        this.playback = playback
        stagedAhead = false
        skip = null
        undo = null
        upNext = null
        stall = null
        stallWaived = false
        resume = null
        alert = null
        if (rollPending) {
            rollPending = false
            rolledOn += 1
        }
    }

    // Opening and closing

    private fun present() {
        if (isPresented) return
        isPresented = true
        clock = now()
        lastInput = clock
        startTicker()
        changed()
    }

    private fun open(playback: ULong, fullScreen: Boolean, speed: Double?, volume: Double?) {
        if (mpv == null) {
            opened = playback
            this.fullScreen = fullScreen
            start(hwdec)
            report(PlayerReport.Power(power().onBattery, power().lowPower))
            display()?.let(::surfaceDisplay)
        }
        speed?.let { enqueue(MpvCall.Set("speed", MpvValue.Real(it))) }
        volume?.let { enqueue(MpvCall.Set("volume", MpvValue.Real(it))) }
        // Opened with the app in the background: the audio alone until it comes back.
        if (background) enqueue(MpvCall.Set("vid", MpvValue.Text("no")))
        present()
    }

    /** A new mpv, decoding with [decoding], and the worker that carries out what is asked of it. */
    private fun start(decoding: String) {
        val mpv = backend()
        this.mpv = mpv
        this.decoding = decoding
        hasMpv = true
        mpv.listener = { event -> handle(event, mpv) }
        val work = Channel<Work>(Channel.UNLIMITED)
        this.work = work
        worker = scope.launch {
            try {
                mpv.begin(decoding, captionOptions())
            } catch (e: Exception) {
                Log.e(TAG, "mpv did not start: $e")
                failed(mpv)
                return@launch
            }
            note("mpv started, decoding with $decoding")
            surface?.let { surface ->
                mpv.attach(surface)
                surfaceSize?.let { (w, h) -> mpv.resize(w, h) }
            }
            for (item in work) {
                when (item) {
                    is Work.Call -> try {
                        val call = if (item.call is MpvCall.Load) ready(item.call) else item.call
                        mpv.perform(call)?.let(::report)
                    } catch (e: Exception) {
                        if (e is kotlinx.coroutines.CancellationException) throw e
                        Log.w(TAG, "mpv refused ${item.call::class.simpleName}: $e")
                    }
                }
            }
        }
    }

    /**
     * A Load, once there is a surface to decode into: a MediaCodec opened before one is there
     * wedges mpv ("Both surface and native_window are NULL"), as mpv-android found. One that waits
     * longer than [PlayerTiming.SURFACE_WAIT] opens anyway, decoding in software.
     */
    private suspend fun ready(load: MpvCall.Load): MpvCall.Load {
        if (!zeroCopy(decoding) || surfaced.value || background) return load
        val came = withTimeoutOrNull(PlayerTiming.SURFACE_WAIT) { surfaced.first { it } }
        if (came != null) return load
        note("no surface to decode into yet: this file decodes in software")
        return load.copy(options = load.options + ("hwdec" to "no"))
    }

    /** Whether [hwdec] may decode into the surface itself, which needs one to be there. */
    private fun zeroCopy(hwdec: String): Boolean =
        hwdec.split(',').map(String::trim).any { it == "mediacodec" || it == "auto" || it == "yes" }

    /**
     * The viewer closed the player: its back arrow, the drag down, the back gesture.
     *
     * Nothing the engine says brings it back up until it stages another playback: a playback still
     * being found, the first or the next after Play now or an Episodes pick, is stopped, and should
     * its player open all the same, it is told the player closed.
     */
    fun viewerClosed() {
        input()
        closedByViewer = true
        if (failure != null) {
            // Nothing is playing: the screen only said so.
            dismiss()
            return
        }
        if (mpv == null) {
            if (isPresented) {
                // Nothing opened yet: stop what is being found.
                stopFinding()
                dismiss()
            }
            return
        }
        close(reporting = true)
        // The file on screen is closed; the next one staged since is still being found.
        if (stagedAhead) send(Command.Player(PlayerCommand.Stop))
    }

    /**
     * Stop what the engine is finding for this player: aimed at the playback on screen, unless one
     * has been staged since that has said nothing yet. Aimed at the one before, a Stop would miss
     * it, so it goes to whatever plays, which is the one staged last.
     */
    private fun stopFinding() {
        if (stagedAhead || playback == null) send(Command.Player(PlayerCommand.Stop)) else control(PlayerCommand.Stop)
    }

    /** Close mpv and the player. [reporting]: say the file ended by quitting, and that the player closed, first. */
    private fun close(reporting: Boolean, awaitingVerdict: Boolean = false) {
        note(if (reporting) "closing: the file ended by quitting" else "closing: the playback is over")
        if (reporting) {
            report(PlayerReport.Ended(PlayerEnd.QUIT))
            report(PlayerReport.Closed)
        }
        teardown(awaitingVerdict)
    }

    /** mpv would not start, or went away by itself: the player is gone. */
    private fun failed(mpv: PlayerBackend) {
        if (mpv !== this.mpv) return
        report(PlayerReport.Closed)
        teardown()
    }

    private fun stopMpv() {
        watchdog?.cancel()
        watchdog = null
        work?.close()
        work = null
        worker?.cancel()
        worker = null
        mpv?.let { gone ->
            gone.listener = null
            scope.launch { withContext(NonCancellable) { gone.end() } }
        }
        mpv = null
        hasMpv = false
    }

    private fun teardown(awaitingVerdict: Boolean = false) {
        stopMpv()
        closedThrough = listOfNotNull(playback, current, opened, loads.maxOrNull()).maxOrNull()
        opened = null
        current = null
        loads.clear()
        sources.clear()
        settings.clear()
        thumbnails = null
        if (!awaitingVerdict || !isPresented) {
            dismiss()
            return
        }
        // mpv is gone; the screen waits a moment for how the playback ended.
        verdictPending = true
        scope.launch {
            delay(PlayerTiming.VERDICT_WAIT)
            if (verdictPending) {
                verdictPending = false
                dismiss()
            }
        }
    }

    /** The player comes down, with everything it showed. */
    private fun dismiss() {
        isPresented = false
        failure = null
        choosingSource = false
        sourceChoices = null
        verdictPending = false
        fullScreen = false
        staged = null
        starting = null
        episode = null
        menus = null
        skip = null
        undo = null
        upNext = null
        stall = null
        resume = null
        alert = null
        stillWatching = false
        coverUrl = null
        bannerUrl = null
        resetFile()
        ticker?.cancel()
        ticker = null
        holds = emptySet()
        scrubbing = null
        changed()
    }

    private fun resetFile() {
        position = 0.0
        playhead = 0.0
        duration = null
        paused = false
        chapters = emptyList()
        tracks = emptyList()
        audioTrack = null
        subtitleTrack = null
        bufferingSince = null
        hasPicture = false
        aspect = null
        videoHeight = null
        pixelFormat = null
        videoCodec = null
        droppedFrames = 0L
    }

    // mpv's events

    private fun handle(event: MpvEvent, from: PlayerBackend) {
        if (from !== mpv) return
        when (event.id) {
            MpvNative.Event.START_FILE -> {
                loads.removeFirstOrNull()?.let { current = it }
                note("#$current mpv started the file")
                val keepPause = paused
                resetFile()
                paused = keepPause
            }
            // A frame of the file being replaced, while the next is loading, is not the next one's.
            MpvNative.Event.PLAYBACK_RESTART -> if (!hasPicture && loads.isEmpty()) firstFrame()
            MpvNative.Event.FILE_LOADED -> {
                note("#$current file loaded")
                // From here, not from the Load: a slow network, or a torrent still finding its
                // peers, is no stalled decoder.
                if (!hasPicture) watch(from)
            }
            MpvNative.Event.END_FILE -> note("#$current file ended: ${event.value["reason"].string}")
            MpvNative.Event.SHUTDOWN -> {
                note("mpv went away by itself")
                failed(from)
                return
            }
            MpvNative.Event.LOG_MESSAGE -> disabledHook(event)?.let { report(PlayerReport.HookDisabled(it)) }
            MpvNative.Event.PROPERTY_CHANGE -> update(event.name ?: return, event.value)
        }
        reportOf(event)?.let(::report)
    }

    /** The file's first picture is up: the starting screen fades from it. */
    private fun firstFrame() {
        hasPicture = true
        watchdog?.cancel()
        watchdog = null
        note("#$current first frame")
        val starting = starting
        if (starting != null && (current?.let { it >= starting.playback } != false)) this.starting = null
        if (stillWatchingAfter > 0 && rolledOn >= stillWatchingAfter && !stillWatching) {
            stillWatching = true
            if (!paused) control(PlayerCommand.TogglePause)
        }
        changed()
    }

    private fun update(name: String, value: MpvValue) {
        when (name) {
            "time-pos" -> {
                val time = value.double ?: return
                playhead = time
                if (scrubbing == null && (kotlin.math.abs(time - position) >= 0.25 || time < position)) position = time
                played()
            }
            "duration" -> {
                duration = value.double
                changed()
            }
            "pause" -> {
                paused = value.bool ?: paused
                changed()
            }
            "speed" -> {
                speed = value.double ?: speed
                changed()
            }
            "volume" -> volume = value.double ?: volume
            "chapter-list" -> chapters = value.array.map(Chapter::of)
            "track-list" -> tracks = value.array.mapNotNull(Track::of)
            "video-params" -> {
                val w = value["dw"].double ?: value["w"].double
                val h = value["dh"].double ?: value["h"].double
                if (w != null && h != null && h > 0) aspect = w / h
                videoHeight = value["h"].double?.toInt()
                pixelFormat = value["hw-pixelformat"].string ?: value["pixelformat"].string
            }
            "current-tracks/video" -> videoCodec = value["codec"].string
            "frame-drop-count" -> droppedFrames = value.long ?: 0L
            "aid" -> audioTrack = value.long
            "sid" -> subtitleTrack = value.long
            "paused-for-cache" -> {
                val waiting = value.bool ?: false
                bufferingSince = if (waiting) bufferingSince ?: now() else null
                changed()
            }
            "seeking" -> if (value.bool == false) {
                position = playhead
                changed()
            }
        }
    }

    /**
     * The playhead moved: the cards that run on playback time. Not Up Next's countdown while the
     * viewer scrubs, whose keyframe seeks move the playhead under the finger.
     */
    private fun played() {
        val card = upNext
        if (card != null && !card.played && !assisted && scrubbing == null) {
            val left = card.remaining(playhead, duration)
            if (left != null && left <= 0) {
                upNext = card.copy(played = true)
                // Asked for by someone there when they sought into the ending: not one in a row with
                // nobody watching.
                rollPending = !card.sought
                control(PlayerCommand.PlayNext)
            }
        }
        undo?.let { note ->
            val from = note.from
            if (from == null) {
                undo = note.copy(from = playhead)
            } else if (playhead - from >= PlayerTiming.UNDO_PLAYBACK || playhead < from - 1) {
                undo = null
            }
        }
        resume?.let { note ->
            if (playhead >= note.position + PlayerTiming.RESUME_PLAYBACK || playhead < note.position - 1) resume = null
        }
    }

    /**
     * A seek, the viewer's or the engine's, to [target]: an Up Next countdown it lands in or past runs
     * again from its full length there, rather than playing the next episode at once.
     */
    private fun restartCountdown(target: Double) {
        val card = upNext ?: return
        if (card.played) return
        val start = card.showsAt(duration) ?: return
        upNext = card.copy(restartedAt = target.takeIf { it >= start })
    }

    /** The viewer sought: Up Next, if it is up, plays next for someone who is there. */
    private fun sought() {
        val card = upNext ?: return
        if (!card.played) upNext = card.copy(sought = true)
    }

    // The decoder watchdog

    /**
     * Once the file has loaded, while mpv decodes in hardware: a file that has no picture for
     * [PlayerTiming.FIRST_FRAME_WATCHDOG] while mpv is not waiting for data has a decoder that
     * stalled, which takes mpv's core with it (AS5). mpv is asked to decode in software; if it
     * cannot even answer that, a new mpv takes over, on a new surface, where the file was.
     */
    private fun watch(mpv: PlayerBackend) {
        if (decoding == "no") return
        watchdog?.cancel()
        watchdog = scope.launch {
            var waited = 0L
            while (isActive && !hasPicture) {
                delay(500)
                waited = if (bufferingSince != null) 0 else waited + 500
                if (waited >= PlayerTiming.FIRST_FRAME_WATCHDOG) break
            }
            if (hasPicture || mpv !== this@PlayerModel.mpv) return@launch
            Log.w(TAG, "no picture ${PlayerTiming.FIRST_FRAME_WATCHDOG} ms after the load, decoding with $decoding: decoding in software")
            if (mpv.decodeInSoftware(timeoutMs = 2_000)) {
                decoding = "no"
                return@launch
            }
            replace()
        }
    }

    /** A new mpv in place of one that stopped answering, decoding in software, where the file was. */
    private fun replace() {
        val playing = current ?: loads.firstOrNull() ?: return
        val load = sources[playing] ?: return
        Log.w(TAG, "mpv stopped answering: opening #$playing again in a new mpv")
        val kept = settings.values.toList()
        val at = playhead
        stopMpv()
        // The stalled mpv holds the surface; the screen makes a new one for the next.
        surfaceGeneration += 1
        surface = null
        surfaced.value = false
        start("no")
        loads.clear()
        loads.addLast(playing)
        val options = load.options + ("hwdec" to "no") + if (at > 1) mapOf("start" to at.toString()) else emptyMap()
        enqueue(MpvCall.Load(load.url, options))
        kept.forEach(::enqueue)
    }

    // Commands

    /**
     * Steer the playback the player shows, and only it (`Command::Control`): a Play now or an Undo
     * sent as an episode rolls on cannot reach the next one. Before the engine has named one,
     * whatever plays.
     */
    private fun control(command: PlayerCommand) {
        val playback = playback
        send(if (playback != null) Command.Control(playback, command) else Command.Player(command))
    }

    // Reports

    /** Tell the engine, for the playback whose file mpv has. */
    private fun report(report: PlayerReport) {
        val target = current ?: opened ?: return
        if (report == PlayerReport.Closed) closedSaid += target
        send(Command.PlayerReport(target, report))
    }

    /** The device's power changed: on battery or off, power saver. */
    fun powerChanged() {
        if (mpv == null) return
        val power = power()
        report(PlayerReport.Power(power.onBattery, power.lowPower))
    }

    // The surface

    /** The SurfaceView's surface is there: mpv draws into it. */
    fun surfaceCreated(surface: Surface) {
        this.surface = surface
        surfaced.value = true
        mpv?.attach(surface)
    }

    /** The surface is [width] × [height] now: mpv's android context is told every time. */
    fun surfaceChanged(width: Int, height: Int) {
        surfaceSize = width to height
        mpv?.resize(width, height)
    }

    /** The surface is going: mpv lets it go before this returns. Playback carries on without a picture. */
    fun surfaceDestroyed() {
        surface = null
        surfaced.value = false
        mpv?.detach()
    }

    /** The screen the surface is on. libplacebo has no compute shaders on GLES, Android's path. */
    fun surfaceDisplay(display: DisplayInfo) {
        if (mpv == null) return
        display.refreshHz?.let { enqueue(MpvCall.Set("display-fps-override", MpvValue.Real(it))) }
        report(
            PlayerReport.Display(
                display.width.toUInt(), display.height.toUInt(), display.scale, display.refreshHz,
                edrHeadroom = null, compute = false,
            ),
        )
    }

    private fun enqueue(call: MpvCall) {
        work?.trySend(Work.Call(call))
    }

    // The clock

    private fun startTicker() {
        ticker?.cancel()
        ticker = scope.launch {
            while (isActive) {
                delay(250)
                tick()
            }
        }
    }

    private fun tick() {
        val now = now()
        clock = now
        undo?.let { if (now - it.since >= PlayerTiming.UNDO_PAUSED) undo = null }
        alert?.let { if (now - it.since >= PlayerTiming.ALERT_STAYS) alert = null }
    }

    // What the screens read

    /** Whether the controls are up: after input, while paused, while something holds them. */
    val controlsVisible: Boolean
        get() = paused || assisted || holds.isNotEmpty() || clock - lastInput < PlayerTiming.CONTROLS_HIDE

    /** The starting screen's line: "Starting episode 7", and after a while "Finding a source · 4 s". */
    fun startingLine(note: String? = null): String? = starting?.line(clock, note)

    /**
     * The thin buffering line along the video's top edge: mid-episode, once mpv has waited for data
     * [PlayerTiming.BUFFERING_AFTER], or while the engine says the stream has stalled. It goes the
     * moment playback goes on; the starting screen says its own wait.
     */
    val showsBuffering: Boolean
        get() {
            if (starting != null || !hasPicture) return false
            if (stall != null) return true
            val since = bufferingSince ?: return false
            return clock - since >= PlayerTiming.BUFFERING_AFTER
        }

    /** The skip button has been up long enough to dim. */
    val skipDimmed: Boolean get() = !assisted && skip?.let { clock - it.since >= PlayerTiming.SKIP_DIMS } ?: false

    /** Up Next is on screen: from the ending, unless cancelled. */
    val upNextShown: Boolean
        get() {
            val card = upNext ?: return false
            if (card.cancelled) return false
            val at = card.showsAt(duration) ?: return false
            return position >= at
        }

    /** "Plays in 8 s", while the countdown runs, and how far it has run, 0 to 1. */
    val countdown: Pair<Int, Float>?
        get() {
            if (!upNextShown || assisted) return null
            val card = upNext ?: return null
            val left = card.remaining(position, duration) ?: return null
            val progress = 1 - left / maxOf(card.countdown, 1.0)
            return kotlin.math.ceil(left).toInt() to progress.toFloat().coerceIn(0f, 1f)
        }

    /** The one card in the corner: the stall prompt, then Undo, then Up Next, then Skip. */
    enum class Corner { Stall, Undo, UpNext, Skip }

    val corner: Corner?
        get() = when {
            stall != null -> Corner.Stall
            undo != null -> Corner.Undo
            upNextShown -> Corner.UpNext
            skip != null -> Corner.Skip
            else -> null
        }

    /** The seek line's stretches, from the file's chapters. */
    val segments: List<SeekSegment> get() = seekSegments(chapters, duration)

    /** What the seek preview opens: the `Thumbnails` address, or the stream playing, with the options that reach it. */
    val previewSource: MpvCall.Load?
        get() {
            val stream = current?.let { sources[it] } ?: return thumbnails?.let { MpvCall.Load(it, emptyMap()) }
            val reaching = stream.options.filterKeys { name ->
                listOf("header", "user-agent", "referrer", "cookie", "-lavf-o", "tls-").any { it in name }
            }
            return MpvCall.Load(thumbnails ?: stream.url, reaching)
        }

    /** The chapter at [seconds], by its name. */
    fun chapter(seconds: Double): String? =
        chapters.sortedBy { it.time }.lastOrNull { it.time <= seconds }?.title?.takeIf { it.isNotEmpty() }

    val audioTracks: List<Track> get() = tracks.filter { it.kind == Track.Kind.Audio }
    val subtitleTracks: List<Track> get() = tracks.filter { it.kind == Track.Kind.Subtitle }

    /** The header's lines: the show, and the episode. */
    val showTitle: String get() = episode?.show ?: staged?.title ?: starting?.title ?: ""
    val episodeLine: String
        get() {
            episode?.let { return it.line }
            val number = staged?.episode ?: starting?.episode ?: ""
            val title = staged?.episodeTitle ?: starting?.episodeTitle
            return if (title != null) "Episode $number · $title" else "Episode $number"
        }

    /** The title playing, for the art and the way back. */
    val titleId: AnilistId? get() = staged?.id ?: starting?.id

    /** The episode starting, as the starting screen names it. */
    val startingEpisode: String? get() = starting?.episode ?: staged?.episode ?: failure?.episode

    // When it could not play

    /** Ask the engine for the episode's releases, to pick one: from "Couldn't play episode 7", or a start that waits. */
    fun chooseSource() {
        input()
        val id = staged?.id ?: failure?.id ?: return
        val episode = startingEpisode ?: return
        choosingSource = true
        sourceChoices = null
        send(Command.LoadSources(id, episode))
    }

    /** Play [candidate] instead: the engine stages it as a playback of its own. */
    fun play(candidate: SourceCandidate) {
        input()
        val id = staged?.id ?: failure?.id ?: return
        val episode = startingEpisode ?: return
        choosingSource = false
        sourceChoices = null
        failure = null
        send(Command.PlaySource(id, episode, candidate.providerId, candidate.id))
    }

    fun cancelChoosingSource() {
        choosingSource = false
        sourceChoices = null
    }

    // The viewer's actions

    /** A tap, drag or key: the controls come up. */
    fun poke() {
        lastInput = now()
        clock = lastInput
    }

    /** A key on the TV's remote or a phone's: somebody is there, for "Still watching?". */
    fun viewerThere() = input()

    /** Something the viewer did: no longer nobody there. */
    private fun input() {
        poke()
        rolledOn = 0
    }

    /** A tap on the video while the controls are up: they go now. */
    fun hideControls() {
        lastInput = now() - 60_000
        clock = now()
    }

    fun hold(reason: String, on: Boolean) {
        holds = if (on) holds + reason else holds - reason
        poke()
    }

    fun togglePause() {
        input()
        control(PlayerCommand.TogglePause)
    }

    /** Play, or pause, whichever it is not: the media session's play and pause. */
    fun setPlaying(playing: Boolean) {
        if (paused == !playing) return
        togglePause()
    }

    fun seekBy(seconds: Double) {
        input()
        sought()
        control(PlayerCommand.Seek(seconds))
    }

    fun seekTo(seconds: Double) {
        input()
        sought()
        control(PlayerCommand.SeekTo(seconds))
    }

    /** The finger on the seek line: keyframe seeks while it moves, an exact one on release. */
    fun scrub(seconds: Double) {
        if (scrubbing == null) hold("scrub", true)
        scrubbing = seconds
        position = seconds
        // A keyframe seek each half second of movement: mpv drops the ones it cannot keep up with.
        scrubSent?.let { if (kotlin.math.abs(it - seconds) < 0.5) return }
        scrubSent = seconds
        enqueue(MpvCall.Seek(seconds, SeekMode.Keyframe))
    }

    fun endScrub() {
        val seconds = scrubbing ?: return
        scrubbing = null
        scrubSent = null
        hold("scrub", false)
        seekTo(seconds)
    }

    /**
     * Play at [target] times normal speed: set outright, whatever the speed is now, as the speed
     * menu picks it. Not a factor of the speed this side last heard, which the engine multiplies
     * by whatever plays then.
     */
    fun playAt(target: Double) {
        input()
        if (target <= 0) return
        control(PlayerCommand.SpeedTo(target))
    }

    /** An audio or subtitle track, by its `track-list` id, or the subtitles off: kept for the series. */
    fun selectAudio(id: Long) {
        input()
        control(PlayerCommand.ChooseTrack(TrackKind.AUDIO, id))
    }

    fun selectSubtitle(id: Long?) {
        input()
        control(PlayerCommand.ChooseTrack(TrackKind.SUBTITLE, id))
    }

    fun takeSkip() {
        input()
        sought()
        skipsTaken += 1
        skip = null
        control(PlayerCommand.TakeSkip)
    }

    fun undoSkip() {
        input()
        val note = undo ?: return
        undo = null
        control(PlayerCommand.UndoSkip(note.undoTo))
    }

    fun playNext() {
        input()
        val card = upNext ?: return
        if (card.played) return
        upNext = card.copy(played = true)
        control(PlayerCommand.PlayNext)
    }

    fun cancelNext() {
        input()
        val card = upNext ?: return
        upNext = card.copy(cancelled = true)
        control(PlayerCommand.CancelNext)
    }

    fun choose(choice: PlayerChoice) {
        input()
        control(PlayerCommand.Choose(choice))
    }

    fun setUpscaling(id: String) {
        input()
        control(PlayerCommand.Upscaling(id))
    }

    fun setNightMode(on: Boolean) {
        input()
        control(PlayerCommand.NightMode(on))
    }

    /** The next (`1`) or previous (`-1`) episode. */
    fun step(by: Long) {
        input()
        control(PlayerCommand.Step(by))
    }

    fun switchSource() {
        input()
        stall = null
        control(PlayerCommand.StallSwitch)
    }

    fun keepWaiting() {
        input()
        stall = null
        stallWaived = true
        control(PlayerCommand.StallWait)
    }

    fun startOver() {
        input()
        resume = null
        control(PlayerCommand.SeekTo(0.0))
    }

    /** "Still watching?": Continue watching. */
    fun continueWatching() {
        input()
        stillWatching = false
        if (paused) control(PlayerCommand.TogglePause)
    }

    /** "Still watching?": Stop. */
    fun stopWatching() {
        stillWatching = false
        viewerClosed()
    }

    /**
     * Lift the subtitles to [percent] of the picture's height from its top, above the controls
     * while they are up: `sub-pos`, which libass applies to the dialogue and leaves signs alone.
     */
    fun liftSubtitles(percent: Double) {
        enqueue(MpvCall.Set("sub-pos", MpvValue.Real(percent.coerceIn(50.0, 100.0))))
    }

    /** The app went to the background, or came back: the audio plays on, the video track is off meanwhile. */
    fun backgrounded(background: Boolean) {
        if (this.background == background) return
        this.background = background
        if (mpv == null) return
        note(if (background) "in the background: the audio plays on, the video track off" else "back: the video track on")
        enqueue(MpvCall.Set("vid", MpvValue.Text(if (background) "no" else "auto")))
    }

    // Caption style

    /** Plain-text subtitles follow the system's caption style: the viewer's setting, on by default. */
    var followsSystemCaptions = true
        set(value) {
            if (field == value) return
            field = value
            captionStyleChanged()
        }

    /** mpv's options for plain-text subtitles, as the setting and the system's style have them. */
    fun captionOptions(): Map<String, String> =
        (if (followsSystemCaptions) systemCaptionStyle() else null)?.mpvOptions() ?: CaptionStyle.MPV_DEFAULTS

    /**
     * The setting or the system's caption style changed: the mpv playing now takes the new look;
     * the next one starts with it.
     */
    fun captionStyleChanged() {
        if (mpv == null) return
        val options = captionOptions()
        note("caption style: ${if (options == CaptionStyle.MPV_DEFAULTS) "mpv's own" else "the system's"}, ${options["sub-border-style"]}")
        for ((name, value) in options.toSortedMap()) enqueue(MpvCall.Set(name, MpvValue.Text(value)))
    }

    /** "Still watching?" now, as after three episodes in a row: for screenshots. */
    fun askStillWatching() {
        stillWatching = true
    }

    private fun changed() {
        onSessionChange?.invoke()
    }

    companion object {
        const val TAG = "player"

        /** The ops worth a line in the log: the ones that change what plays. */
        private fun loud(op: PlayerOp): Boolean = op is PlayerOp.Open || op is PlayerOp.Load || op == PlayerOp.Stop ||
            op == PlayerOp.Quit || op == PlayerOp.Close || op is PlayerOp.Shaders

        /** An op as the log says it: names, never header values, paths or queries. */
        private fun describe(op: PlayerOp): String = when (op) {
            is PlayerOp.Open -> "open${if (op.fullscreen) " full screen" else ""}"
            is PlayerOp.Load -> "load ${op.url.substringBefore('?').substringAfterLast('/')} with ${op.options.map { it.name }.sorted().joinToString()}"
            is PlayerOp.AddSubtitle -> "add a subtitle track (${op.language ?: "no language"})"
            is PlayerOp.Set -> "set ${op.property}"
            is PlayerOp.Cycle -> "cycle ${op.property}"
            PlayerOp.ToggleFullscreen -> "toggle full screen"
            is PlayerOp.Add -> "add ${op.delta} to ${op.property}"
            is PlayerOp.Seek -> if (op.absolute) "seek to ${op.seconds.toInt()} s" else "seek by ${op.seconds.toInt()} s"
            is PlayerOp.Shaders -> if (op.paths.isEmpty()) "shaders off" else "shaders: ${op.paths.joinToString { it.substringAfterLast('/') }}"
            is PlayerOp.AudioFilter -> if (op.add) "add an audio filter" else "remove an audio filter"
            PlayerOp.RestateChapters -> "restate the chapters"
            PlayerOp.Stop -> "stop"
            PlayerOp.Quit -> "quit"
            PlayerOp.Close -> "close"
        }

        private fun note(message: String) {
            Log.i(TAG, message)
        }
    }
}
