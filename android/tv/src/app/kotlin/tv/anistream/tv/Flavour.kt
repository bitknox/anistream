// The app flavour's launch: the real engine on the app's own files.

package tv.anistream.tv

import android.content.Context
import android.content.Intent
import tv.anistream.store.EngineHome
import tv.anistream.store.EngineLaunch
import tv.anistream.store.enginePlatform

internal object Flavour {
    fun launch(context: Context, intent: Intent): EngineLaunch =
        EngineLaunch.app(EngineHome.sandbox(files = context.filesDir, cache = context.cacheDir))
            .copy(platform = enginePlatform(context))

    /** Off the main thread, before the engine starts: nothing for the app. */
    fun prepare(context: Context) = Unit

    fun start(intent: Intent): Start = Start()

    /** Nothing to set up for the app. */
    fun started(store: tv.anistream.store.AppStore, intent: Intent) = Unit
}
