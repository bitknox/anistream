// The fixtures' TVs: two made-up TVs on the loopback interface, each a real remote server with a
// scripted player behind it (`startFakeTv`), found by a browse that lists them as DNS-SD would.
// They pair, take commands and send their state as a TV does; what they play is made up.

package tv.anistream.store

import android.util.Log
import tv.anistream.engine.ffi.FfiDiscoverySink
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiException
import tv.anistream.engine.ffi.FfiFakeTv
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.engine.ffi.FfiTvException

/** A browse that finds the fake TVs. */
class FixtureTvBrowser(val tvs: List<FfiFakeTv>) : FfiTvBrowser {
    override fun start(sink: FfiDiscoverySink) {
        for (tv in tvs) {
            val advert = tv.advert()
            sink.found(advert.instance, listOf(tv.host()), tv.port(), advert.txt)
        }
    }

    override fun stop() {}
}

object FixtureTvs {
    /** The fake TVs, Living room and Bedroom, started on [engine], and the browse that finds them. */
    fun start(engine: FfiEngine): FixtureTvBrowser? = try {
        FixtureTvBrowser(listOf(engine.startFakeTv("Living room"), engine.startFakeTv("Bedroom")))
    } catch (e: FfiException) {
        Log.w("anistream", "the fake TVs did not start: ${e.message}")
        null
    }

    /** The QR link of the [index]th fake TV in [browser], with a code it has open: it pairs at once. */
    fun openLink(browser: FfiTvBrowser, index: Int): String? =
        (browser as? FixtureTvBrowser)?.tvs?.getOrNull(index)?.let { it.pairLink(it.openCode()) }

    /**
     * Set up [scene] on [model], whose browse is [fakes]: Bedroom paired first in every scene, then
     * the picker, Living room's code, or Living room playing [play] with its card open.
     */
    suspend fun stage(model: TvRemoteModel, scene: TvScene, browser: FfiTvBrowser, play: TvPlayRequest) {
        val sender = model.attachedSender ?: return
        val fakes = browser as? FixtureTvBrowser ?: return
        if (fakes.tvs.size != 2) return
        val (living, bedroom) = fakes.tvs
        try {
            sender.pair(bedroom.pairLink(bedroom.openCode())).pair(null)
            model.refresh()
            when (scene) {
                TvScene.PAIRED -> Unit
                TvScene.PICKER -> model.openPicker()
                TvScene.CODE -> {
                    model.openPicker()
                    model.begin(model.rows.value.firstOrNull { it.name == "Living room" }?.key ?: living.pairLink(null))
                }
                TvScene.REMOTE -> {
                    val tv = sender.pair(living.pairLink(living.openCode())).pair(null)
                    model.refresh()
                    model.offer(play)
                    model.use(tv.fingerprint)
                }
            }
        } catch (e: FfiTvException) {
            Log.w("anistream", "the TV scene did not set up: $e")
        }
    }
}
