// The player as Media3 sees it, for the media session: its notification, the lock screen and
// Bluetooth controls.

package tv.anistream.player

import android.graphics.Bitmap
import android.os.Looper
import java.io.ByteArrayOutputStream
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

/**
 * The [PlayerModel] as a Media3 player. It only mirrors the model: play, pause and seek go to the
 * engine as the player's own buttons send them, and what mpv says comes back through the model.
 * Lives with the media session, which lives with [PlayerService], not with the activity.
 */
@OptIn(UnstableApi::class)
class SessionPlayer(
    private val model: PlayerModel,
    private val artwork: (String) -> Bitmap? = { null },
) : SimpleBasePlayer(Looper.getMainLooper()) {
    /** The art last encoded for the session, by its URL. */
    private var art: Pair<String, ByteArray>? = null

    init {
        model.onSessionChange = { invalidateState() }
    }

    override fun getState(): State {
        val commands = Player.Commands.Builder().addAll(
            Player.COMMAND_PLAY_PAUSE, Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_BACK, Player.COMMAND_SEEK_FORWARD, Player.COMMAND_STOP,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM, Player.COMMAND_GET_METADATA,
            Player.COMMAND_GET_TIMELINE, Player.COMMAND_RELEASE,
        ).build()
        val state = State.Builder()
            .setAvailableCommands(commands)
            .setPlayWhenReady(!model.paused, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setSeekBackIncrementMs(SEEK_STEP_MS)
            .setSeekForwardIncrementMs(SEEK_STEP_MS)
        if (!model.isPresented) return state.setPlaybackState(Player.STATE_IDLE).build()
        val metadata = MediaMetadata.Builder()
            .setTitle(model.episodeLine)
            .setArtist(model.showTitle)
            .setDisplayTitle(model.episodeLine)
            .setSubtitle(model.showTitle)
            .setMediaType(MediaMetadata.MEDIA_TYPE_TV_SHOW)
            .apply { artworkBytes()?.let { setArtworkData(it, MediaMetadata.PICTURE_TYPE_FRONT_COVER) } }
            .build()
        val id = "${model.titleId ?: 0u}-${model.startingEpisode ?: model.episode?.number ?: ""}"
        val item = MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata).build()
        val duration = model.duration
        val data = MediaItemData.Builder(id)
            .setMediaItem(item)
            .setMediaMetadata(metadata)
            .setDurationUs(if (duration != null && duration > 0) (duration * 1_000_000).toLong() else C.TIME_UNSET)
            .setIsSeekable(duration != null)
            .build()
        val waiting = !model.hasPicture || model.bufferingSince != null
        return state
            .setPlaylist(listOf(data))
            .setCurrentMediaItemIndex(0)
            .setPlaybackState(if (waiting) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { (model.playhead * 1000).toLong() }
            .setPlaybackParameters(androidx.media3.common.PlaybackParameters(model.speed.toFloat()))
            .build()
    }

    /** The title's cover as the notification shows it, else its banner, encoded once. */
    private fun artworkBytes(): ByteArray? {
        val url = model.coverUrl ?: model.bannerUrl ?: return null
        art?.let { (known, bytes) -> if (known == url) return bytes }
        val bitmap = artwork(url) ?: return null
        val bytes = ByteArrayOutputStream().use { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }
        art = url to bytes
        return bytes
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        model.setPlaying(playWhenReady)
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_BACK -> model.seekBy(-SEEK_STEP_MS / 1000.0)
            Player.COMMAND_SEEK_FORWARD -> model.seekBy(SEEK_STEP_MS / 1000.0)
            else -> model.seekTo(positionMs / 1000.0)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        model.viewerClosed()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> {
        model.onSessionChange = null
        return Futures.immediateVoidFuture()
    }

    companion object {
        /** Back and forward 10 s, as the player's own buttons. */
        const val SEEK_STEP_MS = 10_000L
    }
}
