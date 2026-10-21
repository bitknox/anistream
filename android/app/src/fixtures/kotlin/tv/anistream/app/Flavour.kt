// The fixtures flavour's launch: the testing engine on a scene, each launch in a directory of its
// own, steered by intent extras under the names of the Apple app's -Anistream* arguments:
//
//   scene     watching (the default), fresh or playing
//   section   the destination to open on: continue, calendar, seasonal, library or search
//   watched   episodes to mark watched once the engine is up: 900006:8, or 900001:1,2,3, with more
//             titles joined by / (900006:1,2/900002:1), oldest first
//   title     a title page to open, by id
//   sheet     a sheet to open: torrent, join, kitsu, update or show (from A2)
//   player    a player state to shoot (from A3)

package tv.anistream.app

import android.content.Context
import android.content.Intent
import android.util.Log
import tv.anistream.store.AppStore
import tv.anistream.store.EngineLaunch
import tv.anistream.store.FixtureScene
import tv.anistream.store.fixtures

internal object Flavour {
    const val FIXTURES = true

    private var marked = false

    fun launch(context: Context, intent: Intent): EngineLaunch {
        val named = intent.getStringExtra("scene")
        val scene = named?.let(FixtureScene::named) ?: FixtureScene.WATCHING
        if (named != null && FixtureScene.named(named) == null) Log.w(AnistreamApp.TAG, "no scene named $named")
        return EngineLaunch.fixtures(scene, context.cacheDir)
    }

    /** What the extras ask of the engine once it is up, once per process. */
    fun started(store: AppStore, intent: Intent) {
        if (marked) return
        marked = true
        intent.getStringExtra("watched")?.let { watched ->
            val marks = watched.split('/').mapNotNull { mark ->
                val parts = mark.split(':', limit = 2)
                val id = parts.firstOrNull()?.toUIntOrNull()
                if (parts.size != 2 || id == null) {
                    Log.w(AnistreamApp.TAG, "watched takes <id>:<episodes>, not $mark")
                    null
                } else {
                    id to parts[1].split(',')
                }
            }
            for ((id, episodes) in marks) store.setWatched(id, episodes)
            // Asked again after the marks, which the engine takes in order.
            store.loadContinue(force = true)
            for ((id, _) in marks) store.loadDetail(id)
        }
        for (later in listOf("sheet", "player")) {
            intent.getStringExtra(later)?.let { Log.w(AnistreamApp.TAG, "$later=$it is not in this build yet") }
        }
    }
}
