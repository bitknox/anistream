package tv.anistream.tv

import tv.anistream.i18n.R
import java.net.HttpURLConnection
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import java.net.URI
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import tv.anistream.tv.signin.PageAnswer
import tv.anistream.tv.signin.PageForm
import tv.anistream.tv.signin.PageKey
import tv.anistream.tv.signin.SignInPage
import tv.anistream.tv.signin.anilistCallback
import tv.anistream.tv.signin.spaced

/** The TV's sign-in page, served on loopback: its one-time path, its forms, and what it hands on. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SignInPageTest {
    private val context = RuntimeEnvironment.getApplication()
    private val answers = LinkedBlockingQueue<PageAnswer>()
    private var page: SignInPage? = null

    @After
    fun tearDown() {
        page?.stop()
    }

    private fun start(form: PageForm): String {
        val started = SignInPage(context, "Kitsu", form, authorizeUrl = "https://tracker.example/authorize?state=s1") { answers.add(it) }
        page = started
        val port = started.start(port = 0)!!
        return "http://127.0.0.1:$port/${started.path}"
    }

    private fun get(url: String): Pair<Int, String> {
        val connection = URI(url).toURL().openConnection() as HttpURLConnection
        val code = connection.responseCode
        val body = (if (code < 400) connection.inputStream else connection.errorStream).bufferedReader().readText()
        return code to body
    }

    /** [form] sealed with the page's key, as its script sends it: the TV's JSON reply. */
    private fun post(url: String, form: String): String = postRaw(url, page!!.key.seal(page!!.path, form)).second

    private fun postRaw(url: String, body: String): Pair<Int, String> {
        val connection = URI(url).toURL().openConnection() as HttpURLConnection
        connection.requestMethod = "POST"
        connection.doOutput = true
        connection.setRequestProperty("Content-Type", "text/plain")
        connection.outputStream.use { it.write(body.toByteArray()) }
        val code = connection.responseCode
        return code to (if (code < 400) connection.inputStream else connection.errorStream).bufferedReader().readText()
    }

    @Test
    fun only_the_one_time_path_answers() {
        val url = start(PageForm.Password)
        assertEquals(404, get(url.substringBeforeLast('/') + "/guess").first)
        val (code, body) = get(url)
        assertEquals(200, code)
        assertTrue(body.contains("id=\"password\""))
        // Nothing the browser could send by itself: no field has a name, and the form no action.
        assertFalse(Regex("<(input|form)[^>]*\\b(name|action)=").containsMatchIn(body))
    }

    @Test
    fun a_form_not_sealed_with_the_pages_key_is_turned_away() {
        val url = start(PageForm.Password)
        val plain = postRaw(url, "username=mika&password=secret")
        assertEquals(400, plain.first)
        assertTrue(plain.second.contains(context.getString(R.string.torrents_page_unreadable)))
        // Sealed with another key, or for another page, it opens no better.
        val other = PageKey().seal(page!!.path, "username=mika&password=secret")
        assertEquals(400, postRaw(url, other).first)
        assertEquals(400, postRaw(url, page!!.key.seal("another-path", "username=mika&password=secret")).first)
        assertTrue(answers.isEmpty())
    }

    @Test
    fun the_key_round_trips_through_the_address_and_nothing_else_opens() {
        val key = PageKey()
        val again = PageKey.from(key.fragment)!!
        assertEquals("host=proxy.example", again.open("p", key.seal("p", "host=proxy.example")))
        assertNull(PageKey.from("#k=short"))
        assertNull(PageKey.from(""))
        assertNull(again.open("p", "not sealed"))
    }

    @Test
    fun the_password_form_hands_both_fields_to_the_tv() {
        val url = start(PageForm.Password)
        val reply = post(url, "username=mika%40mail.example&password=p%26ss+word")
        assertTrue(reply.contains("\"ok\":true"))
        assertEquals(PageAnswer.Password("mika@mail.example", "p&ss word"), answers.poll(5, TimeUnit.SECONDS))
    }

    @Test
    fun an_empty_form_is_asked_for_again() {
        val url = start(PageForm.Code)
        assertTrue(post(url, "code=").contains("Fill in the form"))
        assertTrue(answers.isEmpty())
        assertTrue(get(url).second.contains("https://tracker.example/authorize?state=s1"))
    }

    @Test
    fun a_bare_code_becomes_the_redirect_with_the_requests_state() {
        assertEquals(
            "anistream://auth/anilist#access_token=a.b.c&token_type=Bearer&state=s1",
            anilistCallback(" a.b.c\n", "anistream", "s1"),
        )
        val pasted = "https://anistream.example/auth/anilist#access_token=a.b.c&state=s1"
        assertEquals(pasted, anilistCallback(pasted, "anistream", "s1"))
    }

    @Test
    fun torrents_page_takes_one_answer_and_closes() {
        val url = start(PageForm.TorrentProxy)
        val reply = post(url, "index=https%3A%2F%2Findex.example%2Frss&host=proxy.example&port=1080&user=mika&password=p%26ss")
        assertTrue(reply.contains("\"ok\":true"))
        assertEquals(
            PageAnswer.Torrent("https://index.example/rss", "proxy.example", "1080", "mika", "p&ss"),
            answers.poll(5, TimeUnit.SECONDS),
        )
        // No one else on the network reaches it once it has its answer: it stops after the reply
        // is written, so wait for that rather than race it.
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        var gone = false
        while (!gone && System.nanoTime() < deadline) {
            gone = runCatching { get(url) }.isFailure
            if (!gone) Thread.sleep(20)
        }
        assertTrue(gone)
    }

    @Test
    fun torrents_page_says_what_is_missing_and_never_what_was_sent() {
        val url = start(PageForm.TorrentProxy)
        val reply = post(url, "index=ftp%3A%2F%2Findex.example&host=proxy.example&port=1080&password=secret")
        assertTrue(reply.contains("Use an http or https address."))
        assertTrue(reply.contains("\"ok\":false"))
        assertFalse(reply.contains("proxy.example"))
        assertFalse(reply.contains("secret"))
        assertTrue(answers.isEmpty())
        // A proxy without its port is not whole either.
        assertTrue(post(url, "index=https%3A%2F%2Findex.example&host=proxy.example&port=").contains("port"))
        assertTrue(answers.isEmpty())
    }

    @Test
    fun edit_proxy_page_opens_filled_in_but_for_the_password_and_takes_no_index() {
        val started = SignInPage(
            context,
            "torrents",
            PageForm.Proxy,
            authorizeUrl = null,
            onAnswer = { answers.add(it) },
            filled = mapOf("host" to "proxy.example", "port" to "1080", "user" to "mika", "password" to "kept"),
        )
        page = started
        val url = "http://127.0.0.1:${started.start(port = 0)!!}/${started.path}"
        val (code, form) = get(url)
        assertEquals(200, code)
        // Filled in sealed: the page's script opens it, and nothing of it crosses in plain text.
        assertFalse(form.contains("proxy.example"))
        assertFalse(form.contains("mika"))
        val sealed = Regex("data-filled=\"([A-Za-z0-9_-]+)\"").find(form)!!.groupValues[1]
        assertEquals("host=proxy.example&port=1080&user=mika", started.key.open(started.path, sealed))
        assertFalse(form.contains("kept"))
        assertFalse(form.contains("id=\"index\""))
        // Its password left empty, the TV checks with the one it keeps.
        assertTrue(post(url, "host=proxy.example&port=1081&user=mika&password=").contains("\"ok\":true"))
        assertEquals(PageAnswer.Torrent("", "proxy.example", "1081", "mika", ""), answers.poll(5, TimeUnit.SECONDS))
    }

    /**
     * The page's own scripts, run in Node over a stand-in for the browser: the TV's sealed
     * prefill opens, what the script posts holds nothing typed in plain text, and the TV takes it.
     * Without its key, the page says so and sends nothing.
     */
    @Test
    fun the_pages_script_posts_the_form_sealed_and_the_tv_takes_it() {
        assumeTrue("Node runs the page's script", runCatching { ProcessBuilder("node", "--version").start().waitFor() == 0 }.getOrDefault(false))
        val started = SignInPage(context, "torrents", PageForm.Proxy, authorizeUrl = null, filled = mapOf("host" to "proxy.example", "port" to "1080", "user" to "mika")) { answers.add(it) }
        page = started
        val url = "http://127.0.0.1:${started.start(port = 0)!!}/${started.path}"
        val html = get(url).second
        val run = { hash: String -> browse(html, started.path, hash, typed = mapOf("port" to "1081", "password" to "p&ss word")) }

        val missing = run("")
        assertEquals("", missing.getValue("body"))
        assertEquals("hidden", missing.getValue("form"))
        assertTrue(missing.getValue("said").contains("missing its key"))

        val sent = run(started.key.fragment)
        assertEquals("proxy.example", sent.getValue("host"))
        val body = sent.getValue("body")
        listOf("proxy.example", "mika", "1081", "p&ss", "p%26ss", "password").forEach { assertFalse("$it crossed in plain text", body.contains(it)) }
        assertEquals(200, postRaw(url, body).first)
        assertEquals(PageAnswer.Torrent("", "proxy.example", "1081", "mika", "p&ss word"), answers.poll(5, TimeUnit.SECONDS))
    }

    /** [html]'s scripts in Node, at [path] with [hash], [typed] into its fields once it has filled them, then sent. */
    private fun browse(html: String, path: String, hash: String, typed: Map<String, String>): Map<String, String> {
        val scripts = Regex("<script>(.*?)</script>", RegexOption.DOT_MATCHES_ALL).findAll(html).map { it.groupValues[1] }.joinToString("\n")
        val ids = Regex("id=\"([a-z]+)\" data-field").findAll(html).map { it.groupValues[1] }.toList()
        val filled = Regex("data-filled=\"([^\"]+)\"").find(html)?.groupValues?.get(1)
        val quote = { text: String? -> if (text == null) "null" else "\"" + SignInPage.json(text) + "\"" }
        val harness = """
            const fields = {};
            for (const id of [${ids.joinToString(",") { quote(it) }}]) fields[id] = { id, value: '' };
            const posted = [];
            let submit = null;
            const problem = { hidden: true, textContent: '' };
            const button = { disabled: false };
            const form = {
              hidden: true,
              getAttribute: (name) => (name === 'data-filled' ? ${quote(filled)} : null),
              querySelector: () => button,
              querySelectorAll: () => Object.values(fields),
              addEventListener: (type, listener) => { submit = listener; },
            };
            const said = [];
            const main = { set textContent(text) {}, appendChild: (node) => said.push(node.textContent) };
            globalThis.window = globalThis;
            globalThis.location = { hash: ${quote(hash)}, pathname: '/' + ${quote(path)} };
            globalThis.document = {
              getElementById: (id) => (id === 'form' ? form : id === 'problem' ? problem : fields[id] || null),
              querySelector: () => main,
              createElement: () => ({}),
            };
            globalThis.fetch = (url, init) => { posted.push(init.body); return new Promise(() => {}); };
        """.trimIndent() + "\n" + scripts + "\n" + """
            const host = fields.host ? fields.host.value : '';
            ${typed.entries.joinToString("\n") { "fields[${quote(it.key)}].value = ${quote(it.value)};" }}
            if (submit) submit({ preventDefault() {} });
            console.log(JSON.stringify({ host, body: posted[0] || '', form: form.hidden ? 'hidden' : 'shown', said: said.join(' ') }));
        """.trimIndent()
        val file = java.io.File.createTempFile("page", ".js").apply { writeText(harness) }
        try {
            val process = ProcessBuilder("node", file.path).redirectErrorStream(true).start()
            val out = process.inputStream.bufferedReader().readText()
            assertTrue(out, process.waitFor(30, TimeUnit.SECONDS) && process.exitValue() == 0)
            return Regex("\"(\\w+)\":\"((?:[^\"\\\\]|\\\\.)*)\"").findAll(out.lines().last { it.startsWith("{") })
                .associate { it.groupValues[1] to it.groupValues[2] }
        } finally {
            file.delete()
        }
    }

    @Test
    fun torrents_answer_never_says_its_password() {
        val said = PageAnswer.Torrent("https://index.example/rss", "proxy.example", "1080", "mika", "p&ss").toString()
        assertFalse(said.contains("p&ss"))
        assertFalse(said.contains("mika"))
    }

    @Test
    fun a_device_code_reads_in_fours() {
        assertEquals("8F3K 2QPL", spaced("8F3K2QPL"))
    }
}
