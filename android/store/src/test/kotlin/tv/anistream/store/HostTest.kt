package tv.anistream.store

import java.io.File
import java.nio.file.Files
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The host as the engine calls it, on the in-memory backend, and the sealed file on a plain key. */
class HostTest {
    private val dir: File = Files.createTempDirectory("anistream-secrets").toFile()

    @After
    fun tearDown() {
        dir.deleteRecursively()
    }

    private fun key(): SecretKey = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()

    @Test
    fun the_host_keeps_secrets_where_its_backend_does() {
        val secrets = MemorySecrets()
        val opened = mutableListOf<String>()
        val posted = mutableListOf<Pair<String, String>>()
        val host = AppHost(secrets, open = { opened += it; true }, notify = { title, body -> posted += title to body })

        assertNull(host.secretGet("anilist"))
        assertTrue(host.secretSet("anilist", "a-token"))
        assertEquals("a-token", host.secretGet("anilist"))
        assertEquals(mapOf("anilist" to "a-token"), secrets.all)
        assertTrue(host.secretDelete("anilist"))
        // Signing out twice is no failure.
        assertTrue(host.secretDelete("anilist"))
        assertNull(host.secretGet("anilist"))

        assertTrue(host.openUrl("https://anilist.co/"))
        assertEquals(listOf("https://anilist.co/"), opened)
        host.notify("Episode 8 is out", "Summer Wind Relay")
        assertEquals(listOf("Episode 8 is out" to "Summer Wind Relay"), posted)
    }

    @Test
    fun a_host_with_no_way_to_open_links_says_so() {
        assertFalse(AppHost(MemorySecrets()).openUrl("https://anilist.co/"))
    }

    @Test
    fun sealed_secrets_outlive_the_backend_that_kept_them() {
        val key = key()
        val file = dir.resolve("secrets")
        SealedFileSecrets(file) { key }.apply {
            assertTrue(setSecret("a-token", "anilist"))
            assertTrue(setSecret("store-token", "store:9f3c1a2b"))
        }
        // The value itself is never in the file.
        assertFalse(file.readText().contains("a-token"))
        val again = SealedFileSecrets(file) { key }
        assertEquals("a-token", again.secret("anilist"))
        assertEquals("store-token", again.secret("store:9f3c1a2b"))
        assertTrue(again.deleteSecret("anilist"))
        assertNull(SealedFileSecrets(file) { key }.secret("anilist"))
        assertEquals("store-token", SealedFileSecrets(file) { key }.secret("store:9f3c1a2b"))
    }

    @Test
    fun a_secret_reads_as_none_under_another_key_or_another_id() {
        val file = dir.resolve("secrets")
        SealedFileSecrets(file) { key() }.setSecret("a-token", "anilist")
        // The app's data restored where its key is not.
        assertNull(SealedFileSecrets(file) { key() }.secret("anilist"))

        // A value moved to another id does not open there.
        val key = key()
        SealedFileSecrets(file) { key }.setSecret("a-token", "anilist")
        val lines = file.readLines()
        val moved = lines.map { line ->
            if (line.startsWith(java.util.Base64.getEncoder().encodeToString("anilist".toByteArray()))) {
                java.util.Base64.getEncoder().encodeToString("mal".toByteArray()) + " " + line.substringAfter(' ')
            } else line
        }
        file.writeText(moved.joinToString("\n", postfix = "\n"))
        assertNull(SealedFileSecrets(file) { key }.secret("mal"))
    }

    @Test
    fun a_missing_or_foreign_file_is_no_secrets() {
        val file = dir.resolve("secrets")
        assertNull(SealedFileSecrets(file) { key() }.secret("anilist"))
        file.writeText("something else\n")
        assertNull(SealedFileSecrets(file) { key() }.secret("anilist"))
    }
}
