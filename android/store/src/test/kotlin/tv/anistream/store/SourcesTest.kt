package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event

/** An episode's releases, listed for the title page's Choose source and the player's. */
class SourcesTest {
    private val release = SourceCandidate("r1", "fixture", "Release one", 1080u, null, "1.2 GiB", false, false, true)

    @Test
    fun the_releases_listed_reach_the_choice_for_their_episode_and_the_player() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val heard = mutableListOf<Event>()
        store.onPlayback = { heard += it }
        store.loadSources(900001u, "7")
        assertEquals(listOf<Command>(Command.LoadSources(900001u, "7")), store.waiting)
        assertEquals(SourceChoices(900001u, "7"), store.sources.value)

        // Another episode's answer is not this choice's.
        store.apply(Event.Sources(900001u, "8", listOf(release)))
        assertNull(store.sources.value?.candidates)
        store.apply(Event.Sources(900001u, "7", listOf(release)))
        assertEquals(listOf(release), store.sources.value?.candidates)
        assertEquals(2, heard.size)

        store.playSource(900001u, "7", release)
        assertNull(store.sources.value)
        assertEquals(Command.PlaySource(900001u, "7", "fixture", "r1"), store.waiting.last())
    }
}
