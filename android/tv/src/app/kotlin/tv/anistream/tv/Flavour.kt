// The app flavour's launch: the real engine on the app's own files.

package tv.anistream.tv

import android.content.Context
import android.content.Intent
import kotlinx.coroutines.CoroutineScope
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

    /**
     * The TV updates itself from the release's TV APK: the updater, attached to the store's
     * update, with what a stopped process left cleared and a press kept across Android's page.
     */
    fun updater(app: TvApp, scope: CoroutineScope, intent: Intent): UpdateLauncher? {
        val updater = TvUpdates(app, app.store.updates, scope)
        app.store.updates.attach(updater, updater.files)
        updater.clearLeftovers()
        updater.kept()?.let(app.store.updates::returning)
        return updater
    }

    /** The release's TV APK is what this flavour updates from. */
    const val UPDATES = true

    /** Nothing to set up for the app. */
    fun started(store: tv.anistream.store.AppStore, intent: Intent) = Unit
}
