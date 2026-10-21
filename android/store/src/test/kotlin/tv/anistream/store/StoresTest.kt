package tv.anistream.store

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.InvitePreview
import tv.anistream.engine.model.InvitedStore

/** Invite links as the app is handed them, and joining a store with one. */
@OptIn(ExperimentalCoroutinesApi::class)
class StoresTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private val https = "https://plugins.example/join/abc123#k3y"

    @Test
    fun an_invite_is_read_from_each_form_the_app_is_handed() {
        assertEquals(https, InviteLinks.invite(https))
        assertEquals(https, InviteLinks.invite("anistream://plugins.example/join/abc123#k3y"))
        assertEquals(https, InviteLinks.invite("anistream://join?link=https%3A%2F%2Fplugins.example%2Fjoin%2Fabc123%23k3y"))
        // A sign-in's redirect, a page that is no invite, and words are none.
        assertNull(InviteLinks.invite("anistream://auth/anilist#access_token=a"))
        assertNull(InviteLinks.invite("https://plugins.example/about"))
        assertFalse(InviteLinks.looksLikeInvite("not a link"))
        assertTrue(InviteLinks.looksLikeInvite("  $https  "))
    }

    @Test
    fun a_checked_invite_joins_and_the_sheet_moves_into_the_store() {
        store.stores.openJoin(https)
        assertEquals(https, store.stores.joining.value)
        store.previewStore(" $https ")
        assertEquals(StoresModel.Preview.Checking(https), store.stores.preview.value)
        val invited = InvitedStore(link = https, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false)
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        assertEquals(StoresModel.Preview.Store(invited), store.stores.preview.value)

        store.joinStore(https)
        assertEquals(Command.JoinStore(https), store.waiting.last())
        store.apply(listOf(Event.StoreJoined("hotaru")))
        assertEquals("hotaru", store.stores.joined.value)
        assertNull(store.stores.joining.value)
        assertNull(store.stores.preview.value)
    }

    @Test
    fun an_invite_that_leads_nowhere_says_why() {
        store.previewStore(https)
        store.apply(listOf(Event.StorePreview(InvitePreview.Failed("the invite was used already"))))
        assertEquals(StoresModel.Preview.Failed("the invite was used already"), store.stores.preview.value)
        store.stores.closeJoin()
        assertNull(store.stores.preview.value)
    }

    @Test
    fun an_install_is_under_way_until_its_job_ends() {
        store.stores.started(StoresModel.Work.Install("hotaru", "mukashi"), job = 7uL)
        assertTrue(store.stores.isWorking(store.stores.working.value, StoresModel.Work.Install("hotaru", "mukashi")))
        store.apply(listOf(Event.JobDone(7uL)))
        assertFalse(store.stores.isWorking(store.stores.working.value, StoresModel.Work.Install("hotaru", "mukashi")))
    }
}
