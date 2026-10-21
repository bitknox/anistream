// How the fixtures flavour starts the testing engine.

package tv.anistream.store

import java.io.File
import java.util.UUID
import tv.anistream.engine.ffi.FfiDirs
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.ffi.FfiStart

/** The testing engine on [scene]. */
internal fun startFixtures(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine =
    startScene(scene, dirs, host, start)

/**
 * The one place a scene picks how the testing engine starts.
 *
 * The engine's scenes (`start_scene`, which seeds the database for Watching and Playing) are not on
 * main yet, so for now every scene starts as `start_testing` does: a first launch, the Fresh scene.
 * Once they are, this becomes `FfiEngine.startScene(dirs, host, scene.ffi, start)`.
 */
internal fun startScene(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine {
    if (scene != FixtureScene.FRESH) {
        System.err.println("anistream: the ${scene.name.lowercase()} scene starts as fresh until the engine has scenes")
    }
    return FfiEngine.startTesting(dirs, host, start)
}

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
