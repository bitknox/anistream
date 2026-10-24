package tv.anistream.store

import java.io.File
import java.net.URI
import java.net.URLDecoder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.DeviceClass
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.ProviderRow

/** The bug report's address as the app builds it, held to the repository's issue form. */
class BugReportTest {
    private val phone = BugReportDevice(appVersion = "2.0.0", osVersion = "16", device = DeviceClass.PHONE, build = "release")

    private fun row(id: String, kind: String, origin: ProviderOrigin) = ProviderRow(
        id = id, kind = kind, state = "ready", latencyMs = null, lastError = null, usable = true,
        heldBack = false, name = id, version = null, hosts = listOf("$id.example"), origin = origin,
        disabled = false, update = null,
    )

    private fun parameters(url: String): Map<String, String> =
        URI(url).rawQuery.split('&').associate {
            val (k, v) = it.split('=', limit = 2)
            URLDecoder.decode(k, "UTF-8") to URLDecoder.decode(v.replace("+", "%2B"), "UTF-8")
        }

    /** The ids of the inputs in `.github/ISSUE_TEMPLATE/bug_report.yml`. */
    private fun formIds(): Set<String> {
        var dir: File? = File("").absoluteFile
        while (dir != null && !File(dir, ".github/ISSUE_TEMPLATE/bug_report.yml").exists()) dir = dir.parentFile
        val form = File(requireNotNull(dir) { "the issue form" }, ".github/ISSUE_TEMPLATE/bug_report.yml")
        return form.readLines().map { it.trim() }.filter { it.startsWith("id: ") }.map { it.removePrefix("id: ") }.toSet()
    }

    @Test
    fun theAddressIsAnIssueOnTheTrackerWithTheDiagnosticsFilledIn() {
        val url = BugReports.url(phone, listOf(row("torrents", "torrent", ProviderOrigin.BuiltIn)), null)
        assertTrue(url.startsWith("https://github.com/bitknox/anistream/issues/new?"))
        val found = parameters(url)
        assertEquals("bug_report.yml", found["template"])
        assertFalse(found.containsKey("title"))
        val text = found.getValue("diagnostics")
        assertTrue(text, text.contains("Android 16, phone"))
        assertTrue(text.contains("Last error: none"))
    }

    @Test
    fun everyParameterIsAFieldOfTheIssueForm() {
        val ids = formIds()
        assertTrue(ids.containsAll(listOf("what_happened", "steps", "diagnostics")))
        for (context in listOf(null, BugContext("load", "could not load"))) {
            for (name in parameters(BugReports.url(phone, emptyList(), context)).keys) {
                if (name !in listOf("template", "title")) assertTrue("$name is not a field of the form", name in ids)
            }
        }
    }

    @Test
    fun noSourceAddressOrTitleReachesTheAddress() {
        val context = BugContext(
            "load", "moonlit failed for Lantern Harbor at https://cdn.moon.example/v?t=1 from 10.0.0.2", title = "Lantern Harbor",
        )
        val rows = listOf(row("moonlit", "plugin", ProviderOrigin.ByHand), row("lantern", "plugin", ProviderOrigin.ByHand))
        val url = BugReports.url(phone, rows, context)
        val text = parameters(url).values.joinToString("\n")
        for (leak in listOf("moonlit", "Lantern", "moon.example", "10.0.0.2")) assertFalse("$leak in $text", text.contains(leak))
        assertTrue(text.contains("2 plugin sources"))
        assertEquals("Bug: load", parameters(url)["title"])
        assertTrue(url.toByteArray().size <= 7_500)
    }

    @Test
    fun aLongErrorIsTrimmedToFit() {
        val url = BugReports.url(phone, emptyList(), BugContext("load", "word ".repeat(5000)))
        assertTrue(url.toByteArray().size <= 7_500)
    }
}
