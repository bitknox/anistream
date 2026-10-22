// The viewer's setting and the system's caption style, followed for the player: a change to either
// gives the mpv playing the new look.

package tv.anistream.player

import android.content.Context
import android.content.SharedPreferences
import android.view.accessibility.CaptioningManager
import tv.anistream.store.DevicePreferences

/**
 * Keeps [player] on the setting "Subtitles follow your system caption style" and on the style set in
 * Accessibility › Caption preferences, for as long as it is held. Both report on the main thread.
 */
class CaptionStyleWatch(context: Context, private val player: PlayerModel) {
    private val preferences = DevicePreferences.of(context)

    // Held here: SharedPreferences keeps its listeners only weakly.
    private val setting = SharedPreferences.OnSharedPreferenceChangeListener { preferences, key ->
        if (key == DevicePreferences.FOLLOW_SYSTEM_CAPTIONS) {
            player.followsSystemCaptions = preferences.getBoolean(key, true)
        }
    }

    private val system = object : CaptioningManager.CaptioningChangeListener() {
        override fun onEnabledChanged(enabled: Boolean) = player.captionStyleChanged()
        override fun onUserStyleChanged(userStyle: CaptioningManager.CaptionStyle) = player.captionStyleChanged()
        override fun onFontScaleChanged(fontScale: Float) = player.captionStyleChanged()
    }

    init {
        player.followsSystemCaptions = DevicePreferences.followsSystemCaptions(context)
        preferences.registerOnSharedPreferenceChangeListener(setting)
        context.getSystemService(CaptioningManager::class.java)?.addCaptioningChangeListener(system)
    }
}
