// The TV's sign-in page: while TVSignIn is open, a one-page form on the local network at a
// one-time path, where a phone pastes AniList's code or types Kitsu's user name and password. It
// stops when the screen closes or after ten minutes.

package tv.anistream.tv.signin

import android.util.Log
import java.io.BufferedInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.BindException
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketException
import java.net.URLDecoder
import java.security.SecureRandom
import kotlin.concurrent.thread
import tv.anistream.store.TorrentWords

/** What the page sends the TV. */
sealed interface PageAnswer {
    /** AniList: the code from its PIN page, or the whole address its sign-in ended on. */
    data class Code(val text: String) : PageAnswer

    /** Kitsu: the user name or email, and the password. */
    data class Password(val username: String, val password: String) : PageAnswer

    /** A plugin store's invite link, for the TV to preview before joining. */
    data class Link(val text: String) : PageAnswer

    /**
     * Torrents' details: the index, and the SOCKS5 proxy's address when the form asked for it,
     * with its user name and password when the proxy wants them.
     */
    data class Torrent(val index: String, val host: String = "", val port: String = "", val user: String = "", val password: String = "") : PageAnswer {
        override fun toString(): String = "Torrent(index=$index, host=$host, port=$port)"
    }
}

/**
 * Which form the page shows: a code, a user name and password, a plugin store's link, or the
 * torrent index, alone or with a SOCKS5 proxy's address.
 */
enum class PageForm { Code, Password, Link, Torrent, TorrentProxy, Proxy }

/**
 * The page for [tracker] ("AniList"), its form [form], with a link to [authorizeUrl] to open
 * first when there is one. Each answer goes to [onAnswer], on the page's own thread.
 */
class SignInPage(
    private val tracker: String,
    private val form: PageForm,
    private val authorizeUrl: String?,
    /** What the form opens filled in with: a saved proxy's host, port and user name, never a password. */
    filled: Map<String, String> = emptyMap(),
    private val onAnswer: (PageAnswer) -> Unit,
) {
    private val filled = filled - "password"

    /** The one-time path: 128 random bits, so no one on the network guesses it. */
    val path: String = ByteArray(16).also(SecureRandom()::nextBytes).joinToString("") { "%02x".format(it) }

    @Volatile
    private var server: ServerSocket? = null
    private val started = System.nanoTime()

    /** Listen on [port], or any port when it is taken; the port it listens on, or null. */
    fun start(port: Int = DEFAULT_PORT): Int? {
        val socket = try {
            bind(port)
        } catch (_: BindException) {
            runCatching { bind(0) }.getOrNull()
        } catch (e: java.io.IOException) {
            Log.w(TAG, "the sign-in page did not start", e)
            null
        } ?: return null
        server = socket
        thread(name = "sign-in page", isDaemon = true) { serve(socket) }
        return socket.localPort
    }

    fun stop() {
        runCatching { server?.close() }
        server = null
    }

    private fun bind(port: Int) = ServerSocket().apply {
        reuseAddress = true
        bind(InetSocketAddress(port))
    }

    private fun serve(socket: ServerSocket) {
        while (!socket.isClosed) {
            if (System.nanoTime() - started > LIFETIME_NANOS) {
                stop()
                return
            }
            val client = try {
                socket.soTimeout = ACCEPT_TIMEOUT_MILLIS
                socket.accept()
            } catch (_: java.net.SocketTimeoutException) {
                continue
            } catch (_: SocketException) {
                return
            }
            client.use { runCatching { answer(it) }.onFailure { e -> Log.w(TAG, "a sign-in page request failed", e) } }
            // Torrents' page takes one answer: once it has, no one else on the network can reach it.
            if (answered && torrents) {
                stop()
                return
            }
        }
    }

    private val torrents = form == PageForm.Torrent || form == PageForm.TorrentProxy || form == PageForm.Proxy

    @Volatile
    private var answered = false

    private fun answer(client: Socket) {
        client.soTimeout = READ_TIMEOUT_MILLIS
        val input = BufferedInputStream(client.getInputStream())
        val request = readRequest(input) ?: return
        val out = client.getOutputStream()
        val reply = when {
            request.target.substringBefore('?') != "/$path" -> Reply(404, page("Not found", "<p>This page is not here.</p>"))
            request.method == "GET" -> Reply(200, formPage(kept = filled))
            request.method == "POST" -> {
                val fields = parseForm(request.body)
                val answer = when (form) {
                    PageForm.Code -> fields["code"]?.trim()?.takeIf { it.isNotEmpty() }?.let(PageAnswer::Code)
                    PageForm.Password -> {
                        val user = fields["username"]?.trim().orEmpty()
                        val password = fields["password"].orEmpty()
                        if (user.isNotEmpty() && password.isNotEmpty()) PageAnswer.Password(user, password) else null
                    }
                    PageForm.Link -> fields["link"]?.trim()?.takeIf { it.isNotEmpty() }?.let(PageAnswer::Link)
                    PageForm.Torrent, PageForm.TorrentProxy, PageForm.Proxy -> torrentAnswer(fields)
                }
                if (answer == null) {
                    val problem = if (torrents) torrentProblem(fields) else "Fill in the form, then send it again."
                    Reply(200, formPage(error = problem, kept = fields - "password"))
                } else {
                    answered = true
                    onAnswer(answer)
                    val next = when (form) {
                        PageForm.Link -> "it shows the store, to join there"
                        PageForm.Torrent, PageForm.TorrentProxy -> "check the details there, then Turn on"
                        PageForm.Proxy -> "check the proxy there, then Save"
                        else -> "it signs in to ${escape(tracker)} now"
                    }
                    Reply(200, page("Sent to the TV", "<p>Look at the TV: $next. You can close this page.</p>"))
                }
            }
            else -> Reply(405, page("Not allowed", "<p>This page only takes its own form.</p>"))
        }
        val body = reply.html.toByteArray()
        val status = when (reply.status) {
            200 -> "200 OK"
            404 -> "404 Not Found"
            else -> "405 Method Not Allowed"
        }
        out.write(
            (
                "HTTP/1.1 $status\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: ${body.size}\r\n" +
                    "Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n"
                ).toByteArray(),
        )
        out.write(body)
        out.flush()
    }

    private data class Reply(val status: Int, val html: String)

    /** Torrents' details from the form, when they are whole: an http index, and a proxy's host and port when asked. */
    private fun torrentAnswer(fields: Map<String, String>): PageAnswer.Torrent? {
        // The proxy alone leaves the index as it is.
        val index = if (form == PageForm.Proxy) "" else fields["index"]?.trim().orEmpty()
        if (form != PageForm.Proxy && !TorrentWords.indexOk(index)) return null
        if (form == PageForm.Torrent) return PageAnswer.Torrent(index)
        val host = fields["host"]?.trim().orEmpty()
        val port = fields["port"]?.trim().orEmpty()
        if (!TorrentWords.proxyOk(host, port)) return null
        return PageAnswer.Torrent(index, host, port, fields["user"]?.trim().orEmpty(), fields["password"].orEmpty())
    }

    /** What is missing from torrents' form, as the TV's own fields say it. */
    private fun torrentProblem(fields: Map<String, String>): String {
        val index = fields["index"].orEmpty()
        if (form != PageForm.Proxy && !TorrentWords.indexOk(index)) return TorrentWords.indexNote(index)
        return "The proxy needs a host and a port from 1 to 65535."
    }

    private fun formPage(error: String? = null, kept: Map<String, String> = emptyMap()): String {
        val value = { name: String -> kept[name]?.let { " value=\"${escape(it)}\"" }.orEmpty() }
        val steps = when (form) {
            PageForm.Code -> buildString {
                append("<ol>")
                if (authorizeUrl != null) {
                    append("<li><a href=\"${escape(authorizeUrl)}\" target=\"_blank\" rel=\"noreferrer\">Open $tracker</a> and allow anistream.</li>")
                }
                append("<li>Copy the code it shows, or the whole address the page ends on.</li>")
                append("<li>Paste it here and send it to the TV.</li></ol>")
            }
            PageForm.Password -> "<p>Your $tracker user name or email, and password. They go to the TV, which signs in to $tracker with them and keeps only the token.</p>"
            PageForm.Link -> "<p>Paste the plugin store's invite link. The TV shows the store before it joins.</p>"
            PageForm.Torrent -> "<p>The torrent index anistream asks for torrents. anistream ships none. The page closes once it is sent.</p>"
            PageForm.TorrentProxy -> "<p>The torrent index anistream asks for torrents, and the SOCKS5 proxy torrent traffic goes through. anistream ships no index. The page closes once it is sent.</p>"
            PageForm.Proxy -> "<p>The SOCKS5 proxy torrent traffic goes through. ${TorrentWords.PASSWORD_KEPT} The TV checks the proxy before it saves it. The page closes once it is sent.</p>"
        }
        val index = "<label>Torrent index<input name=\"index\" type=\"url\" placeholder=\"https://\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"${value("index")}></label>"
        val fields = when (form) {
            PageForm.Code -> "<label>Code<textarea name=\"code\" rows=\"4\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"></textarea></label>"
            PageForm.Password ->
                "<label>User name or email<input name=\"username\" autocomplete=\"username\" autocapitalize=\"off\"></label>" +
                    "<label>Password<input name=\"password\" type=\"password\" autocomplete=\"current-password\"></label>"
            PageForm.Link -> "<label>Invite link<input name=\"link\" type=\"url\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"></label>"
            PageForm.Torrent -> index
            PageForm.TorrentProxy, PageForm.Proxy -> (if (form == PageForm.Proxy) "" else index) +
                "<label>Proxy host<input name=\"host\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"${value("host")}></label>" +
                "<label>Proxy port<input name=\"port\" inputmode=\"numeric\" autocomplete=\"off\"${value("port")}></label>" +
                "<p>User name and password only if the proxy asks for them.</p>" +
                "<label>User name<input name=\"user\" autocomplete=\"off\" autocapitalize=\"off\"${value("user")}></label>" +
                "<label>Password<input name=\"password\" type=\"password\" autocomplete=\"off\"></label>"
        }
        val warning = error?.let { "<p class=\"error\">${escape(it)}</p>" }.orEmpty()
        return page(
            when (form) {
                PageForm.Link -> "Add a plugin store"
                PageForm.Torrent, PageForm.TorrentProxy, PageForm.Proxy -> "Torrents for anistream"
                else -> "Sign in to $tracker"
            },
            "$steps$warning<form method=\"post\" action=\"/$path\">$fields<button type=\"submit\">Send to the TV</button></form>",
        )
    }

    companion object {
        const val TAG = "anistream-sign-in"
        const val DEFAULT_PORT = 47_110

        /** The page lives ten minutes at most. */
        const val LIFETIME_NANOS = 10 * 60 * 1_000_000_000L
        private const val ACCEPT_TIMEOUT_MILLIS = 1_000
        private const val READ_TIMEOUT_MILLIS = 10_000
        private const val MAX_BODY = 16 * 1024

        data class Request(val method: String, val target: String, val body: String)

        /** A request off [input]: its line, its headers skipped but for the length, its body. */
        fun readRequest(input: InputStream): Request? {
            val line = readLine(input) ?: return null
            val parts = line.split(' ')
            if (parts.size < 2) return null
            var length = 0
            while (true) {
                val header = readLine(input) ?: return null
                if (header.isEmpty()) break
                if (header.substringBefore(':').trim().equals("content-length", ignoreCase = true)) {
                    length = header.substringAfter(':').trim().toIntOrNull()?.coerceIn(0, MAX_BODY) ?: 0
                }
            }
            val body = ByteArray(length)
            var read = 0
            while (read < length) {
                val n = input.read(body, read, length - read)
                if (n < 0) break
                read += n
            }
            return Request(parts[0], parts[1], String(body, 0, read))
        }

        private fun readLine(input: InputStream): String? {
            val bytes = ByteArrayOutputStream()
            while (true) {
                val b = input.read()
                if (b < 0) return if (bytes.size() == 0) null else bytes.toString()
                if (b == '\n'.code) return bytes.toString().trimEnd('\r')
                if (bytes.size() > MAX_BODY) return null
                bytes.write(b)
            }
        }

        /** A form body's fields, decoded. */
        fun parseForm(body: String): Map<String, String> = body.split('&')
            .filter { it.isNotEmpty() }
            .associate {
                URLDecoder.decode(it.substringBefore('='), "UTF-8") to URLDecoder.decode(it.substringAfter('=', ""), "UTF-8")
            }

        fun escape(text: String): String = text
            .replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

        /** A small page in the app's colours, readable on a phone. */
        fun page(title: String, body: String): String = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(title)}</title>
<style>
body{margin:0;padding:24px 16px;background:#161A2E;color:#E4E0D6;font:17px/1.5 system-ui,sans-serif}
main{max-width:480px;margin:0 auto}h1{font-size:24px;font-weight:600}a{color:#F2A64B}
label{display:block;margin:16px 0;color:#9298B5}
input,textarea{display:block;box-sizing:border-box;width:100%;margin-top:6px;padding:12px;border:1px solid #434653;border-radius:12px;background:#222638;color:#E4E0D6;font:inherit}
button{margin-top:8px;padding:14px 24px;border:0;border-radius:999px;background:#F2A64B;color:#161A2E;font:600 17px system-ui,sans-serif}
.error{color:#E06B54}
</style></head><body><main><h1>${escape(title)}</h1>$body</main></body></html>"""
    }
}

/**
 * AniList's answer as the engine takes it: the address the sign-in ended on as it came, or the
 * bare code from AniList's page put into that address, with the [state] the request asked with.
 */
fun anilistCallback(pasted: String, callbackScheme: String, state: String?): String {
    val text = pasted.trim()
    if (text.contains("access_token=")) return text
    val stated = state?.let { "&state=$it" }.orEmpty()
    return "$callbackScheme://auth/anilist#access_token=$text&token_type=Bearer$stated"
}
