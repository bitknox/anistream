// The fixtures flavour's launch: the testing engine on a scene, each launch in a directory of its
// own, steered by intent extras under the names of the Apple app's -Anistream* arguments:
//
//   scene     watching (the default), fresh or playing
//   media     what the playing scene streams: a path or URL on the device
//   section   the destination to open on: continue, calendar, seasonal, library or search
//   watched   episodes to mark watched once the engine is up: 900006:8, or 900001:1,2,3, with more
//             titles joined by / (900006:1,2/900002:1), oldest first
//   title     a title page to open, by id
//   query     a search to run, on Search
//   page      a page over the destination, by its route: settings, settings/playback
//   downloads any value: three downloads seeded, one finished, whose file is the fixtures' clip
//             in the app's download folder, one running and one waiting
//   notify    any value: two new-episode notifications posted for fixture titles, grouped, as the
//             periodic check posts them (the notification permission granted first)
//   sheet     a sheet to open: torrent, join, kitsu, update or show (from A2)
//   player    a player state to shoot (from A3)

package tv.anistream.app

import android.content.Context
import android.content.Intent
import android.os.Environment
import android.util.Log
import java.util.UUID
import tv.anistream.store.AppStore
import tv.anistream.store.EngineHome
import tv.anistream.store.NewEpisode
import tv.anistream.store.EngineLaunch
import tv.anistream.store.FixtureScene
import tv.anistream.store.fixtureDownloadClip
import tv.anistream.store.fixtureMedia
import tv.anistream.store.fixtures

internal object Flavour {
    const val FIXTURES = true

    private var marked = false

    fun launch(context: Context, intent: Intent): EngineLaunch {
        val named = intent.getStringExtra("scene")
        val scene = named?.let(FixtureScene::named) ?: FixtureScene.WATCHING
        if (named != null && FixtureScene.named(named) == null) Log.w(AnistreamApp.TAG, "no scene named $named")
        intent.getStringExtra("media")?.let { fixtureMedia = it }
        if (intent.getStringExtra("downloads") != null) fixtureDownloadClip = downloadClip(context)
        return EngineLaunch.fixtures(scene, context.cacheDir)
    }

    /**
     * The fixtures' clip, copied out of the APK into the app's download folder, where a finished
     * download is kept: what the seeded finished episode plays, offline.
     */
    private fun downloadClip(context: Context): String? = runCatching {
        val folder = (context.getExternalFilesDir(Environment.DIRECTORY_MOVIES) ?: context.filesDir.resolve("downloads"))
            .resolve("fixtures").apply { mkdirs() }
        val file = folder.resolve("episode-06.mkv")
        context.assets.open("fixture-download.mkv").use { input -> file.outputStream().use { input.copyTo(it) } }
        file.path
    }.onFailure { Log.w(AnistreamApp.TAG, "the fixture clip was not copied: ${it.message}") }.getOrNull()

    /**
     * The engine background work starts while the app is not running: the watching scene, in a
     * directory of its own, never the one a launch is using.
     */
    fun background(context: Context): EngineLaunch {
        val parent = context.cacheDir.resolve("fixtures-background")
        parent.listFiles()?.forEach { it.deleteRecursively() }
        return EngineLaunch(
            engine = EngineLaunch.Engine.Fixtures(FixtureScene.WATCHING),
            home = EngineHome.override(parent.resolve(UUID.randomUUID().toString())),
        )
    }

    /** What the extras ask of the engine once it is up, once per process. */
    fun started(store: AppStore, intent: Intent, notifier: EpisodeNotifier) {
        if (marked) return
        marked = true
        if (intent.getStringExtra("notify") != null) {
            val now = System.currentTimeMillis() / 1000
            notifier.post(
                listOf(
                    NewEpisode(900001u, "Stardust Wayfarer", 8u, now - 3_600),
                    NewEpisode(900006u, "Summer Wind Relay", 3u, now - 600),
                ),
            )
        }
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
