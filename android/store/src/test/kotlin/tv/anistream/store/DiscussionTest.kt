// An episode's discussion in the store: what the engine says of it, kept for the sheet and the rows.

package tv.anistream.store

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.DiscussionState
import tv.anistream.engine.model.DiscussionView
import tv.anistream.engine.model.Event

@OptIn(ExperimentalCoroutinesApi::class)
class DiscussionTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), jobs = JobsModel(now = { 0L }), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun view(media: UInt, episode: UInt, state: DiscussionState, count: UInt = 214u) = DiscussionView(
        media = media, episode = episode, title = "Episode $episode Discussion", commentCount = count,
        url = "https://anilist.example/forum/thread/1", comments = emptyList(), state = state,
        canPost = false, signedIn = false, loadImages = false,
    )

    @Test
    fun an_episode_found_is_counted_for_its_row_and_menu() {
        assertNull(store.discussion.count(900001u, 7u))
        store.apply(listOf(Event.DiscussionFound(900001u, 7u, 214u)))
        assertEquals(214u, store.discussion.count(900001u, 7u))
        // Another episode and another title know nothing of it.
        assertNull(store.discussion.count(900001u, 8u))
        assertNull(store.discussion.count(900002u, 7u))
        // Said again, it is the newer count.
        store.apply(listOf(Event.DiscussionFound(900001u, 7u, 215u)))
        assertEquals(215u, store.discussion.count(900001u, 7u))
    }

    @Test
    fun the_open_discussion_is_said_whole_each_time_and_forgotten_on_close() {
        store.apply(listOf(Event.Discussion(view(900001u, 7u, DiscussionState.Loading))))
        assertEquals(DiscussionState.Loading, store.discussion.open.value?.state)
        store.apply(listOf(Event.Discussion(view(900001u, 7u, DiscussionState.Ready(more = false)))))
        assertEquals(DiscussionState.Ready(more = false), store.discussion.open.value?.state)
        store.closeDiscussion()
        assertNull(store.discussion.open.value)
    }

    @Test
    fun asking_to_read_a_discussion_is_remembered_until_it_is_dismissed() {
        store.discussion.show(900001u, 7u)
        assertEquals(DiscussionKey(900001u, 7u), store.discussion.target.value)
        store.discussion.dismiss()
        assertNull(store.discussion.target.value)
    }

    @Test
    fun opening_and_closing_go_to_the_engine() {
        store.openDiscussion(900001u, 7u)
        store.closeDiscussion()
        assertEquals(listOf(Command.OpenDiscussion(900001u, 7u), Command.CloseDiscussion), store.waiting)
    }

    @Test
    fun playerOffersTheDiscussionFromEightyFivePercent() {
        val late = PlayerDiscussion()
        assertEquals(false, late.see(0.0, null))
        assertEquals(false, late.see(1000.0, 0.0))
        assertEquals(false, late.see(1199.0, 1440.0))
        assertEquals(true, late.see(1224.0, 1440.0))
    }

    @Test
    fun playerKeepsOfferingAfterASeekBack() {
        val late = PlayerDiscussion()
        late.see(1300.0, 1440.0)
        assertEquals(true, late.see(10.0, 1440.0))
        assertEquals(false, PlayerDiscussion().see(10.0, 1440.0))
    }

    @Test
    fun playerMenuNeedsTheSettingAndANumberedEpisode() {
        assertEquals(true, PlayerDiscussion.offered(reached = true, enabled = true, episode = 7u))
        assertEquals(false, PlayerDiscussion.offered(reached = false, enabled = true, episode = 7u))
        assertEquals(false, PlayerDiscussion.offered(reached = true, enabled = false, episode = 7u))
        assertEquals(false, PlayerDiscussion.offered(reached = true, enabled = true, episode = null))
        assertEquals(7u, PlayerDiscussion.episode("7"))
        assertNull(PlayerDiscussion.episode("7.5"))
        assertNull(PlayerDiscussion.episode(null))
    }
}
