// Whether new-episode notifications are on, and the schedule that follows it.

package tv.anistream.app

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.AppStore

/**
 * New-episode notifications on this device: the engine's `EpisodeNotifications` setting, off until
 * the person turns it on. The periodic check runs while it is on and a notification can show; where
 * one no longer can, the permission taken back or the channel turned off, it goes off, and Settings
 * says why.
 */
class EpisodeAlerts(private val context: Context, private val store: AppStore, private val notifier: EpisodeNotifier) {
    private val _refused = MutableStateFlow(false)

    /** Whether it went off, or did not go on, because Android does not let anistream notify. */
    val refused: StateFlow<Boolean> = _refused.asStateFlow()

    /** Turn them on, as the activity does once they can show, or off. */
    fun set(on: Boolean) {
        if (on) _refused.value = false
        store.setSwitch(KEY, on)
        NewEpisodesSchedule.set(context, on)
    }

    /** They could not go on, or stay on: the permission declined, or notifications blocked. */
    fun refuse() {
        set(false)
        _refused.value = true
    }

    /**
     * For the life of the process: the schedule as the setting says, asked again at each launch so
     * it keeps its place and takes this version's constraints, and the setting off where
     * notifications can no longer show.
     */
    fun follow(scope: CoroutineScope) {
        scope.launch {
            store.settings.view.filterNotNull().map { store.settings.isOn(KEY, it) }.distinctUntilChanged().collect { on ->
                if (on && !notifier.canPost) refuse() else NewEpisodesSchedule.set(context, on)
            }
        }
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

    companion object {
        val KEY = SettingKey.EPISODE_NOTIFICATIONS
    }
}
