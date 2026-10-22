// Whether new-episode notifications are on, and the schedule that follows them and the widgets.

package tv.anistream.app

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch
import tv.anistream.engine.model.SettingKey
import tv.anistream.store.AppStore

/**
 * New-episode notifications on this device: the engine's `EpisodeNotifications` setting, off until
 * the person turns it on, and the shows switched on for themselves, which notify whatever it says.
 * The periodic check runs while either is on or a home-screen widget is placed, whose record it
 * keeps fresh, and is cancelled only when none of those holds; with only a widget, it notifies of
 * nothing. Where a notification no longer can show, the permission taken back or the channel
 * turned off, the setting goes off, and Settings says why.
 */
class EpisodeAlerts(private val context: Context, private val store: AppStore, private val notifier: EpisodeNotifier) {
    private val _refused = MutableStateFlow(false)

    /** Whether it went off, or did not go on, because Android does not let anistream notify. */
    val refused: StateFlow<Boolean> = _refused.asStateFlow()

    /** Whether a widget is on a home screen, as last looked; `null` before the first look. */
    private val widgets = MutableStateFlow<Boolean?>(null)

    /**
     * Turn them on, as the activity does once they can show, or off: off keeps the schedule for
     * the shows switched on for themselves and the widgets placed, while there are any.
     */
    fun set(on: Boolean) {
        if (on) _refused.value = false
        store.setSwitch(KEY, on)
        val switchedOn = store.settings.view.value?.notifySwitchedOn ?: 0u
        NewEpisodesSchedule.set(context, Wanted.of(on, switchedOn, widgets.value == true).checks)
    }

    /** They could not go on, or stay on: the permission declined, or notifications blocked. */
    fun refuse() {
        set(false)
        _refused.value = true
    }

    /**
     * Whether any widget is [placed], as the app looked at its start and as a widget is added or
     * the last of one removed. A widget placed keeps the schedule whatever the settings say; with
     * none, the settings decide, once the engine has said them.
     */
    fun widgetsPlaced(placed: Boolean) {
        widgets.value = placed
        val view = store.settings.view.value
        when {
            placed -> NewEpisodesSchedule.set(context, true)
            view != null -> NewEpisodesSchedule.set(context, Wanted.of(store.settings.isOn(KEY, view), view.notifySwitchedOn, false).checks)
        }
    }

    /**
     * For the life of the process: the schedule as the setting, the shows' own switches and the
     * widgets say, asked again at each launch so it keeps its place and takes this version's
     * constraints, and the setting off where notifications can no longer show.
     */
    fun follow(scope: CoroutineScope) {
        scope.launch {
            store.settings.view.filterNotNull()
                .combine(widgets) { view, placed -> Wanted.of(store.settings.isOn(KEY, view), view.notifySwitchedOn, placed == true) }
                .distinctUntilChanged()
                .collect { wanted ->
                    if (wanted.setting && !notifier.canPost) refuse() else NewEpisodesSchedule.set(context, wanted.checks)
                }
        }
    }

    /**
     * What keeps the periodic check: the switch, the shows switched on for themselves, and the
     * widgets placed.
     */
    data class Wanted(val setting: Boolean, val switchedOn: Boolean, val widgets: Boolean = false) {
        /** Whether anything notifies: the setting, or a show of its own. */
        val notifies: Boolean get() = setting || switchedOn

        /** Whether the periodic check runs: anything notifies, or a widget's record wants keeping. */
        val checks: Boolean get() = notifies || widgets

        companion object {
            fun of(setting: Boolean, switchedOn: UInt, widgets: Boolean = false): Wanted = Wanted(setting, switchedOn > 0u, widgets)
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
