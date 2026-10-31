package tv.anistream.store

import java.io.File
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import java.io.IOException
import java.nio.file.Files
import java.security.MessageDigest
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.NoticeKind
import tv.anistream.engine.model.Release
import tv.anistream.engine.model.ReleaseAsset
import tv.anistream.engine.model.UpdateStatus

/**
 * The update's steps: available, downloading, verifying, installing, then done or failed, and
 * cancelled from each back to available, on a host that stands in for Android.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class UpdateModelTest {
    private val context = RuntimeEnvironment.getApplication()
    private val dir: File = Files.createTempDirectory("updates").toFile()
    private val files = UpdateFiles(dir.resolve("updates"))
    private val apkBytes = "the new APK".toByteArray()
    private val offer = UpdateOffer("2.0.3", "https://example.invalid/anistream-2.0.3-android-arm64.apk", sha256(apkBytes), apkBytes.size.toLong(), "https://example.invalid/releases/v2.0.3")

    @After
    fun tearDown() {
        dir.deleteRecursively()
    }

    private fun sha256(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    /** Android, as the model sees it: what it was asked, and what it answers. */
    private inner class FakeHost(override val packageName: String = ApkIdentity.PACKAGE) : UpdateHost {
        var allowed = true
        var room = true
        var schedules = true
        var installed = ApkFacts(ApkIdentity.PACKAGE, 200000099, setOf(RELEASE_KEY))
        var apk: ApkFacts? = ApkFacts(ApkIdentity.PACKAGE, 200000399, setOf(RELEASE_KEY))
        var onInspect: () -> Unit = {}
        var installFails: IOException? = null
        val calls = mutableListOf<String>()
        var job: Job? = null
        var handed: ByteArray? = null

        override fun canInstall() = allowed
        override fun hasRoom(offer: UpdateOffer) = room
        override fun openInstallSettings(offer: UpdateOffer) {
            calls += "settings ${offer.version}"
        }
        override fun returnedFromSettings() {
            calls += "returned"
        }
        override fun startDownload(offer: UpdateOffer): Boolean {
            calls += "download ${offer.version}"
            return schedules
        }
        override fun stopDownload() {
            calls += "stop"
            job?.cancel()
        }
        override fun inspect(file: File): ApkFacts? {
            onInspect()
            return apk
        }
        override fun installed() = installed
        override fun install(file: File, offer: UpdateOffer) {
            installFails?.let { throw it }
            calls += "install ${offer.version}"
            handed = file.readBytes()
        }
        override fun abandonInstall() {
            calls += "abandon"
        }
    }

    /** Fetches [bytes], once [gate] opens, counting the fetches. */
    private inner class FakeFetch(var bytes: ByteArray = apkBytes) : ApkFetch {
        var gate: CompletableDeferred<Unit>? = null
        var fails: IOException? = null
        var fetches = 0

        override suspend fun fetch(url: String, into: File, expected: Long, progress: (Long, Long) -> Unit) {
            fetches++
            val part = File(into.path + ".part")
            part.writeBytes(bytes.copyOf(bytes.size / 2))
            progress(bytes.size / 2L, expected)
            gate?.await()
            fails?.let {
                part.delete()
                throw it
            }
            part.writeBytes(bytes)
            progress(bytes.size.toLong(), expected)
            part.renameTo(into)
        }
    }

    private val notices = NoticesModel()

    private fun TestScope.model(host: FakeHost): UpdateModel =
        UpdateModel(notices, io = StandardTestDispatcher(testScheduler)).also {
            it.attach(host, files)
            it.offer(offer)
        }

    /** Run the download job's work, as the job does once it was asked for. */
    private fun TestScope.job(model: UpdateModel, host: FakeHost, fetch: ApkFetch): Job =
        launch { model.run(fetch) }.also { host.job = it }

    private val left: List<String> get() = dir.resolve("updates").list()?.toList().orEmpty()

    @Test
    fun an_offer_is_available_and_nothing_starts_until_update_is_pressed() = runTest {
        val host = FakeHost()
        val model = model(host)
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertEquals(UpdateCheck.Found, model.check.value)
        assertTrue(host.calls.isEmpty())
        // No newer release: nothing to offer.
        model.offer(null)
        assertEquals(UpdateState.None, model.state.value)
        assertEquals(UpdateCheck.UpToDate, model.check.value)
    }

    @Test
    fun pressed_it_downloads_verifies_and_hands_the_file_to_the_installer() = runTest {
        val host = FakeHost()
        val model = model(host)
        val fetch = FakeFetch().apply { gate = CompletableDeferred() }
        model.press()
        assertEquals(listOf("download 2.0.3"), host.calls)
        assertEquals(UpdateState.Downloading(offer, 0, offer.bytes), model.state.value)
        job(model, host, fetch)
        advanceUntilIdle()
        val halfway = model.state.value as UpdateState.Downloading
        assertEquals(apkBytes.size / 2L, halfway.done)
        assertEquals(0.5f, halfway.fraction!!, 0.05f)
        fetch.gate!!.complete(Unit)
        advanceUntilIdle()
        assertEquals(UpdateState.Installing(offer), model.state.value)
        assertEquals("install 2.0.3", host.calls.last())
        assertTrue(apkBytes.contentEquals(host.handed))
        // Installed: done, and the file is gone.
        model.installResult(InstallResult.Installed)
        assertEquals(UpdateState.Done(offer), model.state.value)
        assertTrue(left.isEmpty())
    }

    @Test
    fun without_install_unknown_apps_it_asks_first_and_goes_on_once_back_with_it_on() = runTest {
        val host = FakeHost().apply { allowed = false }
        val model = model(host)
        model.press()
        assertEquals(UpdateState.Allowing(offer), model.state.value)
        // Coming back to the app before the page was opened changes nothing.
        model.resumed()
        assertEquals(UpdateState.Allowing(offer), model.state.value)
        model.openInstallSettings()
        assertEquals(UpdateState.Allowing(offer, opened = true), model.state.value)
        assertEquals(listOf("settings 2.0.3"), host.calls)
        host.allowed = true
        model.resumed()
        assertEquals(listOf("settings 2.0.3", "returned", "download 2.0.3"), host.calls)
        assertTrue(model.state.value is UpdateState.Downloading)
    }

    @Test
    fun back_with_the_switch_still_off_it_waits_for_another_press_and_not_now_drops_it() = runTest {
        val host = FakeHost().apply { allowed = false }
        val model = model(host)
        model.press()
        model.openInstallSettings()
        model.resumed()
        assertEquals(UpdateState.Available(offer), model.state.value)
        model.press()
        model.notNow()
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertFalse(host.calls.any { it.startsWith("download") })
    }

    @Test
    fun started_again_by_android_while_its_page_was_open_it_goes_on_when_back_on_screen() = runTest {
        // Where Android stops the app as the switch turns on, the new process is told what was pressed.
        val host = FakeHost()
        val model = UpdateModel(notices, io = StandardTestDispatcher(testScheduler)).apply { attach(host, files) }
        model.returning(offer)
        assertEquals(UpdateState.Allowing(offer, opened = true), model.state.value)
        model.resumed()
        assertEquals(listOf("returned", "download 2.0.3"), host.calls)
    }

    @Test
    fun a_wrong_checksum_is_refused_with_a_notice_and_never_reaches_the_installer() = runTest {
        val host = FakeHost()
        val model = model(host)
        model.press()
        job(model, host, FakeFetch("some other file".toByteArray()))
        advanceUntilIdle()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Checksum), model.state.value)
        assertFalse(host.calls.any { it.startsWith("install") })
        assertTrue(left.isEmpty())
        val notice = notices.current.value!!
        assertEquals(NoticeKind.ALERT, notice.notice.kind)
        assertEquals("The download does not match the checksum listed for this version, so it was not installed.", notice.text(context))
    }

    @Test
    fun a_file_android_cannot_read_fails_the_update_instead_of_crashing_on_verifying() = runTest {
        val host = FakeHost()
        host.onInspect = { throw IllegalStateException("the package manager could not parse it") }
        val model = model(host)
        model.press()
        job(model, host, FakeFetch())
        advanceUntilIdle()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.NotAnApp), model.state.value)
        assertFalse(host.calls.any { it.startsWith("install") })
    }

    @Test
    fun another_app_or_another_key_is_refused_before_the_installer() = runTest {
        for ((apk, failure) in listOf(
            ApkFacts("tv.anistream.fixtures", 200000399, setOf(RELEASE_KEY)) to UpdateFailure.Package("tv.anistream.fixtures"),
            ApkFacts(ApkIdentity.PACKAGE, 200000399, setOf(OTHER_KEY)) to UpdateFailure.Signer,
            ApkFacts(ApkIdentity.PACKAGE, 200000099, setOf(RELEASE_KEY)) to UpdateFailure.NotNewer,
            null to UpdateFailure.NotAnApp,
        )) {
            val host = FakeHost().apply { this.apk = apk }
            val model = model(host)
            model.press()
            job(model, host, FakeFetch())
            advanceUntilIdle()
            assertEquals(UpdateState.Failed(offer, failure), model.state.value)
            assertFalse(host.calls.any { it.startsWith("install") })
            assertTrue(left.isEmpty())
        }
    }

    @Test
    fun a_download_that_fails_says_so_and_pressing_again_starts_over() = runTest {
        val host = FakeHost()
        val model = model(host)
        val fetch = FakeFetch().apply { fails = IOException("the server answered 503") }
        model.press()
        job(model, host, fetch)
        advanceUntilIdle()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Download("the server answered 503")), model.state.value)
        assertEquals("Couldn't download the update. Check the connection and try again.", notices.current.value!!.text(context))
        assertTrue(left.isEmpty())
        fetch.fails = null
        model.press()
        job(model, host, fetch)
        advanceUntilIdle()
        assertEquals(UpdateState.Installing(offer), model.state.value)
    }

    @Test
    fun the_installer_failing_says_so_in_our_words_and_deletes_the_file() = runTest {
        val host = FakeHost()
        val model = model(host)
        model.press()
        job(model, host, FakeFetch())
        advanceUntilIdle()
        assertTrue(left.isNotEmpty())
        model.installResult(InstallResult.Failed("INSTALL_FAILED_INSUFFICIENT_STORAGE"))
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Install("INSTALL_FAILED_INSUFFICIENT_STORAGE")), model.state.value)
        assertEquals("Couldn't install the update. Try again.", notices.current.value!!.text(context))
        assertTrue(left.isEmpty())
        // A session that could not even be written is the same failure.
        val writing = FakeHost().apply { installFails = IOException("no space") }
        val other = model(writing)
        other.press()
        job(other, writing, FakeFetch())
        advanceUntilIdle()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Install("no space")), other.state.value)
    }

    @Test
    fun the_viewer_saying_no_keeps_the_file_so_update_again_goes_straight_to_the_installer() = runTest {
        val host = FakeHost()
        val model = model(host)
        val fetch = FakeFetch()
        model.press()
        job(model, host, fetch)
        advanceUntilIdle()
        model.installResult(InstallResult.Cancelled)
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertNull(notices.current.value)
        model.press()
        job(model, host, fetch)
        advanceUntilIdle()
        assertEquals(1, fetch.fetches)
        assertEquals(2, host.calls.count { it == "install 2.0.3" })
        assertEquals(UpdateState.Installing(offer), model.state.value)
    }

    @Test
    fun cancelled_while_downloading_the_job_stops_and_its_file_goes() = runTest {
        val host = FakeHost()
        val model = model(host)
        val fetch = FakeFetch().apply { gate = CompletableDeferred() }
        model.press()
        job(model, host, fetch)
        advanceUntilIdle()
        assertTrue(left.isNotEmpty())
        model.cancel()
        advanceUntilIdle()
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertTrue("stop" in host.calls)
        assertTrue(left.isEmpty())
        assertFalse(host.calls.any { it.startsWith("install") })
        assertNull(notices.current.value)
    }

    @Test
    fun cancelled_while_verifying_nothing_reaches_the_installer() = runTest {
        val host = FakeHost()
        val model = model(host)
        host.onInspect = { model.cancel() }
        model.press()
        job(model, host, FakeFetch())
        advanceUntilIdle()
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertFalse(host.calls.any { it.startsWith("install") })
        assertTrue(left.isEmpty())
    }

    @Test
    fun cancelled_while_installing_the_session_goes_and_a_late_answer_changes_nothing() = runTest {
        val host = FakeHost()
        val model = model(host)
        model.press()
        job(model, host, FakeFetch())
        advanceUntilIdle()
        model.cancel()
        assertEquals("abandon", host.calls.last())
        assertEquals(UpdateState.Available(offer), model.state.value)
        assertTrue(left.isEmpty())
        model.installResult(InstallResult.Installed)
        assertEquals(UpdateState.Available(offer), model.state.value)
    }

    @Test
    fun cancelled_while_asking_for_install_unknown_apps_it_is_available_again() = runTest {
        val host = FakeHost().apply { allowed = false }
        val model = model(host)
        model.press()
        model.cancel()
        assertEquals(UpdateState.Available(offer), model.state.value)
    }

    @Test
    fun a_newer_offer_waits_for_the_step_under_way_and_is_taken_after_it() = runTest {
        val host = FakeHost()
        val model = model(host)
        model.press()
        val newer = offer.copy(version = "2.0.4")
        model.offer(newer)
        assertEquals(offer, model.state.value.offer)
        model.cancel()
        assertEquals(UpdateState.Available(newer), model.state.value)
    }

    @Test
    fun android_not_starting_or_stopping_the_job_fails_it() = runTest {
        val refused = FakeHost().apply { schedules = false }
        val model = model(refused)
        model.press()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.NotStarted), model.state.value)
        val host = FakeHost()
        val stopped = model(host)
        stopped.press()
        stopped.stopped()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Download(null)), stopped.state.value)
    }

    @Test
    fun a_release_without_an_apk_is_offered_but_never_downloaded() = runTest {
        val host = FakeHost()
        val model = model(host)
        val page = offer.copy(url = "", sha256 = "", bytes = 0)
        model.offer(page)
        assertFalse(model.installs(page))
        model.press()
        assertEquals(UpdateState.Available(page), model.state.value)
        assertTrue(host.calls.isEmpty())
        // Nor where the build does not update itself, as the fixtures flavour does not.
        assertFalse(UpdateModel().installs(offer))
    }

    @Test
    fun the_notice_is_said_only_when_the_engine_says_to_and_once() = runTest {
        val model = model(FakeHost())
        assertNull(model.notice.value)
        model.offer(offer, tell = true)
        assertEquals(offer, model.notice.value)
        model.told(offer)
        assertNull(model.notice.value)
        // Asked again with Check now, the engine does not say to tell.
        model.offer(offer, tell = false)
        assertNull(model.notice.value)
    }

    @Test
    fun check_now_says_it_is_checking_then_what_it_found_or_why_not() = runTest {
        val model = model(FakeHost())
        model.checking()
        assertEquals(UpdateCheck.Checking, model.check.value)
        model.checkFailed("GitHub did not answer")
        assertEquals(UpdateCheck.Failed("GitHub did not answer"), model.check.value)
        // What was offered stands.
        assertEquals(UpdateState.Available(offer), model.state.value)
    }

    private val release = Release(
        version = "2.0.3",
        publishedAt = 1_792_664_639,
        pageUrl = "https://github.com/bitknox/anistream/releases/tag/v2.0.3",
        notes = "Smoother calendar",
        android = ReleaseAsset(
            name = "anistream-2.0.3-android-arm64.apk",
            url = "https://github.com/bitknox/anistream/releases/download/v2.0.3/anistream-2.0.3-android-arm64.apk",
            sha256 = "0f".repeat(32),
            size = 61_400_000uL,
        ),
    )

    @Test
    fun the_engine_s_release_is_offered_with_its_apk_and_told_only_when_it_says() = runTest {
        val store = AppStore(this)
        store.apply(Event.Update(UpdateStatus.Available(release, notify = false)))
        val offered = UpdateOffer(
            version = "2.0.3",
            url = "https://github.com/bitknox/anistream/releases/download/v2.0.3/anistream-2.0.3-android-arm64.apk",
            sha256 = "0f".repeat(32),
            bytes = 61_400_000,
            page = "https://github.com/bitknox/anistream/releases/tag/v2.0.3",
            notes = "Smoother calendar",
        )
        assertEquals(UpdateState.Available(offered), store.updates.state.value)
        assertEquals("Smoother calendar", offered.headline)
        assertEquals(null, offered.copy(notes = "\n  \n").headline)
        assertTrue(offered.installable)
        assertNull(store.updates.notice.value)
        store.apply(Event.Update(UpdateStatus.Available(release, notify = true)))
        assertEquals(offered, store.updates.notice.value)
        // Up to date, nothing is offered any more.
        store.apply(Event.Update(UpdateStatus.UpToDate()))
        assertEquals(UpdateState.None, store.updates.state.value)
        assertNull(store.updates.notice.value)
    }

    @Test
    fun the_tv_is_offered_the_tv_s_apk_and_no_other() = runTest {
        val tv = ReleaseAsset(
            name = "anistream-2.0.3-android-tv.apk",
            url = "https://github.com/bitknox/anistream/releases/download/v2.0.3/anistream-2.0.3-android-tv.apk",
            sha256 = "1e".repeat(32),
            size = 48_000_000uL,
        )
        val store = AppStore(this, updateAsset = { it.androidTv })
        store.apply(Event.Update(UpdateStatus.Available(release.copy(androidTv = tv), notify = false)))
        val offered = store.updates.state.value.offer!!
        assertEquals(tv.url, offered.url)
        assertEquals("1e".repeat(32), offered.sha256)
        assertEquals(48_000_000L, offered.bytes)
        assertTrue(offered.installable)
        // A release with the phone's APK alone has nothing the TV installs: its page only.
        store.apply(Event.Update(UpdateStatus.Available(release, notify = false)))
        assertFalse(store.updates.state.value.offer!!.installable)
    }

    @Test
    fun the_tv_s_apk_must_be_the_tv_s_package() = runTest {
        val host = FakeHost(packageName = TV_PACKAGE).apply {
            installed = ApkFacts(TV_PACKAGE, 200000099, setOf(RELEASE_KEY))
            apk = ApkFacts(ApkIdentity.PACKAGE, 200000399, setOf(RELEASE_KEY))
        }
        val apk = dir.resolve("phone.apk").apply { writeBytes(apkBytes) }
        assertEquals(UpdateFailure.Package(ApkIdentity.PACKAGE), UpdateModel.verify(apk, offer, host))
        host.apk = ApkFacts(TV_PACKAGE, 200000399, setOf(RELEASE_KEY))
        assertNull(UpdateModel.verify(apk, offer, host))
    }

    @Test
    fun too_little_room_fails_before_anything_downloads() = runTest {
        val host = FakeHost().apply { room = false }
        val model = model(host)
        model.press()
        assertEquals(UpdateState.Failed(offer, UpdateFailure.Storage), model.state.value)
        assertTrue("nothing was asked of the download", host.calls.isEmpty())
        assertEquals(UpdateFailure.Storage.text(context), notices.current.value!!.text(context))
        // With room again, Update starts over.
        host.room = true
        model.press()
        assertEquals(listOf("download 2.0.3"), host.calls)
    }

    @Test
    fun a_release_without_an_apk_offers_its_page() = runTest {
        val store = AppStore(this)
        store.apply(Event.Update(UpdateStatus.Available(release.copy(android = null), notify = true)))
        val offer = store.updates.state.value.offer!!
        assertFalse(offer.installable)
        assertEquals("https://github.com/bitknox/anistream/releases/tag/v2.0.3", offer.page)
    }

    @Test
    fun check_now_s_answers_say_where_it_stands() = runTest {
        val store = AppStore(this)
        store.apply(Event.Update(UpdateStatus.Checking))
        assertEquals(UpdateCheck.Checking, store.updates.check.value)
        store.apply(Event.Update(UpdateStatus.Failed("GitHub could not be reached")))
        assertEquals(UpdateCheck.Failed("GitHub could not be reached"), store.updates.check.value)
        store.apply(Event.Update(UpdateStatus.UpToDate()))
        assertEquals(UpdateCheck.UpToDate, store.updates.check.value)
        // Check now goes to the engine whatever the switch says.
        store.checkForUpdates()
        assertEquals(listOf(Command.CheckForUpdates), store.waiting)
    }

    companion object {
        const val TV_PACKAGE = "tv.anistream.tv"
        /** The release key's fingerprint, as assetlinks.json has it. */
        const val RELEASE_KEY = "4D:66:4A:AB:A3:15:8B:C1:C0:27:81:34:3A:2C:E2:F2:9E:E2:8E:9A:64:10:C9:4F:F1:89:A2:5D:2D:4C:9F:95"
        const val OTHER_KEY = "0E:2B:8C:11:5A:90:3D:44:7F:C2:19:A6:E8:3B:72:0D:91:5C:4E:A8:16:F3:B0:27:6D:89:C4:5E:1A:7B:33:F0"
    }
}
