// Notifications: a copy of a notice for a user who has switched away. Best effort, since the
// notice in the app is the real channel.

package tv.anistream.app

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class Notifier(private val context: Context) {
    private val manager = context.getSystemService(NotificationManager::class.java)
    private val ids = AtomicInteger(1)

    private val _wantsPermission = MutableStateFlow(false)

    /**
     * Whether a notification waited on the permission: the activity asks for it then, the first
     * time one is needed, and never at launch.
     */
    val wantsPermission: StateFlow<Boolean> = _wantsPermission.asStateFlow()

    init {
        val channel = NotificationChannel(CHANNEL, context.getString(R.string.notices_channel), NotificationManager.IMPORTANCE_DEFAULT)
        channel.description = context.getString(R.string.notices_channel_description)
        manager.createNotificationChannel(channel)
    }

    private val allowed: Boolean
        get() = ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED

    /** Post [title] and [body]; called on the engine's threads. */
    fun post(title: String, body: String) {
        if (!allowed) {
            _wantsPermission.value = true
            return
        }
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(title)
            .setContentText(body)
            .setAutoCancel(true)
            .build()
        manager.notify(ids.getAndIncrement(), notification)
    }

    /**
     * A notification is about to be needed, the download job's, which the system posts itself:
     * ask for the permission now, as [post] would, when it is not there.
     */
    fun wantPermission() {
        if (!allowed) _wantsPermission.value = true
    }

    /** The permission was asked for, whatever the answer. */
    fun asked() {
        _wantsPermission.value = false
    }

    private companion object {
        const val CHANNEL = "notices"
    }
}
