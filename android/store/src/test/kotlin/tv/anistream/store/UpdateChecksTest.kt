package tv.anistream.store

import com.sun.net.httpserver.HttpServer
import java.io.File
import java.io.IOException
import java.net.InetSocketAddress
import java.nio.file.Files
import java.util.concurrent.Executors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import tv.anistream.store.UpdateModelTest.Companion.OTHER_KEY
import tv.anistream.store.UpdateModelTest.Companion.RELEASE_KEY

/** The checksum, the package and key check, the folder the APK is kept in, and the fetch. */
class UpdateChecksTest {
    private val dir: File = Files.createTempDirectory("update-checks").toFile()

    @After
    fun tearDown() {
        dir.deleteRecursively()
    }

    // The checksum

    @Test
    fun the_checksum_is_the_files_sha256_read_from_the_release_in_any_spelling() {
        val file = dir.resolve("abc.apk").apply { writeText("abc") }
        val abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        assertEquals(abc, ApkChecksum.of(file))
        assertTrue(ApkChecksum.matches(file, abc))
        assertTrue(ApkChecksum.matches(file, abc.uppercase()))
        // As sha256sum writes a .sha256 file, with the name after it and a newline.
        assertTrue(ApkChecksum.matches(file, "$abc  anistream-2.0.3-android-arm64.apk\n"))
    }

    @Test
    fun a_different_file_or_a_digest_that_does_not_read_is_never_a_match() {
        val file = dir.resolve("abd.apk").apply { writeText("abd") }
        assertFalse(ApkChecksum.matches(file, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"))
        assertFalse(ApkChecksum.matches(file, ""))
        assertFalse(ApkChecksum.matches(file, "not a digest"))
        assertFalse(ApkChecksum.matches(file, ApkChecksum.of(file).dropLast(1)))
        assertNull(ApkChecksum.expected("zz".repeat(32)))
    }

    // The package and the key

    private val installed = ApkFacts(ApkIdentity.PACKAGE, 200000099, setOf(RELEASE_KEY))
    private val newer = ApkFacts(ApkIdentity.PACKAGE, 200000399, setOf(RELEASE_KEY))

    @Test
    fun the_same_app_signed_with_the_same_key_and_newer_may_replace_it() {
        assertNull(ApkIdentity.check(newer, installed))
        // The digest in another spelling is the same key.
        assertNull(ApkIdentity.check(newer.copy(signers = setOf(RELEASE_KEY.replace(":", "").lowercase())), installed))
    }

    @Test
    fun another_package_another_key_an_older_one_or_no_apk_may_not() {
        assertEquals(UpdateFailure.Package("tv.anistream.fixtures"), ApkIdentity.check(newer.copy(packageName = "tv.anistream.fixtures"), installed))
        assertEquals(UpdateFailure.Package("com.example.other"), ApkIdentity.check(newer.copy(packageName = "com.example.other"), installed))
        assertEquals(UpdateFailure.Signer, ApkIdentity.check(newer.copy(signers = setOf(OTHER_KEY)), installed))
        // Signed by the key and another, it is not the app's signature.
        assertEquals(UpdateFailure.Signer, ApkIdentity.check(newer.copy(signers = setOf(RELEASE_KEY, OTHER_KEY)), installed))
        assertEquals(UpdateFailure.NotNewer, ApkIdentity.check(newer.copy(versionCode = 200000099), installed))
        assertEquals(UpdateFailure.NotNewer, ApkIdentity.check(newer.copy(versionCode = 100), installed))
        assertEquals(UpdateFailure.NotAnApp, ApkIdentity.check(null, installed))
        // The fixtures flavour never updates itself, whatever it is offered.
        assertEquals(UpdateFailure.Package(ApkIdentity.PACKAGE), ApkIdentity.check(newer, installed.copy(packageName = "tv.anistream.fixtures")))
        // An installed app whose key Android did not say matches nothing.
        assertEquals(UpdateFailure.Signer, ApkIdentity.check(newer.copy(signers = emptySet()), installed.copy(signers = emptySet())))
    }

    @Test
    fun a_key_rotated_from_the_installed_one_is_taken_as_android_takes_it() {
        val rotated = newer.copy(signers = setOf(OTHER_KEY), lineage = setOf(RELEASE_KEY, OTHER_KEY))
        assertNull(ApkIdentity.check(rotated, installed))
        assertEquals(UpdateFailure.Signer, ApkIdentity.check(rotated.copy(lineage = setOf(OTHER_KEY)), installed))
    }

    @Test
    fun a_certificates_fingerprint_is_spelt_as_assetlinks_spells_it() {
        assertEquals("BA:78:16:BF", ApkIdentity.fingerprint("ba7816bf"))
        assertEquals(
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD",
            ApkIdentity.fingerprint("abc".toByteArray()),
        )
    }

    // The folder

    @Test
    fun the_folder_keeps_one_apk_and_is_emptied_whole() {
        val files = UpdateFiles(dir.resolve("updates"))
        val offer = UpdateOffer("2.0.3", "https://example.invalid/a.apk", "", 0)
        val apk = files.apk(offer)
        assertEquals("anistream-2.0.3.apk", apk.name)
        // A version is never a path.
        assertEquals("anistream-..evil.apk", files.apk(offer.copy(version = "../evil")).name)
        files.keepOnly(apk)
        val stale = files.dir.resolve("anistream-2.0.2.apk").apply { writeText("old") }
        files.dir.resolve("anistream-2.0.3.apk.part").writeText("half")
        apk.writeText("new")
        files.keepOnly(apk)
        assertEquals(listOf(apk.name), files.dir.list()!!.toList())
        assertFalse(stale.exists())
        files.clear()
        assertTrue(files.dir.list()!!.isEmpty())
        // Clearing a folder that is not there is nothing.
        UpdateFiles(dir.resolve("never")).clear()
    }

    // The fetch

    private val body = ByteArray(300_000) { (it % 251).toByte() }

    private fun serve(block: (base: String) -> Unit) {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/apk") { exchange ->
            exchange.sendResponseHeaders(200, body.size.toLong())
            exchange.responseBody.use { it.write(body) }
        }
        server.createContext("/redirect") { exchange ->
            exchange.responseHeaders.add("Location", "/apk")
            exchange.sendResponseHeaders(302, -1)
            exchange.close()
        }
        server.createContext("/missing") { exchange ->
            exchange.sendResponseHeaders(404, -1)
            exchange.close()
        }
        server.createContext("/short") { exchange ->
            // Says the whole size, then stops halfway.
            exchange.sendResponseHeaders(200, body.size.toLong())
            exchange.responseBody.write(body, 0, body.size / 2)
            exchange.close()
        }
        server.createContext("/slow") { exchange ->
            exchange.sendResponseHeaders(200, body.size.toLong())
            exchange.responseBody.write(body, 0, 1_000)
            exchange.responseBody.flush()
            runCatching { Thread.sleep(30_000) }
            exchange.close()
        }
        // Each exchange on a thread of its own, so the slow one holds up nothing else.
        val threads = Executors.newCachedThreadPool()
        server.executor = threads
        server.start()
        try {
            block("http://127.0.0.1:${server.address.port}")
        } finally {
            threads.shutdownNow()
            server.stop(0)
        }
    }

    private val fetch = HttpApkFetch(plainHttp = true)

    @Test
    fun it_fetches_the_whole_file_following_redirects_and_saying_how_far() = serve { base ->
        val into = dir.resolve("updates/anistream-2.0.3.apk")
        val seen = mutableListOf<Pair<Long, Long>>()
        runBlocking { fetch.fetch("$base/redirect", into, body.size.toLong()) { done, total -> seen += done to total } }
        assertTrue(body.contentEquals(into.readBytes()))
        assertEquals(0L to body.size.toLong(), seen.first())
        assertEquals(body.size.toLong() to body.size.toLong(), seen.last())
        assertFalse(File(into.path + ".part").exists())
        // Where the release does not say the size, the server's is the total.
        val unsized = dir.resolve("updates/unsized.apk")
        var total = 0L
        runBlocking { fetch.fetch("$base/apk", unsized, 0) { _, t -> total = t } }
        assertEquals(body.size.toLong(), total)
    }

    private fun refused(url: String, expected: Long = body.size.toLong(), fetch: ApkFetch = this.fetch): IOException {
        val into = dir.resolve("updates/refused.apk")
        try {
            runBlocking { fetch.fetch(url, into, expected) { _, _ -> } }
        } catch (e: IOException) {
            assertFalse("nothing is kept of a fetch that failed", into.exists() || File(into.path + ".part").exists())
            return e
        }
        fail("$url was fetched")
        throw AssertionError()
    }

    @Test
    fun anything_short_of_the_release_s_whole_file_is_refused() = serve { base ->
        assertEquals("the server answered 404", refused("$base/missing").message)
        refused("$base/short")
        // The server's size is not the release's.
        assertEquals("the server's file is 300000 bytes, the release's 1000", refused("$base/apk", expected = 1_000).message)
        // Larger than any APK could be.
        refused("$base/apk", expected = 0, fetch = HttpApkFetch(plainHttp = true, limit = 1_000))
        // Plain HTTP only for the update test's builds; and nothing that is not a link.
        assertEquals("the update is fetched over HTTPS only", refused("$base/apk", fetch = HttpApkFetch()).message)
        refused("not a link")
    }

    @Test
    fun cancelled_it_stops_and_keeps_nothing() = serve { base ->
        // The JDK's connection does not close under a read, as Android's does: the read timeout ends it.
        val fetch = HttpApkFetch(plainHttp = true, timeoutMillis = 1_000)
        val into = dir.resolve("updates/slow.apk")
        runBlocking {
            var started = false
            val download = async(Dispatchers.IO) { fetch.fetch("$base/slow", into, body.size.toLong()) { done, _ -> if (done > 0) started = true } }
            withTimeout(10_000) { while (!started) kotlinx.coroutines.delay(10) }
            download.cancel()
            // The server holds the rest back for 30 s. The read timeout ends the read after 1 s;
            // a busy runner can take a few more, so the wait stops well short of the server's.
            assertEquals(true, withTimeoutOrNull(20_000) { download.join(); true })
            assertTrue(download.isCancelled)
        }
        assertFalse(into.exists())
        assertFalse(File(into.path + ".part").exists())
    }
}
