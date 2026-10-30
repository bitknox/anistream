// The fixtures' TVs with Chromecast built in: made-up Cast records on loopback addresses no TV
// answers on, handed to the engine's own Cast browse so the picker's rows are the engine's, and a
// scripted TV behind the card that plays, pauses, seeks, mutes and steps its volume as a
// receiver does, without any network. Nothing here ever reaches a real TV.

package tv.anistream.store

import android.content.Context
import android.util.Log
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import tv.anistream.engine.ffi.FfiCastBrowser
import tv.anistream.engine.ffi.FfiCastCard
import tv.anistream.engine.ffi.FfiCastRouteHint
import tv.anistream.engine.ffi.FfiCastUndo
import tv.anistream.engine.ffi.FfiCastUpNext
import tv.anistream.engine.ffi.FfiCastSink
import tv.anistream.engine.ffi.FfiLinkPhase
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiRemoteCard
import tv.anistream.engine.ffi.FfiScreen
import tv.anistream.engine.ffi.FfiSkipCard
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiTvException
import tv.anistream.engine.ffi.FfiTvTrack
import tv.anistream.engine.ffi.FfiTvTrackKind
import tv.anistream.engine.ffi.FfiTxtPair
import tv.anistream.engine.ffi.FfiVolumeControl
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.SkipKind
import tv.anistream.i18n.Strings

/**
 * What the cast shots open on, the fixtures' `cast` extra:
 *
 *     picker     Play on TV for the first fixture's episode 7: Bedroom runs anistream, Living room
 *                and Kitchen display have Chromecast built in
 *     searching  the picker before any TV answers, past its quiet 600 ms
 *     none       the picker after 10 s with no TV found
 *     blocked    the picker with Android 17's local network permission turned down
 *     starting   Living room's card, 6 s into starting: "Waiting for Living room · 6 s"
 *     preparing  Living room's card, 6 s in, this device converting: "Preparing the stream · 6 s"
 *     fixed      Living room's card playing, its volume fixed: Mute alone
 *     level      Kitchen display's card in the opening, with a volume level and the skip offer
 *     failed     Living room's card once every stream was refused
 *     never      Living room's card after the TV never read the stream: Try again and Play here
 *     network    Living room's card when this device is not on the TV's network
 *     codec      Living room's card when the TV can't play the video
 *     deeplink   Living room's card when the episode opens in another app
 *     upnext     Living room's card near the end: Up next, plays in 6 s
 *     undo       Kitchen display's card after an automatic skip: "Skipped opening · Undo"
 *     audio      Living room's card loading again after an audio change
 *     lost       Living room's card after the TV dropped off
 *     taken      Library, after someone else took Living room: the notice with Play here
 *     bar        Library while Living room plays, the card closed: the bar over the navigation
 */
enum class CastScene {
    PICKER, SEARCHING, NONE, BLOCKED, STARTING, PREPARING, FIXED, LEVEL, FAILED, NEVER, NETWORK, CODEC, DEEPLINK,
    UPNEXT, UNDO, AUDIO, LOST, TAKEN, BAR;

    /** No TV of either kind answers in this scene. */
    val empty: Boolean get() = this == SEARCHING || this == NONE

    companion object {
        fun named(name: String?): CastScene? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

/** A made-up Cast record: what NsdManager would hand over for a TV with Chromecast built in. */
private data class CastRecord(val instance: String, val id: String, val name: String, val model: String, val address: String)

/** The made-up TVs with Chromecast built in, on loopback addresses no fake anistream TV is on. */
private val RECORDS = listOf(
    CastRecord("Chromecast-fixture-living", "fixture-living", "Living room", "Chromecast", "127.0.0.2"),
    CastRecord("Chromecast-fixture-kitchen", "fixture-kitchen", "Kitchen display", "Google Nest Hub", "127.0.0.3"),
)

/** A browse that finds the fixtures' TVs with Chromecast built in, or none. */
class FixtureCastBrowser(private val found: Boolean) : FfiCastBrowser {
    override fun start(sink: FfiCastSink) {
        if (!found) return
        for (record in RECORDS) {
            // `ca` 4101: video out, as a TV's record says.
            val txt = listOf("id" to record.id, "fn" to record.name, "md" to record.model, "ca" to "4101")
                .map { (key, value) -> FfiTxtPair(key, value) }
            sink.found(record.instance, listOf(record.address), 8009u, txt)
        }
    }

    override fun stop() {}
}

/**
 * A TV with Chromecast built in, scripted: it starts after [startsAfter] ms (never, for `null`),
 * then plays from [from], on its own clock, answering the card's commands as the receiver would.
 */
class FixtureCastLink(
    override val tvName: String,
    private val scope: CoroutineScope,
    private val now: FfiNowCard,
    private val control: FfiVolumeControl,
    startsAfter: Long?,
    private var from: Double,
    private val skipLabel: String?,
    /** The line for an episode that has not started yet, from the scene. */
    private val phase: Msg? = null,
    /** The Up next card, when the scene is near the end. */
    private val upNext: FfiCastUpNext? = null,
    /** The Undo note, when the scene follows an automatic skip. */
    private val undo: FfiCastUndo? = null,
) : CastLink {
    private val changes = MutableStateFlow(0L)
    private var seen = 0L
    private var loading = true
    private var paused = false
    private var muted = false
    private var volume = 45.0
    private var subtitlesOff = false
    private var subtitle = 1L
    private var audio = 1L
    private var notice: Msg? = null
    private var over = false
    private var played = false
    private var reloading = false
    private var upNextShown = upNext != null
    private var undoShown = undo != null
    private var connected = false
    private var since = System.nanoTime()
    private val stopped = CompletableDeferred<Unit>()

    init {
        scope.launch {
            delay(200)
            connected = true
            bump()
            if (startsAfter == null) return@launch
            delay(startsAfter)
            loading = false
            played = true
            since = System.nanoTime()
            bump()
        }
    }

    private fun bump() {
        changes.value += 1
    }

    private fun position(): Double {
        if (loading || paused) return from
        return from + (System.nanoTime() - since) / 1e9
    }

    /** The TV refuses every stream, or drops off: the card says so, and the cast is over. */
    fun end(with: Msg?) {
        from = position()
        notice = with
        over = true
        connected = with !is Msg.NoticeCastLost && connected
        bump()
    }

    override fun card(): FfiCastCard {
        val audioTracks = listOf("ja", "en").mapIndexed { index, language ->
            FfiTvTrack(index + 1L, language, null, audio == index + 1L)
        }
        val subtitleTracks = listOf(FfiTvTrack(1L, "en", null, !subtitlesOff && subtitle == 1L))
        val playing = now.copy(
            loading = loading,
            position = position(),
            paused = paused,
            volume = if (control == FfiVolumeControl.LEVEL) volume else null,
            audio = if (!played) emptyList() else audioTracks,
            subtitles = if (!played) emptyList() else subtitleTracks,
            subtitlesOff = subtitlesOff,
            skip = if (!loading && skipLabel != null && position() < 85.0) FfiSkipCard(85.0, skipLabel) else null,
        )
        return FfiCastCard(
            card = FfiRemoteCard(
                tvName = tvName,
                phase = if (connected) FfiLinkPhase.CONNECTED else FfiLinkPhase.CONNECTING,
                phaseLine = null,
                screen = FfiScreen.Player,
                now = playing,
                rev = changes.value.toULong(),
                over = over,
                reached = true,
            ),
            volumeControl = control,
            muted = if (connected) muted else null,
            notice = notice,
            phaseLine = if (loading && !over) phase ?: if (reloading) Msg.CastPhaseWaiting(tvName, 4L) else Msg.CastPhaseStarting(tvName) else null,
            upNext = upNext?.takeIf { upNextShown && !loading },
            undo = undo?.takeIf { undoShown && !loading },
            route = if (loading) null else FfiCastRouteHint.THROUGH_THIS_DEVICE,
        )
    }

    override suspend fun nextCard(): FfiCastCard? {
        if (stopped.isCompleted) return null
        changes.first { it != seen }
        seen = changes.value
        return card()
    }

    override fun command(command: FfiTvCommand) {
        if (over) return
        when (command) {
            FfiTvCommand.TogglePause -> {
                from = position()
                since = System.nanoTime()
                paused = !paused
            }
            is FfiTvCommand.Seek -> {
                from = (position() + command.by).coerceAtLeast(0.0)
                since = System.nanoTime()
            }
            is FfiTvCommand.SeekTo -> {
                from = command.position
                since = System.nanoTime()
            }
            FfiTvCommand.Skip -> {
                from = 85.0
                since = System.nanoTime()
            }
            is FfiTvCommand.Mute -> muted = command.muted
            is FfiTvCommand.Volume -> if (control == FfiVolumeControl.LEVEL) volume = (volume + command.step).coerceIn(0.0, 100.0)
            is FfiTvCommand.Track -> when (command.kind) {
                FfiTvTrackKind.SUBTITLE -> {
                    subtitlesOff = command.id == null
                    command.id?.let { subtitle = it }
                }
                // The TV loads the stream again where it was.
                FfiTvTrackKind.AUDIO -> command.id?.let {
                    audio = it
                    from = position()
                    loading = true
                    reloading = true
                    scope.launch {
                        delay(8_000)
                        since = System.nanoTime()
                        loading = false
                        reloading = false
                        bump()
                    }
                }
            }
            else -> return
        }
        bump()
    }

    override fun playHere(): Double? {
        val at = position()
        stop()
        return at.takeIf { it >= 5.0 }
    }

    override fun playNext() {
        upNextShown = false
        bump()
    }

    override fun cancelNext() {
        upNextShown = false
        bump()
    }

    override fun undoSkip() {
        undoShown = false
        from = 0.0
        since = System.nanoTime()
        bump()
    }

    override fun tryAgain(): Boolean {
        notice = null
        over = false
        loading = true
        bump()
        return true
    }

    override fun stop() {
        over = true
        stopped.complete(Unit)
        bump()
    }
}

object FixtureCasts {
    /** The scene the launch asked for, read as Play on TV starts: which TVs its browses find. */
    var scene: CastScene? = null

    /** The browse for TVs with Chromecast built in, for [scene]. */
    fun browser(): FfiCastBrowser = FixtureCastBrowser(found = scene?.empty != true)

    /**
     * Set the scripted TVs in for the engine's casts, and set up [scene] on [store] for the
     * first fixture's episode 7, whose title is [title].
     */
    suspend fun stage(context: Context, store: AppStore, scope: CoroutineScope, scene: CastScene, title: String) {
        val cast = store.cast
        // Bedroom, the fake TV that runs anistream, paired already, as on the board.
        val bedroom = (store.tv.attachedBrowser as? FixtureTvBrowser)?.tvs?.firstOrNull()
        val sender = store.tv.attachedSender
        if (bedroom != null && sender != null) {
            try {
                sender.pair(bedroom.pairLink(bedroom.openCode())).pair(null)
                store.tv.refresh()
            } catch (e: FfiTvException) {
                Log.w("anistream", "Bedroom did not pair: $e")
            }
        }
        val id = 900001u
        val episode = "7"
        val episodeTitle = store.titles.episodes.value[id]?.firstOrNull { it.number == episode }?.title
        val line = episodeTitle?.let { Strings.castEpisodeTitled(context, episode, it) } ?: Strings.castEpisode(context, episode)
        val request = TvPlayRequest(id, title, episode)
        val now = FfiNowCard(
            id = id, title = title, episode = episode, episodeLine = line, loading = true, position = 0.0,
            duration = 1420.0, paused = false, speed = 1.0, volume = null, audio = emptyList(), subtitles = emptyList(),
            subtitlesOff = false, skip = null,
        )
        var last: FixtureCastLink? = null
        cast.starter = { key, asked ->
            val kitchen = key.endsWith("fixture-kitchen")
            val name = if (kitchen) "Kitchen display" else "Living room"
            val (startsAfter, from) = when (scene) {
                CastScene.STARTING, CastScene.PREPARING, CastScene.FAILED, CastScene.NEVER, CastScene.NETWORK,
                CastScene.CODEC, CastScene.DEEPLINK,
                -> null to 0.0
                CastScene.LEVEL -> 300L to 58.0
                CastScene.UNDO -> 300L to 90.0
                CastScene.UPNEXT -> 300L to 1_395.0
                else -> 300L to (asked.from ?: 492.0)
            }
            FixtureCastLink(
                tvName = name,
                scope = scope,
                now = now.copy(title = asked.title.ifEmpty { title }),
                control = if (kitchen) FfiVolumeControl.LEVEL else FfiVolumeControl.MUTE_ONLY,
                startsAfter = startsAfter,
                from = from,
                skipLabel = Strings.playerSkipOpening(context),
                phase = when (scene) {
                    CastScene.STARTING -> Msg.CastPhaseWaiting(name, 6L)
                    CastScene.PREPARING -> Msg.CastPhasePreparing(6L)
                    else -> null
                },
                upNext = if (scene == CastScene.UPNEXT) FfiCastUpNext("8", "A Town Without Night", null, true, 6u) else null,
                undo = if (scene == CastScene.UNDO) FfiCastUndo(SkipKind.OPENING, 5.0) else null,
            ).also { last = it }
        }
        try {
            when (scene) {
                CastScene.PICKER, CastScene.SEARCHING, CastScene.NONE -> store.tv.offer(request)
                CastScene.BLOCKED -> {
                    cast.localNetwork(allowed = false)
                    store.tv.offer(request)
                }
                else -> {
                    val row = waitForRow(store, if (scene == CastScene.LEVEL || scene == CastScene.UNDO) "Kitchen display" else "Living room") ?: return
                    cast.cast(row, request)
                    // The engine keeps the found TVs until the browses stop: only once the cast has the row.
                    cast.stopBrowsing()
                    store.tv.stopBrowsing()
                    when (scene) {
                        CastScene.FAILED -> {
                            delay(600)
                            last?.end(Msg.NoticeCastRefused("Living room"))
                        }
                        CastScene.NEVER -> {
                            delay(600)
                            last?.end(Msg.NoticeCastNeverFetches("Living room"))
                        }
                        CastScene.NETWORK -> {
                            delay(600)
                            last?.end(Msg.NoticeCastNotSameNetwork("Living room"))
                        }
                        CastScene.CODEC -> {
                            delay(600)
                            last?.end(Msg.NoticeCastVideoCodec("Living room", "AV1"))
                        }
                        CastScene.DEEPLINK -> {
                            delay(600)
                            last?.end(Msg.NoticeCastDeepLink)
                        }
                        CastScene.AUDIO -> {
                            delay(1_000)
                            last?.command(FfiTvCommand.Track(FfiTvTrackKind.AUDIO, 2L))
                        }
                        CastScene.LOST -> {
                            delay(1_000)
                            last?.end(Msg.NoticeCastLost("Living room"))
                        }
                        CastScene.TAKEN -> {
                            delay(1_000)
                            last?.end(Msg.NoticeCastTakenOver("Living room"))
                        }
                        CastScene.BAR -> {
                            delay(1_000)
                            cast.closeCard()
                        }
                        else -> Unit
                    }
                }
            }
        } catch (e: IllegalStateException) {
            Log.w("anistream", "the cast scene did not set up: ${e.message}")
        }
    }

    /** The Chromecast row named [name], once the picker's browse has found it. */
    private suspend fun waitForRow(store: AppStore, name: String) = run {
        // The Cast browse rides on the picker's own, which starts first.
        store.tv.startBrowsing()
        store.cast.startBrowsing()
        var waited = 0
        var row = store.cast.rows.value.firstOrNull { it.name == name }
        while (row == null && waited < 100) {
            delay(100)
            waited++
            row = store.cast.rows.value.firstOrNull { it.name == name }
        }
        row
    }

    /** Whether [scene] is on screen, as a shot waits for it. */
    fun reached(store: AppStore, scene: CastScene): Boolean {
        val shown = store.cast.shown.value
        return when (scene) {
            CastScene.PICKER -> store.cast.rows.value.size == 2 && store.tv.rows.value.isNotEmpty()
            CastScene.SEARCHING, CastScene.NONE, CastScene.BLOCKED -> store.tv.sheet.value == TvRemoteModel.Sheet.Picker
            CastScene.STARTING, CastScene.PREPARING -> shown?.stage == CastStage.Starting && shown.cast.phaseLine != null
            CastScene.AUDIO -> shown?.stage == CastStage.Starting && shown.cast.card.now?.audio?.isNotEmpty() == true && shown.cast.phaseLine != null
            CastScene.FIXED, CastScene.LEVEL -> shown?.stage == CastStage.Playing
            CastScene.UPNEXT -> shown?.cast?.upNext != null
            CastScene.UNDO -> shown?.cast?.undo != null
            CastScene.FAILED, CastScene.NEVER, CastScene.NETWORK, CastScene.CODEC, CastScene.DEEPLINK -> shown?.stage == CastStage.Failed
            CastScene.LOST -> shown?.stage == CastStage.Lost
            CastScene.TAKEN -> shown == null
            CastScene.BAR -> shown?.stage == CastStage.Playing && !store.cast.open.value
        }
    }
}
