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
import tv.anistream.engine.model.ConflictRow
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.engine.model.Event

/** The sign-ins as the store holds them: the browser's, a device code's, and what a pull disputes. */
@OptIn(ExperimentalCoroutinesApi::class)
class SignInTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private val authorize = Event.Authorize(
        tracker = "anilist",
        url = "https://anilist.co/api/v2/oauth/authorize?client_id=54537&response_type=token",
        callbackScheme = "anistream",
    )

    @Test
    fun the_browser_s_redirect_goes_to_the_engine_as_it_came_and_ends_the_sign_in() {
        store.connect("anilist")
        store.apply(listOf(authorize))
        val request = store.accounts.signIn.value!!
        assertEquals("anilist", request.tracker)
        assertEquals(authorize.url, request.url)

        val callback = "anistream://auth/anilist#access_token=a.b.c&token_type=Bearer"
        store.finishSignIn(request, callback)
        assertNull(store.accounts.signIn.value)
        assertEquals(listOf(Command.Connect("anilist"), Command.FinishSignIn("anilist", callback)), store.waiting)
    }

    @Test
    fun a_tab_closed_without_signing_in_says_so() {
        store.apply(listOf(authorize))
        store.cancelSignIn(store.accounts.signIn.value!!)
        assertNull(store.accounts.signIn.value)
        assertEquals(listOf(Command.CancelSignIn("anilist")), store.waiting)
    }

    @Test
    fun a_device_code_stays_until_the_engine_clears_it() {
        val prompt = DeviceCodePrompt(tracker = "trakt", code = "4F2K9QX1", url = "https://trakt.example/activate")
        store.apply(listOf(Event.DeviceCode(prompt)))
        assertEquals(prompt, store.accounts.deviceCode.value)
        store.apply(listOf(Event.DeviceCode(null)))
        assertNull(store.accounts.deviceCode.value)
    }

    @Test
    fun kitsu_s_password_goes_to_the_engine_and_a_pull_s_disputes_are_kept() {
        store.connectKitsu("nagare", "hunter2")
        assertEquals(listOf(Command.ConnectKitsu("nagare", "hunter2")), store.waiting)
        val row = ConflictRow(anilistId = 900001u, title = "Hoshikuzu", field = "progress", local = "7", remote = "6")
        store.apply(listOf(Event.Conflicts(listOf(row))))
        assertEquals(listOf(row), store.accounts.conflicts.value)
    }
}
