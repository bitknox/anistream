package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.TitleProgress

/**
 * A tracker's Library rows, loaded after sign-in, say 12 of 12. Continue is built from the same
 * cache, so they must not replace what this device watched: episode 7 at 10:06.
 */
class LibraryKeepsLocalProgressTest {
    private val local = TestEntries.entry(900001u).copy(
        progress = TitleProgress(done = 6u, next = 7u),
        resume = ResumePoint(position = 606.0, fraction = 0.42),
    )

    @Test
    fun a_library_list_leaves_continues_progress_and_resume_alone() {
        val titles = TitleCache()
        titles.merge(listOf(local), local = true)
        val tracked = local.copy(progress = TitleProgress(done = 12u, next = 1u), resume = null)
        titles.merge(listOf(tracked))
        val kept = titles.entries.value.getValue(900001u)
        assertEquals(TitleProgress(done = 6u, next = 7u), kept.progress)
        assertEquals(606.0, kept.resume?.position)
    }

    @Test
    fun continues_own_list_does_update_it() {
        val titles = TitleCache()
        titles.merge(listOf(local), local = true)
        titles.merge(listOf(local.copy(progress = TitleProgress(done = 7u, next = 8u), resume = null)), local = true)
        assertEquals(8u, titles.entries.value.getValue(900001u).progress?.next)
    }

    @Test
    fun a_title_only_the_tracker_knows_takes_the_trackers_progress() {
        val titles = TitleCache()
        titles.merge(listOf(local))
        assertEquals(7u, titles.entries.value.getValue(900001u).progress?.next)
    }
}
