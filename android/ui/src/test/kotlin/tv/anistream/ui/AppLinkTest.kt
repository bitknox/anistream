package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppLinkTest {
    @Test
    fun every_link_reads_back_from_its_uri() {
        val links = listOf(
            AppLink.Search,
            AppLink.Downloads,
            AppLink.Title(900001u),
            AppLink.Continue(900002u),
            AppLink.Play(900003u, "7"),
            // An episode number that is not whole, and one with a character a path cannot hold.
            AppLink.Play(900003u, "12.5"),
            AppLink.Play(900003u, "SP 1/2"),
        )
        for (link in links) assertEquals(link, AppLink.parse(link.uri))
    }

    @Test
    fun the_uris_are_the_ones_the_static_shortcuts_name() {
        assertEquals("anistream://open/search", AppLink.Search.uri)
        assertEquals("anistream://open/downloads", AppLink.Downloads.uri)
        assertEquals("anistream://open/continue/900001", AppLink.Continue(900001u).uri)
        assertEquals("anistream://open/play/900001/7", AppLink.Play(900001u, "7").uri)
    }

    @Test
    fun links_that_are_not_the_apps_own_are_left_alone() {
        val others = listOf(
            null,
            "",
            "anistream://auth/anilist#access_token=x",
            "anistream://join?link=https%3A%2F%2Fplugins.example%2Fjoin%2Fabc",
            "anistream://plugins.example/join/abc#key",
            "https://anistream.tv/open/search",
            "anistream://open/",
            "anistream://open/settings",
            "anistream://open/title/",
            "anistream://open/title/not-a-number",
            "anistream://open/title/900001/extra",
            "anistream://open/play/900001",
            "anistream://open/search/more",
        )
        for (uri in others) assertNull(uri, AppLink.parse(uri))
    }
}
