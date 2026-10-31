// How the fixtures flavour starts the testing engine.

package tv.anistream.store

import java.io.File
import java.util.UUID
import tv.anistream.engine.ffi.FfiDirs
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.ffi.FfiScene
import tv.anistream.engine.ffi.FfiStart
import tv.anistream.engine.ffi.seedFixtureDownloads

/** The testing engine on [scene]. */
internal fun startFixtures(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine =
    startScene(scene, dirs, host, start)

/**
 * The testing engine on [scene], through the engine's scenes: the fake AniList answering at the
 * real time, with its art, and the scene's history seeded in the database first.
 */
internal fun startScene(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine {
    fixtureDownloadClip?.let { seedFixtureDownloads(dirs, it) }
    return FfiEngine.startScene(dirs, host, scene.ffi(), start)
}

/** The engine's own scene for [this]. */
private fun FixtureScene.ffi(): FfiScene = when (this) {
    FixtureScene.FRESH -> FfiScene.Fresh
    // With a file to stream, every episode streams it, as the Playing scene's do.
    FixtureScene.WATCHING -> if (fixtureMedia.isEmpty()) FfiScene.Watching else FfiScene.WatchingMedia(media = fixtureMedia)
    FixtureScene.SOURCELESS -> FfiScene.Sourceless
    FixtureScene.UNSURE -> FfiScene.Unsure
    // What it streams is the player's business, which comes with A3: a path or URL on the device.
    FixtureScene.PLAYING -> FfiScene.Playing(media = fixtureMedia)
    FixtureScene.SUGGESTED -> FfiScene.Suggested
    FixtureScene.ONE_LEFT -> FfiScene.OneLeft
    FixtureScene.CONTINUE_FULL -> FfiScene.ContinueFull
    FixtureScene.NOT_FOUND -> FfiScene.NotFound
    FixtureScene.SLOW -> FfiScene.Slow
    FixtureScene.FAILED -> FfiScene.Failed
    FixtureScene.OFFLINE -> FfiScene.Offline
}

/** What the Playing scene streams, and Watching's when set: the fixtures' `media` extra, set at launch. */
@Volatile
var fixtureMedia: String = ""

/**
 * The file the fixture downloads' finished episode plays, when the fixtures' `downloads` extra
 * asked for them: a finished one, one running and one waiting are seeded before the scene starts.
 */
@Volatile
var fixtureDownloadClip: String? = null

/** Whether this build has the testing engine in it. */
const val HAS_TESTING_ENGINE = true

/**
 * The fixtures flavour's launch on [scene], in a new directory under [cache]: a scene seeds the
 * database it starts on, so every launch starts from an empty one. The directories earlier
 * launches left are removed first.
 */
fun EngineLaunch.Companion.fixtures(scene: FixtureScene, cache: File): EngineLaunch {
    val parent = cache.resolve("fixtures")
    parent.listFiles()?.forEach { it.deleteRecursively() }
    val root = parent.resolve(UUID.randomUUID().toString())
    return EngineLaunch(engine = EngineLaunch.Engine.Fixtures(scene), home = EngineHome.override(root))
}
