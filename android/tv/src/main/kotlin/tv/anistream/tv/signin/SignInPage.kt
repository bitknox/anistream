// The TV's sign-in page: while TVSignIn is open, a one-page form on the local network at a
// one-time path, where a phone pastes AniList's code or types Kitsu's user name and password. It
// stops when the screen closes or after ten minutes. The same page takes a plugin store's link and
// torrents' details. What the phone sends is sealed with the one-time key in the address's
// fragment (PageKey), so nothing typed crosses the network in plain text.

package tv.anistream.tv.signin

import tv.anistream.i18n.Strings
import tv.anistream.i18n.R
import android.content.Context
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
import java.net.URLEncoder
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.Semaphore
import java.util.concurrent.TimeUnit
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
 * first when there is one. Each answer goes to [onAnswer], on a thread of the page's, one at a time.
 *
 * Each client is served on a thread of its own, [MAX_CLIENTS] at once, and is cut off
 * [clientDeadlineMillis] after it connected however slowly it sends: one that trickles its request
 * holds no one else up. The page stops [lifetimeMillis] after it started, whatever its clients do.
 */
class SignInPage(
    private val context: Context,
    private val tracker: String,
    private val form: PageForm,
    private val authorizeUrl: String?,
    /** What the form opens filled in with: a saved proxy's host, port and user name, never a password. */
    filled: Map<String, String> = emptyMap(),
    private val lifetimeMillis: Long = LIFETIME_MILLIS,
    private val clientDeadlineMillis: Long = CLIENT_DEADLINE_MILLIS,
    private val onAnswer: (PageAnswer) -> Unit,
) {
    private val filled = filled - "password"

    /** The one-time path: 128 random bits, so no one on the network guesses it. */
    val path: String = ByteArray(16).also(SecureRandom()::nextBytes).joinToString("") { "%02x".format(it) }

    /** The one-time key the form is sealed with, which reaches the phone only in the QR code. */
    internal val key = PageKey()

    /** The page's whole address on [host] and [port]: its path, and its key after the #. */
    fun address(host: String, port: Int): String = "http://$host:$port/$path${key.fragment}"

    @Volatile
    private var server: ServerSocket? = null

    /** What closes the page at the end of its life, and each client at its deadline. */
    private val timer = Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "sign-in page timer").apply { isDaemon = true } }

    /** The clients being served, closed with the page. */
    private val clients = ConcurrentHashMap.newKeySet<Socket>()
    private val serving = Semaphore(MAX_CLIENTS)

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
        timer.schedule({ stop() }, lifetimeMillis, TimeUnit.MILLISECONDS)
        thread(name = "sign-in page", isDaemon = true) { serve(socket) }
        return socket.localPort
    }

    fun stop() {
        runCatching { server?.close() }
        server = null
        clients.forEach { runCatching { it.close() } }
        timer.shutdownNow()
    }

    private fun bind(port: Int) = ServerSocket().apply {
        reuseAddress = true
        bind(InetSocketAddress(port))
    }

    private fun serve(socket: ServerSocket) {
        while (!socket.isClosed) {
            val client = try {
                socket.accept()
            } catch (_: SocketException) {
                return
            } catch (e: java.io.IOException) {
                Log.w(TAG, "the sign-in page could not take a client", e)
                continue
            }
            // As many at once as a household's phones; one more is turned away rather than kept waiting.
            if (!serving.tryAcquire()) {
                runCatching { client.close() }
                continue
            }
            clients += client
            val deadline = runCatching { timer.schedule({ runCatching { client.close() } }, clientDeadlineMillis, TimeUnit.MILLISECONDS) }.getOrNull()
            thread(name = "sign-in page client", isDaemon = true) {
                try {
                    client.use { runCatching { answer(it) }.onFailure { e -> Log.w(TAG, "a sign-in page request failed", e) } }
                } finally {
                    deadline?.cancel(false)
                    clients -= client
                    serving.release()
                }
                // Torrents' page takes one answer: once it has, no one else on the network can reach it.
                if (answered && torrents) stop()
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
            request.target.substringBefore('?') != "/$path" -> Reply(404, page(context.getString(R.string.torrents_page_not_found), "<p>${escape(context.getString(R.string.tv_page_not_found_text))}</p>"))
            request.method == "GET" -> Reply(200, formPage())
            request.method == "POST" -> {
                // Only a sealed form opens: a plain one, or one sealed for another page, is turned away.
                val fields = key.open(path, request.body)?.let(::parseForm)
                val answer = fields?.let(::answerFrom)
                when {
                    fields == null -> Reply(400, said(ok = false, context.getString(R.string.torrents_page_unreadable)))
                    answer == null -> Reply(200, said(ok = false, if (torrents) torrentProblem(fields) else context.getString(R.string.torrents_page_fill_form)))
                    // Torrents' page takes one answer, so two sent at once are not both taken.
                    else -> synchronized(this) {
                        if (answered && torrents) return
                        answered = true
                        onAnswer(answer)
                        val next = when (form) {
                            PageForm.Link -> Strings.tvPageNextLink(context)
                            PageForm.Torrent, PageForm.TorrentProxy -> Strings.tvPageNextTorrent(context)
                            PageForm.Proxy -> Strings.tvPageNextProxy(context)
                            else -> Strings.tvPageNextSignin(context, tracker)
                        }
                        Reply(200, said(ok = true, Strings.torrentsPageLookAtTv(context, next)))
                    }
                }
            }
            else -> Reply(405, page(context.getString(R.string.torrents_page_not_allowed), "<p>${escape(context.getString(R.string.tv_page_not_allowed_text))}</p>"))
        }
        val body = reply.html.toByteArray()
        val status = when (reply.status) {
            200 -> "200 OK"
            400 -> "400 Bad Request"
            404 -> "404 Not Found"
            else -> "405 Method Not Allowed"
        }
        val type = if (request.method == "POST" && reply.status != 405) "application/json" else "text/html; charset=utf-8"
        out.write(
            (
                "HTTP/1.1 $status\r\nContent-Type: $type\r\nContent-Length: ${body.size}\r\n" +
                    "Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n"
                ).toByteArray(),
        )
        out.write(body)
        out.flush()
    }

    private data class Reply(val status: Int, val html: String)

    /** The page's form, answered: its fields as the TV takes them, or null when they are not whole. */
    private fun answerFrom(fields: Map<String, String>): PageAnswer? = when (form) {
        PageForm.Code -> fields["code"]?.trim()?.takeIf { it.isNotEmpty() }?.let(PageAnswer::Code)
        PageForm.Password -> {
            val user = fields["username"]?.trim().orEmpty()
            val password = fields["password"].orEmpty()
            if (user.isNotEmpty() && password.isNotEmpty()) PageAnswer.Password(user, password) else null
        }
        PageForm.Link -> fields["link"]?.trim()?.takeIf { it.isNotEmpty() }?.let(PageAnswer::Link)
        PageForm.Torrent, PageForm.TorrentProxy, PageForm.Proxy -> torrentAnswer(fields)
    }

    /** The reply to a sent form, for the page's script: whether the TV took it, and what to say. Never a field sent. */
    private fun said(ok: Boolean, text: String): String = "{\"ok\":$ok,\"text\":\"${json(text)}\"}"

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
        if (form != PageForm.Proxy && !TorrentWords.indexOk(index)) return TorrentWords.indexNote(context, index)
        return context.getString(R.string.torrents_proxy_invalid)
    }

    private fun formPage(): String {
        val steps = when (form) {
            PageForm.Code -> buildString {
                append("<ol>")
                if (authorizeUrl != null) {
                    val link = "<a href=\"${escape(authorizeUrl)}\" target=\"_blank\" rel=\"noreferrer\">${escape(Strings.tvPageOpenLink(context, tracker))}</a>"
                    append("<li>${Strings.tvPageStepOpen(context, link)}</li>")
                }
                append("<li>${escape(context.getString(R.string.tv_page_step_copy))}</li>")
                append("<li>${escape(context.getString(R.string.tv_page_step_paste))}</li></ol>")
            }
            PageForm.Password -> "<p>${escape(Strings.tvPagePassword(context, tracker))}</p>"
            PageForm.Link -> "<p>${escape(context.getString(R.string.tv_page_link))}</p>"
            PageForm.Torrent -> "<p>${escape(introOf(R.string.tv_page_torrent))}</p>"
            PageForm.TorrentProxy -> "<p>${escape(introOf(R.string.tv_page_torrent_proxy))}</p>"
            PageForm.Proxy -> "<p>${escape(Strings.tvPageProxy(context, context.getString(TorrentWords.PASSWORD_KEPT)) + " " + context.getString(R.string.tv_page_closes_once_sent))}</p>"
        }
        // No field has a name, and the form no action: only the script sends it, sealed.
        val input = { label: String, id: String, attributes: String -> "<label>$label<input id=\"$id\" data-field $attributes></label>" }
        val index = input(escape(context.getString(R.string.torrents_index)), "index", "type=\"url\" placeholder=\"https://\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"")
        val fields = when (form) {
            PageForm.Code -> "<label>${escape(context.getString(R.string.tv_page_label_code))}<textarea id=\"code\" data-field rows=\"4\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"></textarea></label>"
            PageForm.Password ->
                input(escape(context.getString(R.string.accounts_user_or_email)), "username", "autocomplete=\"username\" autocapitalize=\"off\"") +
                    input(escape(context.getString(R.string.torrents_proxy_password)), "password", "type=\"password\" autocomplete=\"current-password\"")
            PageForm.Link -> input(escape(context.getString(R.string.stores_invite_link)), "link", "type=\"url\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"")
            PageForm.Torrent -> index
            PageForm.TorrentProxy, PageForm.Proxy -> (if (form == PageForm.Proxy) "" else index) +
                input(escape(context.getString(R.string.torrents_proxy_host)), "host", "autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"") +
                input(escape(context.getString(R.string.torrents_proxy_port)), "port", "inputmode=\"numeric\" autocomplete=\"off\"") +
                "<p>${escape(context.getString(R.string.torrents_sheet_proxy_auth_note))}</p>" +
                input(escape(context.getString(R.string.torrents_proxy_user)), "user", "autocomplete=\"off\" autocapitalize=\"off\"") +
                input(escape(context.getString(R.string.torrents_proxy_password)), "password", "type=\"password\" autocomplete=\"off\"")
        }
        // A saved proxy's details, sealed like the answer, for the script to fill in.
        val sealed = filled.takeIf { it.isNotEmpty() }?.let { key.seal(path, formBody(it)) }
        val prefill = sealed?.let { " data-filled=\"$it\"" }.orEmpty()
        return page(
            when (form) {
                PageForm.Link -> context.getString(R.string.stores_add_title)
                PageForm.Torrent, PageForm.TorrentProxy, PageForm.Proxy -> context.getString(R.string.tv_page_title_torrents)
                else -> context.getString(R.string.torrents_page_sign_in, tracker)
            },
            "$steps<noscript><p class=\"error\">${escape(context.getString(R.string.torrents_page_needs_script))}</p></noscript>" +
                "<form id=\"form\" hidden$prefill>$fields<p id=\"problem\" class=\"error\" role=\"alert\" hidden></p><button type=\"submit\">${escape(context.getString(R.string.tv_page_send))}</button></form>" +
                "<script>${PAGE_CRYPTO}</script><script>${pageScript(context)}</script>",
        )
    }

    /** A torrent intro, its legal line and its closing sentence after it. */
    private fun introOf(res: Int): String =
        listOf(context.getString(res), Strings.legalHostsNoContent(context), context.getString(R.string.tv_page_closes_once_sent)).joinToString(" ") // i18n-skip

    companion object {
        const val TAG = "anistream-sign-in"

        /** @noble/ciphers' AES-GCM and the seal around it, bundled by android/tv/page-crypto/build.sh. */
        private val PAGE_CRYPTO: String by lazy {
            SignInPage::class.java.getResourceAsStream("page-crypto.js")!!.use { it.readBytes().decodeToString() }
        }

        /**
         * The page's own script: the key from the address, the form filled in from what the TV
         * sealed, and the form sealed and posted, the reply said in place.
         */
        private fun pageScript(context: Context): String = """
(function () {
  var page = window.anistreamPage;
  var key = page.keyFrom(location.hash);
  var path = location.pathname.slice(1);
  var form = document.getElementById('form');
  var problem = document.getElementById('problem');
  var main = document.querySelector('main');
  function say(title, text, error) {
    main.textContent = '';
    var heading = document.createElement('h1');
    heading.textContent = title;
    var line = document.createElement('p');
    line.textContent = text;
    if (error) line.className = 'error';
    main.appendChild(heading);
    main.appendChild(line);
  }
  if (!key) {
    say('${json(context.getString(R.string.tv_page_scan_again))}', "${json(context.getString(R.string.torrents_page_missing_key))}", true);
    return;
  }
  var filled = form.getAttribute('data-filled');
  if (filled) {
    try {
      new URLSearchParams(page.open(key, path, filled)).forEach(function (value, name) {
        var field = document.getElementById(name);
        if (field) field.value = value;
      });
    } catch (e) {}
  }
  form.hidden = false;
  var button = form.querySelector('button');
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var fields = new URLSearchParams();
    form.querySelectorAll('[data-field]').forEach(function (field) { fields.append(field.id, field.value); });
    button.disabled = true;
    problem.hidden = true;
    fetch('/' + path, { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: page.seal(key, path, fields.toString()) })
      .then(function (response) { return response.json(); })
      .then(function (reply) {
        if (reply.ok) {
          say('${json(context.getString(R.string.tv_page_sent_title))}', reply.text, false);
        } else {
          problem.textContent = reply.text;
          problem.hidden = false;
          button.disabled = false;
        }
      })
      .catch(function () {
        problem.textContent = '${json(context.getString(R.string.tv_page_no_answer))}';
        problem.hidden = false;
        button.disabled = false;
      });
  });
})();
"""

        /** [text] as a JSON string's inside. */
        fun json(text: String): String = buildString {
            text.forEach { c ->
                when {
                    c == '"' -> append("\\\"")
                    c == '\\' -> append("\\\\")
                    c == '<' -> append("\\u003c")
                    c == '\'' -> append("\\u0027")
                    c < ' ' -> append("\\u%04x".format(c.code))
                    else -> append(c)
                }
            }
        }

        /** [fields] as a form body, as the page's script sends them. */
        fun formBody(fields: Map<String, String>): String = fields.entries.joinToString("&") {
            URLEncoder.encode(it.key, "UTF-8") + "=" + URLEncoder.encode(it.value, "UTF-8")
        }
        const val DEFAULT_PORT = 47_110

        /** The page lives ten minutes at most. */
        const val LIFETIME_MILLIS = 10 * 60 * 1_000L

        /** A client has this long from connecting to its answer, however it sends its request. */
        const val CLIENT_DEADLINE_MILLIS = 15_000L

        /** Clients served at once. */
        const val MAX_CLIENTS = 4
        private const val READ_TIMEOUT_MILLIS = 10_000
        private const val MAX_BODY = 16 * 1024

        /** Header lines a request may have; a browser sends a dozen or two. */
        private const val MAX_HEADERS = 64

        data class Request(val method: String, val target: String, val body: String)

        /** A request off [input]: its line, its headers skipped but for the length, its body. */
        fun readRequest(input: InputStream): Request? {
            val line = readLine(input) ?: return null
            val parts = line.split(' ')
            if (parts.size < 2) return null
            var length = 0
            var headers = 0
            while (true) {
                val header = readLine(input) ?: return null
                if (header.isEmpty()) break
                if (++headers > MAX_HEADERS) return null
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
        fun page(title: String, body: String, lang: String = java.util.Locale.getDefault().toLanguageTag()): String = """<!doctype html>
<html lang="${escape(lang)}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
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
