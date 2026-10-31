package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppLinkTest {
    private val links = listOf(
        AppLink.Search,
        AppLink.Downloads,
        AppLink.Calendar,
        AppLink.Accounts,
        AppLink.Title(900001u),
        AppLink.Continue(900002u),
        AppLink.Play(900003u, "7"),
        // An episode number that is not whole, and one with characters a path could not hold.
        AppLink.Play(900003u, "12.5"),
        AppLink.Play(900003u, "SP 1/2"),
        AppLink.Download(42),
        // The Updated notification's, back where the update was taken.
        AppLink.Section(Destination.Library),
        AppLink.Section(Destination.Home),
    )

    @Test
    fun every_link_reads_back_from_its_fields() {
        for (link in links) assertEquals(link, AppLink.of(link.fields::get))
    }

    @Test
    fun the_fields_are_the_ones_the_static_shortcuts_name_and_each_link_has_a_key_of_its_own() {
        assertEquals(mapOf("open" to "search"), AppLink.Search.fields)
        assertEquals(mapOf("open" to "downloads"), AppLink.Downloads.fields)
        assertEquals("play/900001/7", AppLink.Play(900001u, "7").key)
        assertEquals(links.size, links.map { it.key }.distinct().size)
    }

    @Test
    fun the_widgets_links_name_the_calendar_and_accounts() {
        assertEquals(mapOf("open" to "calendar"), AppLink.Calendar.fields)
        assertEquals(mapOf("open" to "accounts"), AppLink.Accounts.fields)
    }

    @Test
    fun fields_that_are_not_a_link_are_left_alone() {
        val others = listOf(
            emptyMap(),
            mapOf("open" to "settings"),
            mapOf("open" to "title"),
            mapOf("open" to "title", "show" to "not-a-number"),
            mapOf("open" to "play", "show" to "900001"),
            mapOf("open" to "play", "show" to "900001", "episode" to " "),
            mapOf("open" to "download", "download" to "x"),
            mapOf("open" to "section"),
            mapOf("open" to "section", "section" to "settings"),
        )
        for (fields in others) assertNull(fields.toString(), AppLink.of(fields::get))
    }
}
