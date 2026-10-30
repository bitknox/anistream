// The fixtures flavour's launch: the testing engine on a scene, each launch in a directory of its
// own, steered by intent extras under the names of the Apple app's -Anistream* arguments:
//
//   scene     watching (the default), sourceless (watching with no source set up), fresh or playing
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
//   check     seconds: the new-episode check run once that many seconds from now, as the periodic
//             work runs it, 0 for at once; kill the app first to have it start a quiet engine
//   widgets   which widgets to offer the launcher to pin, joined by commas: continue-small,
//             continue-wide, airing-small, airing-wide (the launcher asks before each); and the
//             widgets' record made at once, with its art
//   tv        a Play on TV scene, on the two fake TVs: paired, picker, code or remote (TvScene)
//   discussion an episode number of the first fixture whose discussion opens in its sheet
//   sheet     a sheet to open: torrent, join, kitsu, update or show (from A2)
//   player    a player state to shoot (from A3)
//   code      a tracker whose device-code sheet opens, with a made-up code that expires in 15 minutes
//   expires   seconds the `code` sheet's code has left, 0 for one already expired
//   day       days after today the calendar opens on, for a day with broadcasts: the fixtures'
//             own day has none

package tv.anistream.app

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Environment
import android.util.Log
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.engine.model.DiscussionState
import tv.anistream.engine.model.Event
import tv.anistream.store.EngineEpisodeCheck
import tv.anistream.store.covers
import tv.anistream.store.AppStore
import tv.anistream.store.EngineHome
import tv.anistream.store.NewEpisode
import tv.anistream.store.EngineLaunch
import tv.anistream.store.enginePlatform
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.store.FixtureScene
import tv.anistream.store.FixtureTvs
import tv.anistream.store.TvPlayRequest
import tv.anistream.store.TvScene
import tv.anistream.store.fixtureDownloadClip
import tv.anistream.store.fixtureMedia
import tv.anistream.store.fixtures
import tv.anistream.widgets.AiringSmallReceiver
import tv.anistream.widgets.AiringWideReceiver
import tv.anistream.widgets.ContinueSmallReceiver
import tv.anistream.widgets.ContinueWideReceiver

internal object Flavour {
    const val FIXTURES = true

    /** The fixtures flavour never installs anything: it has no permission to. */
    const val UPDATES = false

    private var marked = false

    fun launch(context: Context, intent: Intent): EngineLaunch {
        val named = intent.getStringExtra("scene")
        val scene = named?.let(FixtureScene::named) ?: FixtureScene.WATCHING
        if (named != null && FixtureScene.named(named) == null) Log.w(AnistreamApp.TAG, "no scene named $named")
        intent.getStringExtra("media")?.let { fixtureMedia = it }
        if (intent.getStringExtra("downloads") != null) fixtureDownloadClip = downloadClip(context)
        return EngineLaunch.fixtures(scene, context.cacheDir).copy(platform = enginePlatform(context))
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

    /**
     * Two new-episode notifications for fixture titles, as a check posts them, with the covers the
     * engine serves: what the `notify` extra asks for.
     */
    private fun notifyFixtures(store: AppStore, notifier: EpisodeNotifier) {
        MainScope().launch {
            val ids = listOf(900001u, 900006u)
            val entries = withTimeoutOrNull(60_000) { store.titles.entries.first { titles -> ids.all { it in titles } } } ?: return@launch
            val covers = store.covers(ids.mapNotNull { entries[it]?.coverUrl }, EngineEpisodeCheck.COVER_SIZE, 10_000)
            val now = System.currentTimeMillis() / 1000
            notifier.post(
                listOf(900001u to 8u to 3_600L, 900006u to 3u to 600L).map { (show, ago) ->
                    val (id, episode) = show
                    val entry = entries.getValue(id)
                    NewEpisode(id, entry.title, episode, now - ago, entry.coverUrl, entry.coverUrl?.let(covers::get))
                },
            )
        }
    }

    /**
     * Offer the launcher the widgets [names] names, for a look at them on a home screen, and make
     * their record now rather than at the next check.
     */
    private fun pinWidgets(context: Context, names: String) {
        val receivers = mapOf(
            "continue-small" to ContinueSmallReceiver::class.java,
            "continue-wide" to ContinueWideReceiver::class.java,
            "airing-small" to AiringSmallReceiver::class.java,
            "airing-wide" to AiringWideReceiver::class.java,
        )
        val manager = context.getSystemService(AppWidgetManager::class.java)
        for (name in names.split(',').map(String::trim).filter(String::isNotEmpty)) {
            val receiver = receivers[name] ?: run {
                Log.w(AnistreamApp.TAG, "no widget named $name: ${receivers.keys.joinToString()}")
                null
            } ?: continue
            if (manager?.isRequestPinAppWidgetSupported != true) {
                Log.w(AnistreamApp.TAG, "this launcher pins no widgets")
                break
            }
            manager.requestPinAppWidget(ComponentName(context, receiver), null, null)
        }
        val app = context.applicationContext as? AnistreamApp ?: return
        MainScope().launch {
            runCatching { app.makeWidgetRecord() }.onFailure { Log.w(AnistreamApp.TAG, "no widgets' record: ${it.message}") }
        }
    }

    /** The fake TVs, Living room and Bedroom, on the loopback interface: never the real network. */
    fun tvBrowser(context: Context, store: AppStore): FfiTvBrowser? = store.engine?.let(FixtureTvs::start)

    /**
     * The TV scene the `tv` extra names, set up once Play on TV is, and true once it is on
     * screen: what announceReady waits on besides the store. True with no scene asked for.
     */
    suspend fun reachedTv(intent: Intent, store: AppStore): Boolean {
        intent.getStringExtra("discussion")?.let { value ->
            // "row" waits for the thread to be found without opening it, for the episode row's quiet line.
            if (value == "row") {
                foundFixtureThread(store, 7u)
                delay(500)
            } else {
                value.toUIntOrNull()?.let { reachedDiscussion(store, it) }
            }
        }
        val scene = TvScene.named(intent.getStringExtra("tv")) ?: return true
        val browser = withTimeoutOrNull(60_000) {
            while (store.tv.attachedBrowser == null) delay(50)
            store.tv.attachedBrowser
        } ?: return false
        val id = 900001u
        val title = withTimeoutOrNull(30_000) { store.titles.entries.first { id in it }[id]?.title }.orEmpty()
        FixtureTvs.stage(store.tv, scene, browser, TvPlayRequest(id, title, "3"))
        return withTimeoutOrNull(10_000) {
            while (!store.tv.reached(scene)) delay(50)
            // The sheet's own animation in.
            delay(700)
            true
        } ?: false
    }

    /**
     * Marking episodes watched here does not make the engine look their threads up, as playback
     * does: say the fixture thread found, as the engine would, with its six comments.
     */
    private fun foundFixtureThread(store: AppStore, episode: UInt) {
        store.apply(listOf(Event.DiscussionFound(900001u, episode, 6u)))
    }

    /**
     * The discussion of [episode] of the first fixture, open in its sheet once the engine has
     * found its thread and read the first page: what the `discussion` extra asks for.
     */
    private suspend fun reachedDiscussion(store: AppStore, episode: UInt) {
        val id = 900001u
        foundFixtureThread(store, episode)
        store.discussion.show(id, episode)
        withTimeoutOrNull(30_000) { store.discussion.open.first { it?.state is DiscussionState.Ready } }
        // The sheet's own animation in, and the avatars.
        delay(1_200)
    }

    /** What the extras ask of the engine once it is up, once per process. */
    fun started(context: Context, store: AppStore, intent: Intent, notifier: EpisodeNotifier) {
        if (marked) return
        marked = true
        if (intent.getStringExtra("notify") != null) notifyFixtures(store, notifier)
        intent.getStringExtra("widgets")?.let { pinWidgets(context, it) }
        intent.getStringExtra("check")?.toLongOrNull()?.let { delay ->
            val once = OneTimeWorkRequestBuilder<NewEpisodesWorker>().setInitialDelay(delay, TimeUnit.SECONDS).build()
            WorkManager.getInstance(context).enqueue(once)
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
        intent.getStringExtra("day")?.toIntOrNull()?.let { store.calendar.opensOn = it }
        intent.getStringExtra("code")?.let { tracker ->
            val expiresAt = System.currentTimeMillis() / 1000 + codeSecondsLeft(intent)
            store.apply(Event.DeviceCode(DeviceCodePrompt(tracker, "4F2K9QX1", "https://activate.example/pin", expiresAt)))
        }
        for (later in listOf("sheet", "player")) {
            intent.getStringExtra(later)?.let { Log.w(AnistreamApp.TAG, "$later=$it is not in this build yet") }
        }
    }

    /** How long a `code` extra's prompt has left: `expires=<seconds>`, 0 for an expired code, or 899. */
    private fun codeSecondsLeft(intent: Intent): Long = intent.getStringExtra("expires")?.toLongOrNull()?.coerceAtLeast(0) ?: 899
}
