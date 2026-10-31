package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.HiddenTitle
import tv.anistream.engine.model.HomeAction
import tv.anistream.engine.model.HomeItem
import tv.anistream.engine.model.HomeSection
import tv.anistream.engine.model.HomeView
import tv.anistream.engine.model.Msg
import tv.anistream.engine.model.Playable
import tv.anistream.engine.model.SectionKind

class HomeModelTest {
    private fun item(id: UInt) = HomeItem(entry = TestEntries.entry(id), action = HomeAction.PLAY, playable = Playable.UNKNOWN)

    private fun view(failed: Boolean = false, vararg ids: UInt) = HomeView(
        sections = listOf(HomeSection(kind = SectionKind.THIS_SEASON, title = Msg.HomeThisSeason, items = ids.map(::item), total = ids.size.toUInt())),
        failed = failed,
    )

    @Test
    fun a_send_replaces_the_view_and_puts_its_titles_in_the_cache() {
        val home = HomeModel()
        val titles = TitleCache()
        assertNull(home.view.value)
        home.apply(view(false, 1u, 2u), titles)
        assertEquals(listOf(1u, 2u), home.view.value?.sections?.single()?.items?.map { it.entry.id })
        assertEquals("Title 2", titles.entries.value[2u]?.title)
    }

    @Test
    fun the_sources_check_is_forgotten_by_the_next_send() {
        val home = HomeModel()
        home.apply(view(false, 1u), TitleCache())
        home.playable(1u, Playable.NOT_FOUND)
        assertTrue(home.notFound(1u, Playable.UNKNOWN))
        home.apply(view(false, 1u), TitleCache())
        assertFalse(home.notFound(1u, Playable.UNKNOWN))
        assertTrue(home.notFound(1u, Playable.NOT_FOUND))
    }

    @Test
    fun each_failed_send_counts_once() {
        val home = HomeModel()
        home.apply(view(true, 1u), TitleCache())
        home.apply(view(false, 1u), TitleCache())
        home.apply(view(true, 1u), TitleCache())
        assertEquals(2, home.failures.value)
    }

    @Test
    fun without_leaves_out_the_titles_gone() {
        val section = view(false, 1u, 2u, 3u).sections.single()
        assertEquals(listOf(1u, 3u), section.without(setOf(2u)).map { it.entry.id })
    }

    @Test
    fun hidden_titles_are_kept_in_the_order_sent() {
        val home = HomeModel()
        home.hidden(listOf(HiddenTitle(TestEntries.entry(5u), untilAirs = true), HiddenTitle(TestEntries.entry(4u))), TitleCache())
        assertEquals(listOf(5u, 4u), home.hidden.value?.map { it.entry.id })
        assertTrue(home.hidden.value!!.first().untilAirs)
    }
}
