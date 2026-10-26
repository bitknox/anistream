package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind
import tv.anistream.engine.model.TorrentProtection
import tv.anistream.engine.model.TorrentProxy

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

    @Test
    fun an_alert_reaches_the_player_as_well_as_the_notices_and_an_info_does_not() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val heard = mutableListOf<Event>()
        store.onPlayback = { heard += it }
        val alert = Event.Toast(Notice(NoticeKind.ALERT, "no playable stream", null))
        store.apply(Event.Toast(Notice(NoticeKind.INFO, "looking", null)))
        store.apply(alert)
        assertEquals(listOf<Event>(alert), heard)
    }

    @Test
    fun a_protection_check_runs_one_at_a_time_and_its_verdict_lands_on_what_it_checked() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val first = TorrentProxy("proxy.example", 1080u, "mika", "one")
        val second = TorrentProxy("proxy.example", 1081u, "mika", null)
        val third = TorrentProxy("other.example", 1080u, null, null)
        store.checkTorrentProtection(TorrentProtection.PROXY, first)
        store.checkTorrentProtection(TorrentProtection.PROXY, second)
        store.checkTorrentProtection(TorrentProtection.PROXY, third)
        // Only the first goes to the engine; the newest of the rest waits for its verdict.
        assertEquals(listOf<Command>(Command.CheckTorrentProtection(TorrentProtection.PROXY, first)), store.waiting)

        // The first's verdict is no form's any more: the third is sent, and is still checking.
        store.apply(Event.ProtectionChecked(GuardView.Leaking))
        assertEquals(Command.CheckTorrentProtection(TorrentProtection.PROXY, third), store.waiting.last())
        val checking = store.providers.trial.value
        assertTrue(checking is ProvidersModel.Trial.Checking && checking.isFor(TorrentProtection.PROXY, third))

        store.apply(Event.ProtectionChecked(GuardView.Protected("203.0.113.24")))
        val said = store.providers.trial.value as ProvidersModel.Trial.Said
        assertTrue(said.isFor(TorrentProtection.PROXY, third))
        assertFalse(said.isFor(TorrentProtection.PROXY, first))
        assertEquals(GuardView.Protected("203.0.113.24"), said.view)

        // A verdict after its form let go is not kept, and the next check goes straight out.
        store.checkTorrentProtection(TorrentProtection.PROXY, first)
        store.providers.clearTrial()
        store.apply(Event.ProtectionChecked(GuardView.Protected("203.0.113.24")))
        assertNull(store.providers.trial.value)
        store.checkTorrentProtection(TorrentProtection.PROXY, second)
        assertEquals(Command.CheckTorrentProtection(TorrentProtection.PROXY, second), store.waiting.last())
    }
}
