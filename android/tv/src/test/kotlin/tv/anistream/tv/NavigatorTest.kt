package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.tv.shell.Destination
import tv.anistream.tv.shell.Page
import tv.anistream.tv.shell.TvNavigator

class NavigatorTest {
    @Test
    fun title_pages_stack_and_pop_forgetting_their_state() {
        val forgotten = mutableListOf<String>()
        val nav = TvNavigator().apply { forget = { forgotten += it } }
        nav.openTitle(900001u)
        nav.openTitle(900002u)
        assertEquals(900002u, (nav.top as Page.Title).id)
        assertTrue(nav.pop())
        assertEquals(900001u, (nav.top as Page.Title).id)
        assertTrue(nav.pop())
        assertFalse(nav.pop())
        assertEquals(Destination.Continue.name, nav.screenKey)
        assertEquals(2, forgotten.size)
    }

    @Test
    fun another_season_changes_the_title_page_in_place_so_back_returns_to_the_opener() {
        val forgotten = mutableListOf<String>()
        val nav = TvNavigator().apply { forget = { forgotten += it } }
        nav.openTitle(920001u)
        val page = nav.top as Page.Title
        nav.switchTitle(920002u)
        nav.switchTitle(920003u)
        // The same page, its saved state kept: no new screen to fade to.
        assertSame(page, nav.top)
        assertEquals(920003u, page.id)
        assertTrue(forgotten.isEmpty())
        assertTrue(nav.pop())
        assertNull(nav.top)
    }

    @Test
    fun another_destination_starts_afresh_and_search_forgets_its_text() {
        val nav = TvNavigator(Destination.Search)
        nav.typeSearch("hoshi")
        nav.openTitle(900001u)
        nav.go(Destination.Library)
        assertNull(nav.top)
        assertEquals(Destination.Library, nav.destination)
        assertEquals("", nav.searchText)
    }

    @Test
    fun a_phone_types_at_the_cursor_and_its_whole_text_replaces_only_its_own() {
        val nav = TvNavigator(Destination.Search)
        nav.typeSearch("kiriya")
        // Each text is the phone's whole field: it goes after what the TV typed.
        nav.phoneSearch("p")
        nav.phoneSearch("pass")
        nav.phoneSearch("pass")
        assertEquals("kiriya pass", nav.searchText)
        assertEquals(11, nav.searchCursor)
        nav.phoneDone()
        // The system keyboard moved the cursor to the start: the next typing goes there.
        nav.typeSearch(nav.searchText, cursor = 0)
        nav.phoneSearch("rain")
        assertEquals("rain kiriya pass", nav.searchText)
        nav.phoneSearch("")
        assertEquals("kiriya pass", nav.searchText)
    }

    @Test
    fun the_same_destination_keeps_where_focus_was() {
        val nav = TvNavigator(Destination.Search)
        nav.typeSearch("hoshi")
        nav.openTitle(900001u)
        nav.go(Destination.Search)
        assertNull(nav.top)
        assertEquals("hoshi", nav.searchText)
    }

    @Test
    fun destinations_are_named_as_the_fixtures_extras_name_them() {
        assertEquals(Destination.Seasonal, Destination.named("seasonal"))
        assertNull(Destination.named("downloads"))
    }
}
