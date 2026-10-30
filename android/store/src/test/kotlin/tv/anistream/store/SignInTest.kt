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
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind

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
        url = "https://anilist.co/api/v2/oauth/authorize?client_id=54537&response_type=token&state=Zq7%2Bk",
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
    fun only_the_open_sign_in_s_own_redirect_ends_it_and_every_one_goes_to_the_engine() {
        store.apply(listOf(authorize))
        val request = store.accounts.signIn.value!!
        store.accounts.tabOpened(request)
        // For another tracker: the engine says so, and AniList's waits on.
        val other = "anistream://auth/mal?code=c0de&state=Zq7%2Bk"
        assertEquals(false, store.redirected("mal", other))
        // With another request's state, or none: the engine goes on waiting for its own, and so does
        // the app, tab and all, for the tab's own redirect still to come.
        val stale = "anistream://auth/anilist#access_token=a.b.c&state=0ld"
        val bare = "anistream://auth/anilist#access_token=a.b.c"
        assertEquals(false, store.redirected("anilist", stale))
        assertEquals(false, store.redirected("anilist", bare))
        assertEquals(request, store.accounts.signIn.value)
        assertEquals(request, store.accounts.tab)
        assertNull("the tab is not a second time", store.accounts.toOpen(request))

        val own = "anistream://auth/anilist#access_token=a.b.c&token_type=Bearer&state=Zq7%2Bk"
        assertEquals(true, store.redirected("anilist", own))
        assertNull(store.accounts.signIn.value)
        assertNull(store.accounts.tab)
        assertEquals(
            listOf(other, stale, bare, own).map { Command.FinishSignIn(if (it == other) "mal" else "anilist", it) },
            store.waiting,
        )
    }

    @Test
    fun a_tab_open_for_a_sign_in_is_not_opened_again() {
        store.apply(listOf(authorize))
        val request = store.accounts.signIn.value!!
        assertEquals(request, store.accounts.toOpen(request))
        store.accounts.tabOpened(request)
        // An activity made again while the tab is open asks the store, which has it.
        assertNull(store.accounts.toOpen(store.accounts.signIn.value))
        store.cancelSignIn(request)
        assertNull(store.accounts.tab)
        assertEquals(false, store.accounts.leftForTab)
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

    private fun kitsuFailed(reason: String) = Event.Toast(
        Notice(NoticeKind.ALERT, "Couldn't sign in to Kitsu. Try again. $reason", null, msg = Msg.NoticeSignInFailed("Kitsu", reason)),
    )

    /** The sheet is over the snackbar: a failed sign-in from it is said in it, and kept in Messages. */
    @Test
    fun a_failed_sign_in_from_the_open_sheet_is_the_sheet_s_to_say() {
        val refused = "token exchange failed: 400 Bad Request: The provided authorization grant is invalid (invalid_grant)"
        store.accounts.credentialsSheet("kitsu")
        store.connectKitsu("wayfarer", "not it")
        store.apply(listOf(kitsuFailed(refused)))
        assertEquals(refused, store.accounts.signInFailure.value)
        assertNull("not under the sheet", store.notices.current.value)
        assertEquals(refused, (store.notices.log.value.first().notice.msg as Msg.NoticeSignInFailed).reason)

        // The next try clears it, and with the sheet closed a failure is a notice again.
        store.connectKitsu("wayfarer", "stardust")
        assertNull(store.accounts.signInFailure.value)
        store.accounts.credentialsSheet(null)
        store.apply(listOf(kitsuFailed(refused)))
        assertNull(store.accounts.signInFailure.value)
        assertEquals(refused, (store.notices.current.value?.notice?.msg as Msg.NoticeSignInFailed).reason)
    }
}

/** Which AniList client the app flavour signs in with: the Android one, or the shared one without it. */
class AniListClientTest {
    @Test
    fun the_app_signs_in_with_the_android_client_on_the_verified_app_link() {
        assertEquals("54545", EngineLaunch.ANDROID_ANILIST_CLIENT_ID)
        val launch = EngineLaunch.app(EngineHome.sandbox(java.io.File("files"), java.io.File("cache")))
        assertEquals("54545", launch.record.anilistClientId)
        assertEquals("https://anistream.tv/auth/anilist", launch.record.anilistRedirectUri)
    }

    @Test
    fun without_the_android_client_the_shared_one_comes_back_on_the_scheme() {
        assertEquals(AniListClient("54537", null), EngineLaunch.aniListClient(null))
        assertEquals(AniListClient("54537", null), EngineLaunch.aniListClient(" "))
        val launch = EngineLaunch.app(EngineHome.sandbox(java.io.File("files"), java.io.File("cache")), androidAniListClientId = null)
        assertEquals("54537", launch.record.anilistClientId)
        assertNull(launch.record.anilistRedirectUri)
    }

    @Test
    fun any_android_client_comes_back_on_the_verified_app_link() {
        assertEquals(AniListClient("61234", "https://anistream.tv/auth/anilist"), EngineLaunch.aniListClient("61234"))
        val launch = EngineLaunch.app(EngineHome.sandbox(java.io.File("files"), java.io.File("cache")), androidAniListClientId = "61234")
        assertEquals("61234", launch.record.anilistClientId)
        assertEquals("https://anistream.tv/auth/anilist", launch.record.anilistRedirectUri)
        // MyAnimeList keeps the scheme, and Simkl and Trakt their device codes.
        assertNull(launch.record.malRedirectUri)
        assertEquals(EngineLaunch.TRAKT_CLIENT_ID, launch.record.traktClientId)
        assertNull(launch.record.traktClientSecret)
    }
}
