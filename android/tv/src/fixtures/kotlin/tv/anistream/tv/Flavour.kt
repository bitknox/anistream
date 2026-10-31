// The fixtures flavour's launch: the testing engine on a scene, steered by intent extras under
// the phone's names:
//
//   scene     watching (the default), sourceless, fresh or playing
//   media     what every episode streams: a path or URL on the device, by default the
//             flavour's own episode.mp4, which seeks, so a resume starts where it should at once
//   section   the destination to open on: search, continue, calendar, seasonal, library or settings
//   title     a title page to open, by id
//   query     a search to type, on Search
//   signin    a tracker whose device-code sign-in opens, with a made-up code that expires in 15 minutes
//   expires   seconds the `signin` code has left, 0 for one already expired
//   day       days after today the calendar opens on, 0 to 6, for a day with broadcasts: the
//             fixtures' own day has none
//   firstLaunch  true to show first launch's pages, which fixtures leave out otherwise
//   update    a made-up release staged for the update's page, which opens over the section:
//             available, allowing, downloading, failed or storage; nothing is fetched or installed

package tv.anistream.tv

import android.content.Context
import android.content.Intent
import android.util.Log
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.engine.model.Event
import tv.anistream.store.AppStore
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
        if (intent.getStringExtra("world") == "1") android.system.Os.setenv("ANISTREAM_FIXTURES_HOME_WORLD", "1", true)
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
            destination = section ?: if (query != null) Destination.Search else Destination.Home,
            title = intent.getStringExtra("title")?.toUIntOrNull(),
            query = query,
            firstLaunch = intent.getBooleanExtra("firstLaunch", false),
            update = UpdateStage.named(intent.getStringExtra("update")) != null,
            signIn = intent.getStringExtra("signin"),
        )
    }

    /** The fixtures never check for, fetch or install an update: only a staged one, for its page. */
    fun updater(app: TvApp, scope: CoroutineScope, intent: Intent): UpdateLauncher? =
        UpdateStage.named(intent.getStringExtra("update"))?.let { StagedUpdate(app.store.updates, scope, it, app.cacheDir.resolve("staged-update")) }

    const val UPDATES = false

    /**
     * The device-code prompt a `signin` extra asks for, kept up for the screen: the testing engine
     * has no tracker to answer with one. `expires=<seconds>` sets how long its code has left, 0 for
     * one already expired, 899 unless it says.
     */
    fun started(store: AppStore, intent: Intent) {
        intent.getStringExtra("day")?.toIntOrNull()?.let { store.calendar.opensOn = it }
        // Episodes of the first fixture marked watched, 1,2,3: playback would also have the engine
        // look their threads up, so the one with a fixture thread is said found as it would be.
        intent.getStringExtra("watched")?.let { episodes ->
            val marks = episodes.split(',').filter(String::isNotBlank)
            store.setWatched(900001u, marks)
            store.loadContinue(force = true)
            store.loadDetail(900001u)
            if ("7" in marks) store.apply(listOf(Event.DiscussionFound(900001u, 7u, 6u)))
        }
        val tracker = intent.getStringExtra("signin") ?: return
        val expiresAt = System.currentTimeMillis() / 1000 + (intent.getStringExtra("expires")?.toLongOrNull()?.coerceAtLeast(0) ?: 899)
        MainScope().launch {
            while (true) {
                if (store.accounts.deviceCode.value == null) {
                    store.apply(Event.DeviceCode(DeviceCodePrompt(tracker, "4F2K9QX1", "https://activate.example/pin", expiresAt)))
                }
                delay(500)
            }
        }
    }
}
