// The fixtures flavour's launch: the testing engine on a scene, steered by intent extras under
// the phone's names:
//
//   scene     watching (the default), sourceless, fresh or playing
//   media     what the playing scene streams: a path or URL on the device
//   section   the destination to open on: search, continue, calendar, seasonal, library or settings
//   title     a title page to open, by id
//   query     a search to type, on Search

package tv.anistream.tv

import android.content.Context
import android.content.Intent
import android.util.Log
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
        intent.getStringExtra("media")?.let { fixtureMedia = it }
        return EngineLaunch.fixtures(scene, context.cacheDir).copy(platform = enginePlatform(context))
    }

    fun start(intent: Intent): Start {
        val query = intent.getStringExtra("query")
        val section = Destination.named(intent.getStringExtra("section"))
        return Start(
            destination = section ?: if (query != null) Destination.Search else Destination.Continue,
            title = intent.getStringExtra("title")?.toUIntOrNull(),
            query = query,
        )
    }
}
