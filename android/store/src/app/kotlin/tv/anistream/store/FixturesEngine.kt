// The app flavour has no testing engine: its engine is built without the `testing` feature.

package tv.anistream.store

import uniffi.anistream_ffi.FfiDirs
import uniffi.anistream_ffi.FfiEngine
import uniffi.anistream_ffi.FfiException
import uniffi.anistream_ffi.FfiHost
import uniffi.anistream_ffi.FfiStart

internal fun startFixtures(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine =
    throw FfiException.Engine("this build has no testing engine")

/** Whether this build has the testing engine in it. */
const val HAS_TESTING_ENGINE = false
