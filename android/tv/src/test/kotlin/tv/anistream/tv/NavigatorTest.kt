package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
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
    fun another_destination_starts_afresh_and_search_forgets_its_text() {
        val nav = TvNavigator(Destination.Search)
        nav.searchText = "hoshi"
        nav.openTitle(900001u)
        nav.go(Destination.Library)
        assertNull(nav.top)
        assertEquals(Destination.Library, nav.destination)
        assertEquals("", nav.searchText)
    }

    @Test
    fun the_same_destination_keeps_where_focus_was() {
        val nav = TvNavigator(Destination.Search)
        nav.searchText = "hoshi"
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
