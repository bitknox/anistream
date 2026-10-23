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
import tv.anistream.engine.model.StorePluginRow
import tv.anistream.engine.model.StorePluginStatus

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
    fun a_stores_open_link_is_read_whole_with_its_mark() {
        val open = "https://plugins.example/join/abc123?open#k3y"
        assertEquals(open, InviteLinks.invite(open))
        assertEquals(open, InviteLinks.invite("anistream://plugins.example/join/abc123?open#k3y"))
        assertEquals(open, InviteLinks.invite("anistream://join?link=https%3A%2F%2Fplugins.example%2Fjoin%2Fabc123%3Fopen%23k3y"))
        assertTrue(InviteLinks.looksLikeInvite(open))
    }

    @Test
    fun a_preview_of_an_open_link_says_anyone_can_join() {
        val open = "https://plugins.example/join/abc123?open#k3y"
        val invited = InvitedStore(link = open, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false, open = true, anonymous = true)
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        val shown = (store.stores.preview.value as StoresModel.Preview.Store).store
        assertTrue(shown.open && shown.anonymous)
    }

    @Test
    fun a_preview_says_who_runs_the_store_and_where_each_plugin_stands() {
        val invited = InvitedStore(link = https, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false)
        assertEquals("plugins.example", invited.runByLine)
        assertEquals("Run by Ren, Sora · plugins.example", invited.copy(owners = listOf("Ren", "Sora")).runByLine)
        val row = StorePluginRow(id = "a", name = "A", version = "2.0.0", hosts = listOf("a.example"), status = StorePluginStatus.Available)
        assertNull(row.standing)
        assertEquals("Installed", row.copy(status = StorePluginStatus.Installed).standing)
        assertEquals("1.4.0 installed", row.copy(status = StorePluginStatus.Update("1.4.0", emptyList())).standing)
        assertEquals("Added by hand", row.copy(status = StorePluginStatus.AddedByHand).standing)
        assertEquals("From Kumo", row.copy(status = StorePluginStatus.OtherStore("Kumo")).standing)
    }

    @Test
    fun a_checked_invite_joins_and_the_sheet_moves_into_the_store() {
        store.stores.openJoin(https)
        assertEquals(https, store.stores.joining.value)
        store.previewStore(" $https ")
        assertEquals(StoresModel.Preview.Checking(https), store.stores.preview.value)
        val invited = InvitedStore(link = https, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false, deviceLabel = "Android phone")
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        assertEquals(StoresModel.Preview.Store(invited), store.stores.preview.value)

        store.joinStore(https)
        assertEquals(Command.JoinStore(https, "Android phone"), store.waiting.last())
        store.apply(listOf(Event.StoreJoined("hotaru")))
        assertEquals("hotaru", store.stores.joined.value)
        assertNull(store.stores.joining.value)
        assertNull(store.stores.preview.value)
    }

    @Test
    fun an_invite_offers_the_kind_of_device_and_joins_with_the_name_typed() {
        store.previewStore(https)
        val invited = InvitedStore(link = https, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false, deviceLabel = "Android tablet")
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        // The kind of device, never its own name, as the field starts.
        assertEquals("Android tablet", store.stores.deviceName.value)
        // Typed over, and kept when the link is checked again.
        store.stores.nameDevice("Den")
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        assertEquals("Den", store.stores.deviceName.value)
        store.joinStore(" $https ")
        assertEquals(Command.JoinStore(https, "Den"), store.waiting.last())
        // The sheet closed: nothing typed is kept for the next one.
        store.stores.closeJoin()
        assertEquals("", store.stores.deviceName.value)
    }

    @Test
    fun an_open_link_joins_with_no_name() {
        store.previewStore(https)
        val open = InvitedStore(link = https, name = "Hotaru Plugins", url = "https://plugins.example", host = "plugins.example", fingerprint = "9f3c 1a2b 77de 0c41", alreadyJoined = false, open = true, anonymous = true)
        store.apply(listOf(Event.StorePreview(InvitePreview.Store(open))))
        store.joinStore(https)
        assertEquals(Command.JoinStore(https, null), store.waiting.last())
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
