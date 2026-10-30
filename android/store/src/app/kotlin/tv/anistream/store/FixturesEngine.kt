// The app flavour has no testing engine: its engine is built without the `testing` feature.

package tv.anistream.store

import tv.anistream.engine.ffi.FfiDirs
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiException
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.ffi.FfiStart
import tv.anistream.engine.model.Msg

internal fun startFixtures(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine =
    throw FfiException.Engine(reason = "this build has no testing engine", msg = Msg.NoticeEngineFault)

/** Whether this build has the testing engine in it. */
const val HAS_TESTING_ENGINE = false
