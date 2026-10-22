// The app flavour's launch: the engine the app ships, in the sandbox, on the real AniList.

package tv.anistream.app

import android.content.Context
import android.content.Intent
import tv.anistream.store.AppStore
import tv.anistream.store.EngineHome
import tv.anistream.store.EngineLaunch

internal object Flavour {
    const val FIXTURES = false

    /** The app flavour updates itself, from the release the engine finds. */
    const val UPDATES = true

    fun launch(context: Context, intent: Intent): EngineLaunch =
        EngineLaunch.app(EngineHome.sandbox(files = context.filesDir, cache = context.cacheDir))

    /** The engine background work starts while the app is not running: the app's own files. */
    fun background(context: Context): EngineLaunch =
        EngineLaunch.app(EngineHome.sandbox(files = context.filesDir, cache = context.cacheDir))

    fun started(context: Context, store: AppStore, intent: Intent, notifier: EpisodeNotifier) {}
}
