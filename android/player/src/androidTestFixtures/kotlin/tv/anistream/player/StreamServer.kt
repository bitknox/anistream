// A stream on the loopback that stops sending part-way: what a stalled source looks like to mpv.

package tv.anistream.player

import java.io.File
import java.io.RandomAccessFile
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * Serves [file] over HTTP on 127.0.0.1, with ranges, and holds once a response reaches [holdAfter]
 * bytes into the file until [release]. A request that starts in the file's last 256 KB, where an
 * MKV keeps its index, is served whole, so opening the file never waits on the hold.
 */
class StreamServer(private val file: File, private val holdAfter: Long) : AutoCloseable {
    private val socket = ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"))
    private val released = CountDownLatch(1)
    val port: Int get() = socket.localPort

    init {
        thread(name = "stream-server", isDaemon = true) {
            while (!socket.isClosed) {
                val client = runCatching { socket.accept() }.getOrNull() ?: break
                thread(name = "stream-client", isDaemon = true) { serve(client) }
            }
        }
    }

    fun release() = released.countDown()

    override fun close() {
        released.countDown()
        runCatching { socket.close() }
    }

    private fun serve(client: Socket): Unit = client.use {
        val input = client.getInputStream().bufferedReader(Charsets.ISO_8859_1)
        while (true) {
            val request = input.readLine() ?: return
            if (request.isEmpty()) continue
            var range: LongRange? = null
            while (true) {
                val header = input.readLine() ?: return
                if (header.isEmpty()) break
                if (header.startsWith("Range:", ignoreCase = true)) {
                    val spec = header.substringAfter("bytes=").trim()
                    val start = spec.substringBefore('-').toLongOrNull() ?: 0
                    val end = spec.substringAfter('-').toLongOrNull() ?: (file.length() - 1)
                    range = start..minOf(end, file.length() - 1)
                }
            }
            val length = file.length()
            val span = range ?: 0L..(length - 1)
            val out = client.getOutputStream()
            val head = buildString {
                append(if (range != null) "HTTP/1.1 206 Partial Content\r\n" else "HTTP/1.1 200 OK\r\n")
                append("Content-Type: video/x-matroska\r\n")
                append("Accept-Ranges: bytes\r\n")
                append("Content-Length: ${span.last - span.first + 1}\r\n")
                if (range != null) append("Content-Range: bytes ${span.first}-${span.last}/$length\r\n")
                append("\r\n")
            }
            out.write(head.toByteArray(Charsets.ISO_8859_1))
            if (request.startsWith("HEAD")) {
                out.flush()
                continue
            }
            val tail = span.first >= length - 256 * 1024
            RandomAccessFile(file, "r").use { data ->
                data.seek(span.first)
                val buffer = ByteArray(16 * 1024)
                var at = span.first
                while (at <= span.last) {
                    if (!tail && at >= holdAfter && released.count > 0) {
                        out.flush()
                        released.await(5, TimeUnit.MINUTES)
                    }
                    val n = data.read(buffer, 0, minOf(buffer.size.toLong(), span.last - at + 1).toInt())
                    if (n <= 0) break
                    runCatching { out.write(buffer, 0, n) }.onFailure { return }
                    at += n
                }
            }
            out.flush()
        }
    }
}
