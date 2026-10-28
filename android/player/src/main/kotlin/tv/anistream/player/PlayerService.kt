// The media session around the player, as a mediaPlayback foreground service: the notification,
// the lock screen and Bluetooth controls, and the process kept while it plays in the background.

package tv.anistream.player

import tv.anistream.i18n.Strings
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.util.Log
import androidx.compose.runtime.snapshotFlow
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.Player
import androidx.media3.session.CommandButton
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaController
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

/** What the process holds the player in: the application, for as long as the process. */
interface PlayerHost {
    val player: PlayerModel

    /** The art at [url], decoded, when the app has it: for the notification and the lock screen. */
    fun artwork(url: String): Bitmap? = null

    /** The notification's small icon, the app's monochrome mark, or 0 for Media3's own. */
    val notificationIcon: Int get() = 0
}

/**
 * The media session, served while the player is up. It holds no mpv of its own: mpv lives with
 * the process's [PlayerModel], so the activity coming and going changes nothing, and Media3 keeps
 * this service in the foreground, with its notification, while it plays.
 */
@OptIn(UnstableApi::class)
class PlayerService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val model = (application as PlayerHost).player
        val open = packageManager.getLaunchIntentForPackage(packageName)
            ?.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            ?.let { PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT) }
        val host = application as PlayerHost
        // Back and forward 10 s either side of play and pause, on the notification and the lock screen.
        val steps = listOf(
            CommandButton.Builder(CommandButton.ICON_SKIP_BACK_10)
                .setDisplayName(Strings.playerBack10(this))
                .setPlayerCommand(Player.COMMAND_SEEK_BACK)
                .setSlots(CommandButton.SLOT_BACK)
                .build(),
            CommandButton.Builder(CommandButton.ICON_SKIP_FORWARD_10)
                .setDisplayName(Strings.playerForward10(this))
                .setPlayerCommand(Player.COMMAND_SEEK_FORWARD)
                .setSlots(CommandButton.SLOT_FORWARD)
                .build(),
        )
        session = MediaSession.Builder(this, SessionPlayer(this, model, host::artwork))
            .apply { open?.let(::setSessionActivity) }
            .setMediaButtonPreferences(steps)
            .build()
        if (host.notificationIcon != 0) {
            setMediaNotificationProvider(DefaultMediaNotificationProvider.Builder(this).build().apply { setSmallIcon(host.notificationIcon) })
        }
        Log.i(PlayerModel.TAG, "the media session is up")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** Swiped away from the recent apps: the player closes, as the viewer closing it would. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        (application as PlayerHost).player.viewerClosed()
        stopSelf()
    }

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        Log.i(PlayerModel.TAG, "the media session is down")
        super.onDestroy()
    }
}

/**
 * Keeps [PlayerService] bound while the player is up, so the media session is there whenever a
 * playback is, and lets it go when the player comes down.
 */
@OptIn(UnstableApi::class)
class PlayerConnection(private val context: Context, private val model: PlayerModel, scope: CoroutineScope) {
    private var controller: ListenableFuture<MediaController>? = null

    init {
        scope.launch {
            snapshotFlow { model.isPresented }.distinctUntilChanged().collect { up -> if (up) bind() else unbind() }
        }
    }

    private fun bind() {
        if (controller != null) return
        val token = SessionToken(context, ComponentName(context, PlayerService::class.java))
        controller = MediaController.Builder(context, token).buildAsync()
    }

    private fun unbind() {
        controller?.let(MediaController::releaseFuture)
        controller = null
        context.stopService(Intent(context, PlayerService::class.java))
    }
}
