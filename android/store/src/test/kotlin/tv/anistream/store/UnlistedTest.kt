package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.ProviderRow

/** A title no source has: its episodes AniList's alone, until a source is ready. */
class UnlistedTest {
    private fun row(metadataOnly: Boolean) = EpisodeRow(
        number = "1", title = null, thumbnail = null, description = null, durationSecs = null, watched = 0.0,
        completed = false, kind = null, skippable = false, airsAt = null, metadataOnly = metadataOnly,
    )

    private fun source(usable: Boolean) = ProviderRow(
        id = "fixture", kind = "plugin", state = "ready", latencyMs = null, lastError = null, usable = usable,
        heldBack = false, name = "fixture", version = null, hosts = emptyList(), origin = ProviderOrigin.ByHand,
        disabled = false, update = null,
    )

    @Test
    fun a_source_ready_where_none_was_asks_again_for_episodes_that_were_anilists_alone() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        store.loadEpisodes(900001u)
        store.loadEpisodes(900005u)
        store.apply(Event.Episodes(900001u, listOf(row(metadataOnly = true))))
        store.apply(Event.Episodes(900005u, listOf(row(metadataOnly = false))))
        assertTrue(store.titles.unlisted(900001u))
        assertFalse("a source listed it", store.titles.unlisted(900005u))
        val asked = listOf<Command>(Command.LoadEpisodes(900001u), Command.LoadEpisodes(900005u))
        assertEquals(asked, store.waiting)

        store.apply(Event.Providers(listOf(source(usable = false))))
        assertEquals("none ready yet", asked, store.waiting)
        // Ready now: asked again at once, though it was asked already.
        store.apply(Event.Providers(listOf(source(usable = true))))
        assertEquals(asked + Command.LoadEpisodes(900001u), store.waiting)
        store.apply(Event.Providers(listOf(source(usable = true))))
        assertEquals("ready already: nothing new", 3, store.waiting.size)
    }
}
