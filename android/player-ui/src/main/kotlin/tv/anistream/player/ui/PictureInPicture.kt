// Picture in Picture, as motion-android.md has it: the system's window and its motion, and ours
// only the parameters, set again whenever one of them changes, and the three actions.

package tv.anistream.player.ui

import tv.anistream.i18n.Strings
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Rect
import android.graphics.drawable.Icon
import android.util.Rational
import androidx.activity.compose.LocalActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import tv.anistream.player.PlayerModel
import tv.anistream.player.ui.R

/**
 * The window's parameters while the player is up: the video's aspect, its bounds in the window as
 * the source-rect hint, auto-enter only while playing, seamless resize, and back 10 s, play or
 * pause, and forward 10 s. Auto-enter goes off with the player.
 */
@Composable
internal fun PictureInPicture(model: PlayerModel, videoBounds: Rect?) {
    val activity = LocalActivity.current ?: return
    val context = LocalContext.current
    val playing = model.hasPicture && !model.paused
    DisposableEffect(context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                when (intent.getStringExtra(EXTRA)) {
                    BACK -> model.seekBy(-10.0)
                    PLAY_PAUSE -> model.togglePause()
                    FORWARD -> model.seekBy(10.0)
                }
            }
        }
        ContextCompat.registerReceiver(context, receiver, IntentFilter(ACTION), ContextCompat.RECEIVER_NOT_EXPORTED)
        onDispose {
            context.unregisterReceiver(receiver)
            activity.setPictureInPictureParams(PictureInPictureParams.Builder().setAutoEnterEnabled(false).build())
        }
    }
    LaunchedEffect(videoBounds, playing, model.aspect) {
        val aspect = (model.aspect ?: (16.0 / 9.0)).coerceIn(MIN_ASPECT, MAX_ASPECT)
        val params = PictureInPictureParams.Builder()
            .setAspectRatio(Rational((aspect * 1000).toInt(), 1000))
            .setAutoEnterEnabled(playing)
            .setSeamlessResizeEnabled(true)
            .setActions(
                listOf(
                    action(context, R.drawable.pip_back, Strings.playerBack10(context), BACK, 1),
                    if (playing) action(context, R.drawable.pip_pause, Strings.actionPause(context), PLAY_PAUSE, 2)
                    else action(context, R.drawable.pip_play, Strings.actionPlay(context), PLAY_PAUSE, 2),
                    action(context, R.drawable.pip_forward, Strings.playerForward10(context), FORWARD, 3),
                ),
            )
        videoBounds?.takeIf { !it.isEmpty }?.let(params::setSourceRectHint)
        activity.setPictureInPictureParams(params.build())
    }
}

private fun action(context: Context, icon: Int, title: String, what: String, code: Int): RemoteAction {
    val intent = Intent(ACTION).setPackage(context.packageName).putExtra(EXTRA, what)
    val pending = PendingIntent.getBroadcast(context, code, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    return RemoteAction(Icon.createWithResource(context, icon), title, title, pending)
}

private const val ACTION = "tv.anistream.player.PIP_ACTION"
private const val EXTRA = "action"
private const val BACK = "back"
private const val PLAY_PAUSE = "play-pause"
private const val FORWARD = "forward"

/** The aspect ratios the system's window takes, 1 : 2.39 to 2.39 : 1. */
private const val MIN_ASPECT = 1 / 2.39
private const val MAX_ASPECT = 2.39
