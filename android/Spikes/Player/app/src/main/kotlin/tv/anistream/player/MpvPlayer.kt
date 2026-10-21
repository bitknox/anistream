package tv.anistream.player

import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import org.json.JSONObject

/**
 * mpv as a Media3 player, for the media session, its notification and PiP's controls. It only
 * mirrors mpv's state: play, pause and seek go to mpv, and mpv's properties come back through
 * [onMpvEvent]. Lives as long as the mpv it wraps, not the activity.
 */
@UnstableApi
class MpvPlayer(private val mpv: MpvClient, private val title: () -> String) :
    SimpleBasePlayer(Looper.getMainLooper()) {

    private var paused = true
    private var loaded = false
    private var buffering = false
    private var durationMs = 0L
    @Volatile private var positionMs = 0L

    fun onMpvEvent(event: MpvEvent) {
        when (event.id) {
            MpvNative.Event.FILE_LOADED -> { loaded = true; invalidateState() }
            MpvNative.Event.END_FILE -> {
                if (JSONObject(event.json ?: "{}").optString("reason") != "redirect") {
                    loaded = false
                    invalidateState()
                }
            }
            MpvNative.Event.PROPERTY_CHANGE -> when (event.name) {
                "pause" -> { paused = event.json == "true"; invalidateState() }
                "paused-for-cache" -> { buffering = event.json == "true"; invalidateState() }
                "duration" -> {
                    durationMs = ((event.json?.toDoubleOrNull() ?: 0.0) * 1000).toLong()
                    invalidateState()
                }
                // Read through the position supplier; no new state for every frame.
                "time-pos" -> positionMs = ((event.json?.toDoubleOrNull() ?: 0.0) * 1000).toLong()
                "seeking" -> invalidateState()
            }
        }
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
            .setPlayWhenReady(!paused, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setSeekBackIncrementMs(10_000)
            .setSeekForwardIncrementMs(10_000)
        if (!loaded) return state.setPlaybackState(Player.STATE_IDLE).build()
        val item = MediaItem.Builder()
            .setMediaId("spike")
            .setMediaMetadata(MediaMetadata.Builder().setTitle(title()).setArtist("Player spike").build())
            .build()
        return state
            .setPlaylist(
                listOf(
                    MediaItemData.Builder("spike").setMediaItem(item)
                        .setDurationUs(if (durationMs > 0) durationMs * 1000 else androidx.media3.common.C.TIME_UNSET)
                        .setIsSeekable(true).build(),
                ),
            )
            .setPlaybackState(if (buffering) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { positionMs }
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        mpv.set("pause", if (playWhenReady) "no" else "yes")
        paused = !playWhenReady
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_BACK -> mpv.command("seek", "-10", "relative")
            Player.COMMAND_SEEK_FORWARD -> mpv.command("seek", "10", "relative")
            else -> mpv.command("seek", (positionMs / 1000.0).toString(), "absolute")
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        mpv.command("stop")
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()
}
