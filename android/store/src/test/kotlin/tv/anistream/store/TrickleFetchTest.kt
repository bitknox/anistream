package tv.anistream.store

import java.io.File
import java.io.IOException
import java.net.ServerSocket
import java.nio.file.Files
import kotlin.concurrent.thread
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The update's download ends at its deadline, whatever pace the server keeps. */
class TrickleFetchTest {
    @Test
    fun a_trickling_server_does_not_hold_the_download_past_its_deadline() {
        val server = ServerSocket(0)
        val size = 12
        thread(isDaemon = true) {
            runCatching {
                server.accept().use { client ->
                    client.getInputStream().bufferedReader().let { r -> while (r.readLine().orEmpty().isNotEmpty()) {} }
                    val out = client.getOutputStream()
                    out.write("HTTP/1.1 200 OK\r\nContent-Length: $size\r\n\r\n".toByteArray())
                    out.flush()
                    // Each byte inside the 1 s wait, so only the deadline ends it.
                    repeat(size) {
                        out.write('x'.code)
                        out.flush()
                        Thread.sleep(900)
                    }
                }
            }
        }
        val dir = Files.createTempDirectory("trickle").toFile()
        val into = File(dir, "app.apk")
        val started = System.currentTimeMillis()
        val outcome = runCatching {
            runBlocking {
                HttpApkFetch(plainHttp = true, timeoutMillis = 1_000, deadlineMillis = 2_000)
                    .fetch("http://127.0.0.1:${server.localPort}/app.apk", into, size.toLong()) { _, _ -> }
            }
        }
        val took = System.currentTimeMillis() - started
        server.close()
        val left = dir.listFiles().orEmpty().map { it.name }
        dir.deleteRecursively()
        // Whole, the 12 bytes take about 11 s. The deadline ends it after 2 s and one more wait.
        val failure = outcome.exceptionOrNull()
        assertTrue("took $took ms, ${failure ?: "finished"}", took < 5_000 && failure is IOException)
        assertEquals("the download took longer than 2 s", failure?.message)
        assertEquals("nothing kept", emptyList<String>(), left)
    }
}
