package tv.anistream.tv

import java.net.HttpURLConnection
import java.net.URI
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.tv.signin.PageAnswer
import tv.anistream.tv.signin.PageForm
import tv.anistream.tv.signin.SignInPage
import tv.anistream.tv.signin.anilistCallback
import tv.anistream.tv.signin.spaced

/** The TV's sign-in page, served on loopback: its one-time path, its forms, and what it hands on. */
class SignInPageTest {
    private val answers = LinkedBlockingQueue<PageAnswer>()
    private var page: SignInPage? = null

    @After
    fun tearDown() {
        page?.stop()
    }

    private fun start(form: PageForm): String {
        val started = SignInPage("Kitsu", form, authorizeUrl = "https://tracker.example/authorize?state=s1") { answers.add(it) }
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

    private fun post(url: String, form: String): String {
        val connection = URI(url).toURL().openConnection() as HttpURLConnection
        connection.requestMethod = "POST"
        connection.doOutput = true
        connection.setRequestProperty("Content-Type", "application/x-www-form-urlencoded")
        connection.outputStream.use { it.write(form.toByteArray()) }
        return connection.inputStream.bufferedReader().readText()
    }

    @Test
    fun only_the_one_time_path_answers() {
        val url = start(PageForm.Password)
        assertEquals(404, get(url.substringBeforeLast('/') + "/guess").first)
        val (code, body) = get(url)
        assertEquals(200, code)
        assertTrue(body.contains("name=\"password\""))
    }

    @Test
    fun the_password_form_hands_both_fields_to_the_tv() {
        val url = start(PageForm.Password)
        val reply = post(url, "username=mika%40mail.example&password=p%26ss+word")
        assertTrue(reply.contains("Sent to the TV"))
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
    fun a_device_code_reads_in_fours() {
        assertEquals("8F3K 2QPL", spaced("8F3K2QPL"))
    }
}
