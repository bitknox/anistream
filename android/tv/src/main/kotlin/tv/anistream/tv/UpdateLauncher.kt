// What the TV's own update needs of the activity: Android's pages it opens, the "Install unknown
// apps" switch and the installer's confirmation, started once the app is on screen. The app
// flavour's updater gives one; the fixtures flavour installs nothing.

package tv.anistream.tv

import android.content.Intent
import kotlinx.coroutines.flow.StateFlow

interface UpdateLauncher {
    /** An activity to start from the app's own, once it is on screen. */
    val launch: StateFlow<Intent?>

    /** [intent] was started, or could not be. */
    fun launched(intent: Intent, started: Boolean)

    /** The app is on screen again, as after Android's page for the switch. */
    fun resumed()
}

/** The release build's package, the only one the TV's update installs over. */
const val TV_PACKAGE = "tv.anistream.tv"

/**
 * Whether there is room for an update of [bytes]: the APK in the app's files and the installer's
 * copy of it, with [margin] to spare for the system. [free] is what the app's files may still use.
 */
fun roomForUpdate(bytes: Long, free: Long, margin: Long = 64L * 1024 * 1024): Boolean =
    free >= bytes.coerceAtLeast(0) * 2 + margin
