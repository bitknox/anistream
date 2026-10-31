// The cast running now, kept through the background: a media-style notification with the title,
// pause, Next and Stop, the TV's volume on the volume keys when it takes a level, and the Wi-Fi
// and the processor kept awake while the stream goes through this device.

package tv.anistream.app.tv

import tv.anistream.i18n.Strings
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Looper
import android.os.PowerManager
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.DeviceInfo
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.CommandButton
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaController
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import tv.anistream.app.AnistreamApp
import tv.anistream.app.R
import tv.anistream.engine.ffi.FfiTvCommand
import tv.anistream.store.CastModel
import tv.anistream.store.CastStage

/**
 * The media session around the cast, in the foreground while it starts or plays. The link lives
 * in the process's [CastModel]; this keeps the process, the Wi-Fi and the processor up for it.
 */
@OptIn(UnstableApi::class)
class CastService : MediaSessionService() {
    private var session: MediaSession? = null
    private val scope = MainScope()
    private var wifi: WifiManager.WifiLock? = null
    private var awake: PowerManager.WakeLock? = null

    override fun onCreate() {
        super.onCreate()
        val cast = (application as AnistreamApp).store.cast
        val player = CastSessionPlayer(this, cast)
        val open = packageManager.getLaunchIntentForPackage(packageName)
            ?.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            ?.putExtra(EXTRA_CAST, true)
            ?.let { PendingIntent.getActivity(this, CAST_REQUEST, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT) }
        val buttons = listOf(
            CommandButton.Builder(CommandButton.ICON_NEXT)
                .setDisplayName(Strings.playerNextEpisode(this))
                .setPlayerCommand(Player.COMMAND_SEEK_TO_NEXT)
                .setSlots(CommandButton.SLOT_FORWARD)
                .build(),
            CommandButton.Builder(CommandButton.ICON_STOP)
                .setDisplayName(Strings.castCardStop(this))
                .setPlayerCommand(Player.COMMAND_STOP)
                .setSlots(CommandButton.SLOT_FORWARD_SECONDARY, CommandButton.SLOT_OVERFLOW)
                .build(),
        )
        session = MediaSession.Builder(this, player)
            .setId(SESSION_ID)
            .apply { open?.let(::setSessionActivity) }
            .setMediaButtonPreferences(buttons)
            .build()
        setMediaNotificationProvider(
            DefaultMediaNotificationProvider.Builder(this).build().apply { setSmallIcon(R.drawable.ic_launcher_monochrome) },
        )
        holdAwake()
        scope.launch { cast.shown.collect { player.changed() } }
        Log.i(AnistreamApp.TAG, "the cast's session is up")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** In the foreground paused too: the stream to the TV goes through this device. */
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        super.onUpdateNotification(session, true)
    }

    /** Swiped away from the recent apps: the TV still plays, driven from the notification. */
    override fun onTaskRemoved(rootIntent: Intent?) {}

    override fun onDestroy() {
        scope.cancel()
        session?.run {
            player.release()
            release()
        }
        session = null
        letSleep()
        Log.i(AnistreamApp.TAG, "the cast's session is down")
        super.onDestroy()
    }

    /** The stream goes out from here: the Wi-Fi stays quick and the processor on while it does. */
    private fun holdAwake() {
        wifi = getSystemService(WifiManager::class.java)
            ?.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, LOCK_TAG)
            ?.apply { setReferenceCounted(false); acquire() }
        awake = getSystemService(PowerManager::class.java)
            ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, LOCK_TAG)
            ?.apply { setReferenceCounted(false); acquire() }
    }

    private fun letSleep() {
        wifi?.takeIf { it.isHeld }?.release()
        awake?.takeIf { it.isHeld }?.release()
        wifi = null
        awake = null
    }

    companion object {
        /** The notification's tap: the app, on the cast's card. */
        const val EXTRA_CAST = "tv.anistream.extra.CAST"
        private const val CAST_REQUEST = 8
        private const val SESSION_ID = "cast"
        private const val LOCK_TAG = "anistream:cast"
    }
}

/**
 * The cast's card as a Media3 player, a remote one: it mirrors the card, and each button sends the
 * TV a command. A TV that takes a level has it on the volume keys, 5 a press; one whose volume is
 * fixed leaves the keys to this device.
 */
@OptIn(UnstableApi::class)
class CastSessionPlayer(private val context: Context, private val cast: CastModel) : SimpleBasePlayer(Looper.getMainLooper()) {
    /** The card changed: the session reads the state again. */
    fun changed() = invalidateState()

    override fun getState(): State {
        val shown = cast.shown.value
        val level = shown?.level == true
        val commands = Player.Commands.Builder().addAll(
            Player.COMMAND_PLAY_PAUSE, Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_STOP,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM, Player.COMMAND_GET_METADATA,
            Player.COMMAND_GET_TIMELINE, Player.COMMAND_RELEASE,
        ).apply {
            if (level) {
                addAll(
                    Player.COMMAND_GET_DEVICE_VOLUME, Player.COMMAND_SET_DEVICE_VOLUME_WITH_FLAGS,
                    Player.COMMAND_ADJUST_DEVICE_VOLUME_WITH_FLAGS,
                )
            }
        }.build()
        val now = shown?.let(CastModel::now)
        val state = State.Builder()
            .setAvailableCommands(commands)
            .setDeviceInfo(if (level) LEVEL else FIXED)
            .setDeviceVolume(now?.volume?.toInt()?.coerceIn(0, 100) ?: 0)
            .setPlayWhenReady(now?.paused == false, Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE)
        if (shown == null || now == null) return state.setPlaybackState(Player.STATE_IDLE).build()
        val starting = shown.stage == CastStage.Starting
        val line = if (starting || now.episodeLine.isEmpty()) {
            Strings.tvSessionStarting(context, shown.tvName)
        } else {
            Strings.tvSessionLine(context, now.episodeLine, shown.tvName)
        }
        val metadata = MediaMetadata.Builder()
            .setTitle(now.title)
            .setArtist(line)
            .setDisplayTitle(now.title)
            .setSubtitle(line)
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
            .setPlaybackState(if (starting) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { ((cast.playhead() ?: now.position) * 1000).toLong() }
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        val paused = cast.shown.value?.cast?.card?.now?.paused
        if (paused != null && paused == playWhenReady) cast.send(FfiTvCommand.TogglePause)
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> cast.send(FfiTvCommand.Next)
            else -> cast.send(FfiTvCommand.SeekTo(positionMs / 1000.0))
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleIncreaseDeviceVolume(flags: Int): ListenableFuture<*> {
        cast.stepVolume(up = true)
        return Futures.immediateVoidFuture()
    }

    override fun handleDecreaseDeviceVolume(flags: Int): ListenableFuture<*> {
        cast.stepVolume(up = false)
        return Futures.immediateVoidFuture()
    }

    override fun handleSetDeviceVolume(deviceVolume: Int, flags: Int): ListenableFuture<*> {
        cast.setVolume(deviceVolume.toDouble())
        return Futures.immediateVoidFuture()
    }

    /** Stop casting, from the notification: the receiver quits on the TV. */
    override fun handleStop(): ListenableFuture<*> {
        cast.stop()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()

    private companion object {
        val LEVEL = DeviceInfo.Builder(DeviceInfo.PLAYBACK_TYPE_REMOTE).setMaxVolume(100).build()

        /** The TV's own volume is in charge: nothing for the volume keys to move. */
        val FIXED = DeviceInfo.Builder(DeviceInfo.PLAYBACK_TYPE_REMOTE).setMaxVolume(0).build()
    }
}

/** Keeps [CastService] up while a cast starts or plays, and lets it go once the cast is over. */
@OptIn(UnstableApi::class)
class CastKeeper(private val context: Context, cast: CastModel, scope: CoroutineScope) {
    private var controller: ListenableFuture<MediaController>? = null

    init {
        scope.launch {
            cast.shown
                .map { it?.stage == CastStage.Starting || it?.stage == CastStage.Playing }
                .distinctUntilChanged()
                .collect { held -> if (held) bind() else unbind() }
        }
    }

    private fun bind() {
        if (controller != null) return
        val token = SessionToken(context, ComponentName(context, CastService::class.java))
        controller = MediaController.Builder(context, token).buildAsync()
    }

    private fun unbind() {
        val bound = controller ?: return
        MediaController.releaseFuture(bound)
        controller = null
        context.stopService(Intent(context, CastService::class.java))
    }
}
