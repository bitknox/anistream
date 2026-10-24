// The TV this phone drives, as Media3 sees it: the notification's title, play and pause and the
// 10 s steps, and the TV's volume on the phone's volume keys while the session is up.

package tv.anistream.app.tv

import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.DeviceInfo
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import tv.anistream.engine.ffi.FfiNowCard
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.store.TvRemoteModel

/**
 * The TV's card as a Media3 player, a remote one: it only mirrors the card, and each button sends
 * the TV a command. Its volume is the TV's own, so the volume keys reach the TV.
 */
@OptIn(UnstableApi::class)
class TvSessionPlayer(private val tv: TvRemoteModel) : SimpleBasePlayer(Looper.getMainLooper()) {
    /** The card changed: the session reads the state again. */
    fun changed() = invalidateState()

    override fun getState(): State {
        val commands = Player.Commands.Builder().addAll(
            Player.COMMAND_PLAY_PAUSE, Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_BACK, Player.COMMAND_SEEK_FORWARD, Player.COMMAND_STOP,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM, Player.COMMAND_GET_METADATA,
            Player.COMMAND_GET_TIMELINE, Player.COMMAND_RELEASE,
            Player.COMMAND_GET_DEVICE_VOLUME, Player.COMMAND_SET_DEVICE_VOLUME_WITH_FLAGS,
            Player.COMMAND_ADJUST_DEVICE_VOLUME_WITH_FLAGS,
        ).build()
        val card = tv.card.value?.card
        val now = card?.now
        val state = State.Builder()
            .setAvailableCommands(commands)
            .setDeviceInfo(DEVICE)
            .setDeviceVolume(now?.volume?.toInt()?.coerceIn(0, 100) ?: 50)
            .setSeekBackIncrementMs(STEP_MS)
            .setSeekForwardIncrementMs(STEP_MS)
            .setPlayWhenReady(now?.paused == false, Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE)
        if (card == null || now == null) return state.setPlaybackState(Player.STATE_IDLE).build()
        val metadata = MediaMetadata.Builder()
            .setTitle(now.title)
            .setArtist(words(now, card.tvName))
            .setDisplayTitle(now.title)
            .setSubtitle(words(now, card.tvName))
            .setMediaType(MediaMetadata.MEDIA_TYPE_TV_SHOW)
            .build()
        val id = "${now.id}-${now.episode}"
        val duration = now.duration
        val data = MediaItemData.Builder(id)
            .setMediaItem(MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata).build())
            .setMediaMetadata(metadata)
            .setDurationUs(if (duration != null && duration > 0) (duration * 1_000_000).toLong() else C.TIME_UNSET)
            .setIsSeekable(duration != null)
            .build()
        return state
            .setPlaylist(listOf(data))
            .setCurrentMediaItemIndex(0)
            .setPlaybackState(if (now.loading) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { ((tv.playhead() ?: now.position) * 1000).toLong() }
            .setPlaybackParameters(PlaybackParameters(now.speed.toFloat()))
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        val paused = tv.card.value?.card?.now?.paused
        if (paused != null && paused == playWhenReady) tv.send(FfiTvCommand.TogglePause)
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_BACK -> tv.send(FfiTvCommand.Seek(-STEP_MS / 1000.0))
            Player.COMMAND_SEEK_FORWARD -> tv.send(FfiTvCommand.Seek(STEP_MS / 1000.0))
            else -> tv.send(FfiTvCommand.SeekTo(positionMs / 1000.0))
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleIncreaseDeviceVolume(flags: Int): ListenableFuture<*> {
        tv.send(FfiTvCommand.Volume(TvRemoteModel.VOLUME_STEP))
        return Futures.immediateVoidFuture()
    }

    override fun handleDecreaseDeviceVolume(flags: Int): ListenableFuture<*> {
        tv.send(FfiTvCommand.Volume(-TvRemoteModel.VOLUME_STEP))
        return Futures.immediateVoidFuture()
    }

    override fun handleSetDeviceVolume(deviceVolume: Int, flags: Int): ListenableFuture<*> {
        val volume = tv.card.value?.card?.now?.volume
        if (volume != null) tv.send(FfiTvCommand.Volume(deviceVolume - volume))
        return Futures.immediateVoidFuture()
    }

    /** Disconnect, from the notification: the TV plays on. */
    override fun handleStop(): ListenableFuture<*> {
        tv.disconnect()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()

    companion object {
        /** Back and forward 10 s, as the card's own buttons. */
        const val STEP_MS = 10_000L

        private val DEVICE = DeviceInfo.Builder(DeviceInfo.PLAYBACK_TYPE_REMOTE).setMaxVolume(100).build()

        /** "Episode 3 · Low Tide · Living room", or "Starting on Living room". */
        fun words(now: FfiNowCard, tvName: String): String =
            if (now.loading) "Starting on $tvName" else "${now.episodeLine} · $tvName"
    }
}
