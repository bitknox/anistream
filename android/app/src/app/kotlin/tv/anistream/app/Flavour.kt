// The app flavour's launch: the engine the app ships, in the sandbox, on the real AniList.

package tv.anistream.app

import android.content.Context
import android.content.Intent
import tv.anistream.app.tv.NsdTvBrowser
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.store.AppStore
import tv.anistream.store.EngineHome
import tv.anistream.store.EngineLaunch
import tv.anistream.store.enginePlatform

internal object Flavour {
    const val FIXTURES = false

    /** The app flavour updates itself, from the release the engine finds. */
    const val UPDATES = true

    fun launch(context: Context, intent: Intent): EngineLaunch =
        EngineLaunch.app(EngineHome.sandbox(files = context.filesDir, cache = context.cacheDir))
            .copy(platform = enginePlatform(context))

    /** The engine background work starts while the app is not running: the app's own files. */
    fun background(context: Context): EngineLaunch =
        EngineLaunch.app(EngineHome.sandbox(files = context.filesDir, cache = context.cacheDir))

    fun started(context: Context, store: AppStore, intent: Intent, notifier: EpisodeNotifier) {}

    /** The TVs Android's DNS-SD finds on the network. */
    fun tvBrowser(context: Context, store: AppStore): FfiTvBrowser = NsdTvBrowser(context)

    /** The app flavour stages no TV scene. */
    suspend fun reachedTv(intent: Intent, store: AppStore): Boolean = true
}
