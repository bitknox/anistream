package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.model.Entry

class ContinueQueueTest {
    private fun entry(id: UInt, nextEpisode: UInt? = null, nextAiringIn: Long? = null) = Entry(
        id = id, title = "Title $id", secondary = null, format = "TV", episodes = 12u, year = 2026u, score = null,
        studio = null, synopsis = "", coverUrl = null, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = nextEpisode, nextAiringIn = nextAiringIn, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = "Title $id", titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )

    @Test
    fun the_next_up_is_the_first_title_with_an_episode_out() {
        // The first title's next episode airs in an hour; the second's is out.
        val titles = listOf(entry(1u, nextEpisode = 1u, nextAiringIn = 3_600), entry(2u)).associateBy { it.id }
        val queue = ContinueQueue.of(listOf(1u, 2u), titles)
        assertEquals(1u, queue.lead?.id)
        assertEquals(2u, queue.nextUp?.id)
    }

    @Test
    fun nothing_is_next_up_when_nothing_is_out() {
        val titles = listOf(entry(1u, nextEpisode = 1u, nextAiringIn = 3_600)).associateBy { it.id }
        assertNull(ContinueQueue.of(listOf(1u), titles).nextUp)
        assertNull(ContinueQueue.of(emptyList(), titles).lead)
    }
}
