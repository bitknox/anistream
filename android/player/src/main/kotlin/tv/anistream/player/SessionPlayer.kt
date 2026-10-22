// The player as Media3 sees it, for the media session: its notification, the lock screen and
// Bluetooth controls.

package tv.anistream.player

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import java.io.ByteArrayOutputStream
import java.util.concurrent.Executor
import java.util.concurrent.Executors
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

    /** The art being encoded off the main thread, by its URL, so it is asked for once. */
    private var encoding: String? = null

    /** Released: an encoding that lands after it changes nothing. */
    private var released = false

    private val main = Handler(Looper.getMainLooper())

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

    /**
     * The title's cover as the notification shows it, else its banner, encoded once. The encoding
     * runs off the main thread, since the state is asked for on it as the player opens (T2): the
     * metadata goes without art until it is ready, then the state is asked for again.
     */
    private fun artworkBytes(): ByteArray? {
        val url = model.coverUrl ?: model.bannerUrl ?: return null
        art?.let { (known, bytes) -> if (known == url) return bytes }
        if (encoding == url) return null
        val bitmap = artwork(url) ?: return null
        encoding = url
        ENCODER.execute {
            val bytes = runCatching { encode(bitmap) }.getOrNull()
            main.post {
                if (encoding == url) encoding = null
                if (bytes != null && !released) {
                    art = url to bytes
                    invalidateState()
                }
            }
        }
        return null
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
        released = true
        model.onSessionChange = null
        return Futures.immediateVoidFuture()
    }

    companion object {
        /** Back and forward 10 s, as the player's own buttons. */
        const val SEEK_STEP_MS = 10_000L

        /** The longest side the session's art is sent at: more than the lock screen draws. */
        private const val ART_SIDE = 1024

        /** Where the art is encoded: one thread, at low priority, off the frames' way. */
        private val ENCODER: Executor = Executors.newSingleThreadExecutor { work ->
            Thread(work, "session-art").apply {
                isDaemon = true
                priority = Thread.MIN_PRIORITY
            }
        }

        /** [bitmap] as a JPEG at most [ART_SIDE] on its longer side. */
        private fun encode(bitmap: Bitmap): ByteArray {
            val side = maxOf(bitmap.width, bitmap.height)
            val scaled = if (side <= ART_SIDE) {
                bitmap
            } else {
                val scale = ART_SIDE.toFloat() / side
                Bitmap.createScaledBitmap(bitmap, (bitmap.width * scale).toInt().coerceAtLeast(1), (bitmap.height * scale).toInt().coerceAtLeast(1), true)
            }
            return ByteArrayOutputStream().use { out ->
                scaled.compress(Bitmap.CompressFormat.JPEG, 90, out)
                out.toByteArray()
            }
        }
    }
}
