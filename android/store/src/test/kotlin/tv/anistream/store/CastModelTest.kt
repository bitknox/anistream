package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiCastCard
import tv.anistream.engine.ffi.FfiCastRouteHint
import tv.anistream.engine.ffi.FfiCastRow
import tv.anistream.engine.ffi.FfiLinkPhase
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiRemoteCard
import tv.anistream.engine.ffi.FfiScreen
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.engine.ffi.FfiVolumeControl
import tv.anistream.engine.model.Msg

/** The cast card's stages, its endings and its volume, on a scripted TV with no engine. */
@OptIn(ExperimentalCoroutinesApi::class)
class CastModelTest {
    private var time = 0L

    /** A TV that sends what the test tells it to, and keeps what it was asked. */
    private class ScriptedLink(first: FfiCastCard) : CastLink {
        override val tvName = "Living room"
        var card = first
        val cards = Channel<FfiCastCard?>(Channel.UNLIMITED)
        val sent = mutableListOf<FfiTvCommand>()
        var stopped = false
        var playedHere = false
        val asked = mutableListOf<String>()
        var again = true

        override fun card() = card
        override suspend fun nextCard(): FfiCastCard? = cards.receive()?.also { card = it }
        override fun command(command: FfiTvCommand) {
            sent += command
        }

        override fun playHere(): Double? {
            playedHere = true
            return card.card.now?.position
        }

        override fun playNext() {
            asked += "play-next"
        }

        override fun cancelNext() {
            asked += "cancel-next"
        }

        override fun undoSkip() {
            asked += "undo-skip"
        }

        override fun tryAgain(): Boolean {
            asked += "try-again"
            return again
        }

        override fun stop() {
            stopped = true
        }
    }

    private val request = TvPlayRequest(id = 900001u, title = "The Lantern Road", episode = "7", from = null, fromPlayer = true)

    private fun card(
        loading: Boolean,
        volume: Double? = 40.0,
        over: Boolean = false,
        notice: Msg? = null,
        control: FfiVolumeControl = FfiVolumeControl.LEVEL,
    ) = FfiCastCard(
        card = FfiRemoteCard(
            tvName = "Living room",
            phase = FfiLinkPhase.CONNECTED,
            phaseLine = null,
            screen = FfiScreen.Player,
            now = FfiNowCard(
                id = 900001u, title = "The Lantern Road", episode = "7", episodeLine = "Episode 7", loading = loading,
                position = 120.0, duration = 1420.0, paused = false, speed = 1.0, volume = volume, audio = emptyList(),
                subtitles = emptyList(), subtitlesOff = false, skip = null,
            ),
            rev = 1u,
            over = over,
            reached = true,
        ),
        volumeControl = control,
        muted = false,
        notice = notice,
    )

    private fun TestScope.model(link: ScriptedLink): CastModel {
        val cast = CastModel(CoroutineScope(StandardTestDispatcher(testScheduler)), clock = { time })
        cast.starter = { _, _ -> link }
        return cast
    }

    private val row = FfiCastRow(
        key = "cast:fixture-living", name = "Living room", model = "Chromecast", address = "127.0.0.2",
        routeHint = FfiCastRouteHint.THROUGH_THIS_DEVICE,
    )

    @Test
    fun a_cast_starts_plays_and_closes_the_player_once() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        var handed = 0
        cast.handedOff = { handed++ }
        cast.cast(row, request)
        assertEquals(CastStage.Starting, cast.shown.value?.stage)
        assertTrue(cast.open.value)
        time = 5_000
        assertEquals(5L, cast.startingSeconds())
        link.cards.send(card(loading = false))
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        assertEquals(1, handed)
    }

    @Test
    fun taken_over_closes_the_card_with_a_notice_and_lost_keeps_it() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        val ended = backgroundScope.async { cast.ended.first() }
        runCurrent()
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastTakenOver("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertFalse(cast.open.value)
        assertEquals("7", ended.await().episode)

        val again = ScriptedLink(card(loading = false))
        val lost = model(again)
        lost.cast(row, request)
        again.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, lost.shown.value?.stage)
        lost.dismiss()
        assertNull(lost.shown.value)
    }

    @Test
    fun the_volume_keys_step_5_only_on_a_tv_that_takes_a_level() = runTest {
        val level = ScriptedLink(card(loading = false))
        val cast = model(level)
        cast.cast(row, request)
        cast.stepVolume(up = true)
        cast.setVolume(55.0)
        assertEquals(listOf(FfiTvCommand.Volume(5.0), FfiTvCommand.Volume(15.0)), level.sent)

        val fixed = ScriptedLink(card(loading = false, volume = null, control = FfiVolumeControl.MUTE_ONLY))
        val other = model(fixed)
        other.cast(row, request)
        other.stepVolume(up = true)
        other.toggleMute()
        assertEquals(listOf(FfiTvCommand.Mute(true)), fixed.sent)
    }

    @Test
    fun play_here_stops_the_tv_and_plays_on_here() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        var played: Pair<AnilistId, String>? = null
        cast.playHere = { id, episode -> played = id to episode }
        cast.cast(row, request)
        cast.playHere()
        assertTrue(link.playedHere)
        assertEquals(request.id to "7", played)
        assertNull(cast.shown.value)
    }

    @Test
    fun up_next_and_undo_go_to_the_engine() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        cast.playNext()
        cast.cancelNext()
        cast.undoSkip()
        assertEquals(listOf("play-next", "cancel-next", "undo-skip"), link.asked)
    }

    @Test
    fun try_again_starts_the_failed_cast_over_on_the_same_session() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastNeverFetches("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Failed, cast.shown.value?.stage)
        time = 9_000
        // The engine's card starts over.
        link.card = card(loading = true)
        cast.tryAgain()
        assertEquals(listOf("try-again"), link.asked)
        assertEquals(CastStage.Starting, cast.shown.value?.stage)
        assertEquals(9_000L, cast.shown.value?.startedAt)
        link.cards.send(card(loading = false, volume = 41.0))
        advanceUntilIdle()
        assertEquals(41.0, cast.shown.value?.cast?.card?.now?.volume)
    }
}
