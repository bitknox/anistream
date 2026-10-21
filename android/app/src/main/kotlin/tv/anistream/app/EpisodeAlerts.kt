// Whether new-episode notifications are on, and the schedule that follows it.

package tv.anistream.app

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The new-episode notifications switch, off until the person turns it on. Kept on the device
 * until this build's engine has `SettingKey::EpisodeNotifications`, which it then follows; the
 * periodic check runs while it is on.
 */
class EpisodeAlerts(private val context: Context, private val notifier: EpisodeNotifier, private val available: Boolean) {
    private val preferences get() = context.getSharedPreferences("anistream", Context.MODE_PRIVATE)

    private val _on = MutableStateFlow(available && preferences.getBoolean(KEY, false))
    val on: StateFlow<Boolean> = _on.asStateFlow()

    /** Turn them on or off, and the check with them. The permission is the activity's to ask first. */
    fun set(on: Boolean) {
        val wanted = on && available
        preferences.edit().putBoolean(KEY, wanted).apply()
        _on.value = wanted
        NewEpisodesSchedule.set(context, wanted)
    }

    /**
     * At launch: the schedule as the switch says, and the switch off where notifications can no
     * longer show, the permission taken back or the channel turned off.
     */
    fun reconcile() {
        set(_on.value && notifier.canPost)
    }

    /** What turning the switch takes. */
    enum class Turn {
        /** Off, with nothing asked. */
        Off,

        /** On: the notifications can show. */
        On,

        /** The permission first, asked now and only now; on once it is given. */
        Ask,

        /** Allowed, but turned off for the app or the channel: the system's settings turn it on. */
        SystemSettings;

        companion object {
            fun of(on: Boolean, canPost: Boolean, permitted: Boolean): Turn = when {
                !on -> Off
                canPost -> On
                permitted -> SystemSettings
                else -> Ask
            }
        }
    }

    private companion object {
        const val KEY = "episodeNotifications"
    }
}
