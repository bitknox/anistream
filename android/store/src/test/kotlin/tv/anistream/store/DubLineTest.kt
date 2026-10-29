package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.DubExtent
import tv.anistream.engine.model.Event

/** How far a title's dub goes: asked for once, and worded from the catalogue. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class DubLineTest {
    @Test
    fun the_dub_is_asked_for_once_and_shown_when_it_lands() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        store.loadDub(900001u)
        store.loadDub(900001u)
        assertEquals(listOf<Command>(Command.LoadDub(900001u)), store.waiting)
        assertNull("nothing shows before the engine answers", store.titles.dubLines.value[900001u])

        store.apply(Event.TitleDub(900001u, DubExtent(through = 6u, complete = false)))
        assertEquals("Dubbed to episode 6", store.titles.dubLines.value[900001u]?.line(RuntimeEnvironment.getApplication()))
        store.apply(Event.TitleDub(900001u, null))
        assertNull("a dub the source cannot say shows nothing", store.titles.dubLines.value[900001u])
    }
}
