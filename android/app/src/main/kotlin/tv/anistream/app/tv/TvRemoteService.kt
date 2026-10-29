// The TV this phone drives, kept through the background: a media-style notification with the
// title, play and pause and the 10 s steps, the TV's volume on the volume keys, and the process
// kept, with the engine's link to the TV in it, until the session is over or the viewer
// disconnects.

package tv.anistream.app.tv

import tv.anistream.i18n.Strings
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Network
import android.util.Log
import androidx.annotation.OptIn
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.CommandButton
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaController
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import tv.anistream.app.AnistreamApp
import tv.anistream.app.R
import tv.anistream.store.TvRemoteModel

/**
 * The media session around the TV's card, in the foreground for as long as it is up, paused or
 * not: a TV paused for a while is still being driven. It holds no connection of its own; the
 * link lives in the process's [TvRemoteModel], which this keeps.
 */
@OptIn(UnstableApi::class)
class TvRemoteService : MediaSessionService() {
    private var session: MediaSession? = null
    private val scope = MainScope()

    override fun onCreate() {
        super.onCreate()
        val tv = (application as AnistreamApp).store.tv
        val player = TvSessionPlayer(this, tv)
        val open = packageManager.getLaunchIntentForPackage(packageName)
            ?.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            ?.putExtra(EXTRA_REMOTE, true)
            ?.let { PendingIntent.getActivity(this, REMOTE_REQUEST, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT) }
        val buttons = listOf(
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
            CommandButton.Builder(CommandButton.ICON_STOP)
                .setDisplayName(Strings.actionDisconnect(this))
                .setPlayerCommand(Player.COMMAND_STOP)
                .setSlots(CommandButton.SLOT_OVERFLOW)
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
        scope.launch { tv.card.collect { player.changed() } }
        Log.i(AnistreamApp.TAG, "the TV's session is up")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** In the foreground paused too: the link to the TV stays while it is driven. */
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        super.onUpdateNotification(session, true)
    }

    /** Swiped away from the recent apps: the TV is still driven, from the notification. */
    override fun onTaskRemoved(rootIntent: Intent?) {}

    override fun onDestroy() {
        scope.cancel()
        session?.run {
            player.release()
            release()
        }
        session = null
        Log.i(AnistreamApp.TAG, "the TV's session is down")
        super.onDestroy()
    }

    companion object {
        /** The notification's tap: the app, on the TV's card. */
        const val EXTRA_REMOTE = "tv.anistream.extra.TV_REMOTE"
        private const val REMOTE_REQUEST = 7
        private const val SESSION_ID = "tv"
    }
}

/**
 * Keeps [TvRemoteService] up while a TV is driven and plays something, and the link to it
 * coming back: a try at once when a network comes up and when the app comes to the front, no
 * polling between. A session the engine calls over, as the TV stops playing or stays out of
 * reach, ends here, unless the card is open in front of the viewer.
 */
@OptIn(UnstableApi::class)
class TvConnection(private val context: Context, private val tv: TvRemoteModel, scope: CoroutineScope) {
    private var controller: ListenableFuture<MediaController>? = null
    private var network: ConnectivityManager.NetworkCallback? = null

    init {
        scope.launch {
            combine(tv.connected, tv.card, tv.sheet) { connected, card, sheet ->
                val over = card?.card?.over == true
                Triple(connected, connected != null && card?.card?.now != null && !over, over && sheet != TvRemoteModel.Sheet.Remote)
            }.distinctUntilChanged().collect { (connected, held, ended) ->
                if (ended) {
                    tv.disconnect()
                    return@collect
                }
                if (held) bind() else unbind()
                if (connected != null) watchNetwork() else unwatchNetwork()
            }
        }
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) = tv.retryNow()
        })
    }

    private fun bind() {
        if (controller != null) return
        val token = SessionToken(context, ComponentName(context, TvRemoteService::class.java))
        controller = MediaController.Builder(context, token).buildAsync()
    }

    private fun unbind() {
        val bound = controller ?: return
        MediaController.releaseFuture(bound)
        controller = null
        context.stopService(Intent(context, TvRemoteService::class.java))
    }

    /** A try at the TV each time a network comes up, while one is driven. */
    private fun watchNetwork() {
        if (network != null) return
        val connectivity = context.getSystemService(ConnectivityManager::class.java) ?: return
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                context.mainExecutor.execute { tv.retryNow() }
            }
        }
        network = callback
        connectivity.registerDefaultNetworkCallback(callback)
    }

    private fun unwatchNetwork() {
        val callback = network ?: return
        network = null
        runCatching { context.getSystemService(ConnectivityManager::class.java)?.unregisterNetworkCallback(callback) }
    }
}
