// The app flavour has no fake TVs: its engine is built without the `testing` feature.

package tv.anistream.store

import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiTvBrowser

object FixtureTvs {
    fun start(@Suppress("UNUSED_PARAMETER") engine: FfiEngine): FfiTvBrowser? = null

    fun openLink(@Suppress("UNUSED_PARAMETER") browser: FfiTvBrowser, @Suppress("UNUSED_PARAMETER") index: Int): String? = null

    suspend fun stage(model: TvRemoteModel, scene: TvScene, fakes: FfiTvBrowser, play: TvPlayRequest) {}
}
