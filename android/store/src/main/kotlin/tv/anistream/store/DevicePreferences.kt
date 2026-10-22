// Settings this device keeps for itself rather than in the engine: the keys the app's screens store
// them under in its own preferences file, shared by the screens that set them and the parts that read
// them (DevicePreferences on Apple).

package tv.anistream.store

import android.content.Context
import android.content.SharedPreferences

object DevicePreferences {
    /** The app's own preferences file. */
    const val FILE = "anistream"

    /** Plain-text subtitles follow the system's caption style. On unless the viewer turns it off. */
    const val FOLLOW_SYSTEM_CAPTIONS = "followSystemCaptions"

    fun of(context: Context): SharedPreferences = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun followsSystemCaptions(context: Context): Boolean = of(context).getBoolean(FOLLOW_SYSTEM_CAPTIONS, true)
}
