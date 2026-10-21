package tv.anistream.spike.player

import android.app.PendingIntent
import android.content.Intent
import android.util.Log
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * The media session around the spike's mpv, as the plan's `mediaPlayback` service: its
 * notification, lock-screen and PiP controls come from Media3. It holds no player of its own;
 * [SpikePlayer]'s mpv outlives both it and the activity.
 */
@UnstableApi
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        // After a crash the system may restart the service before any activity runs.
        SpikePlayer.start(this, "mediacodec,mediacodec-copy", "gpu-next", "android", emptyList())
        val open = PendingIntent.getActivity(
            this, 0,
            Intent(this, SpikeActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, SpikePlayer.player).setSessionActivity(open).build()
        Log.i(SpikePlayer.TAG, "SPIKE_SERVICE created")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        Log.i(SpikePlayer.TAG, "SPIKE_SERVICE destroyed")
        session?.release()
        session = null
        super.onDestroy()
    }
}
