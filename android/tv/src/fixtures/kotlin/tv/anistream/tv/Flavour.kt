// The fixtures flavour's launch: the testing engine on a scene, steered by intent extras under
// the phone's names:
//
//   scene     watching (the default), sourceless, fresh or playing
//   media     what every episode streams: a path or URL on the device, by default the
//             flavour's own episode.mp4, which seeks, so a resume starts where it should at once
//   section   the destination to open on: search, continue, calendar, seasonal, library or settings
//   title     a title page to open, by id
//   query     a search to type, on Search
//   firstLaunch  true to show first launch's pages, which fixtures leave out otherwise
//   update    a made-up release staged for the update's page, which opens over the section:
//             available, allowing, downloading, failed or storage; nothing is fetched or installed

package tv.anistream.tv

import android.content.Context
import android.content.Intent
import android.util.Log
import java.io.File
import kotlinx.coroutines.CoroutineScope
import tv.anistream.store.EngineLaunch
import tv.anistream.store.FixtureScene
import tv.anistream.store.enginePlatform
import tv.anistream.store.fixtureMedia
import tv.anistream.store.fixtures
import tv.anistream.tv.shell.Destination

internal object Flavour {
    fun launch(context: Context, intent: Intent): EngineLaunch {
        val named = intent.getStringExtra("scene")
        val scene = named?.let(FixtureScene::named) ?: FixtureScene.WATCHING
        if (named != null && FixtureScene.named(named) == null) Log.w(TvApp.TAG, "no scene named $named")
        fixtureMedia = intent.getStringExtra("media") ?: File(context.cacheDir, EPISODE).path
        return EngineLaunch.fixtures(scene, context.cacheDir).copy(platform = enginePlatform(context))
    }

    /**
     * Off the main thread, before the engine starts: the flavour's episode, copied out of the APK
     * the first time, for mpv to open as a file: 24 minutes of colour bars with a counter of
     * seconds, at 2 frames a second and a keyframe every 10 s, 330 KB. Made with ffmpeg:
     *
     *     ffmpeg -f lavfi -i smptebars=duration=1440:size=320x240:rate=2 \
     *       -f lavfi -i testsrc=duration=1440:size=320x240:rate=2 \
     *       -filter_complex "[1:v]crop=120:76:196:82[n];[0:v][n]overlay=100:82" \
     *       -c:v libx264 -preset veryslow -crf 30 -g 20 -pix_fmt yuv420p -movflags +faststart \
     *       -map_metadata -1 -fflags +bitexact -flags:v +bitexact episode.mp4
     */
    fun prepare(context: Context) {
        val file = File(context.cacheDir, EPISODE)
        if (!file.exists()) {
            val partial = File(context.cacheDir, "$EPISODE.part")
            context.assets.open(EPISODE).use { from -> partial.outputStream().use { from.copyTo(it) } }
            partial.renameTo(file)
        }
    }

    private const val EPISODE = "episode.mp4"

    fun start(intent: Intent): Start {
        val query = intent.getStringExtra("query")
        val section = Destination.named(intent.getStringExtra("section"))
        return Start(
            destination = section ?: if (query != null) Destination.Search else Destination.Continue,
            title = intent.getStringExtra("title")?.toUIntOrNull(),
            query = query,
            firstLaunch = intent.getBooleanExtra("firstLaunch", false),
            update = UpdateStage.named(intent.getStringExtra("update")) != null,
        )
    }

    /** The fixtures never check for, fetch or install an update: only a staged one, for its page. */
    fun updater(app: TvApp, scope: CoroutineScope, intent: Intent): UpdateLauncher? =
        UpdateStage.named(intent.getStringExtra("update"))?.let { StagedUpdate(app.store.updates, scope, it, app.cacheDir.resolve("staged-update")) }

    const val UPDATES = false
}
