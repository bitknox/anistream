package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.TitleProgress
import tv.anistream.store.ContinueQueue
import tv.anistream.tv.home.WatchNextCard
import tv.anistream.tv.home.WatchNextKind
import tv.anistream.tv.home.parsePlayLink
import tv.anistream.tv.home.playLink

/** The Watch Next row from the Continue queue, under Google's rules, and the links it opens. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class WatchNextTest {
    private fun entry(id: UInt, next: UInt = 4u, resume: ResumePoint? = null) = Entry(
        id = id,
        title = "Title $id",
        bannerUrl = "https://art.example/banner/$id.jpg",
        progress = TitleProgress(done = next - 1u, next = next),
        resume = resume,
    )

    private fun cards(vararg entries: Entry, episodes: Map<UInt, List<EpisodeRow>> = emptyMap()) =
        WatchNextCard.of(ContinueQueue.of(entries.map { it.id }, entries.associateBy { it.id }), episodes)

    @Test
    fun an_episode_left_part_way_continues_where_it_stopped() {
        val card = cards(entry(1u, resume = ResumePoint(position = 600.0, fraction = 0.5))).single()
        assertEquals(WatchNextKind.Continue, card.kind)
        assertEquals(600_000L, card.positionMillis)
        assertEquals(1_200_000L, card.durationMillis)
        assertEquals("Episode 4", card.description)
    }

    @Test
    fun under_two_minutes_in_or_at_the_ending_it_is_the_next_episode() {
        val early = cards(entry(1u, resume = ResumePoint(position = 60.0, fraction = 0.05))).single()
        assertEquals(WatchNextKind.Next, early.kind)
        assertEquals(0L, early.positionMillis)
        val ending = cards(entry(2u, resume = ResumePoint(position = 1_300.0, fraction = 0.9))).single()
        assertEquals(WatchNextKind.Next, ending.kind)
    }

    @Test
    fun the_episode_title_follows_its_number_once_known() {
        val rows = mapOf(1u to listOf(EpisodeRow(number = "4", title = "The Lantern Road", durationSecs = 1_440uL)))
        val card = cards(entry(1u), episodes = rows).single()
        assertEquals(WatchNextKind.Next, card.kind)
        assertEquals("Episode 4 · The Lantern Road", card.description)
        assertEquals(1_440_000L, card.durationMillis)
    }

    @Test
    fun a_play_link_reads_back() {
        assertEquals(900_001u to 7u, parsePlayLink(playLink(900_001u, 7u)))
        assertNull(parsePlayLink("https://anistream.example/play/1/2"))
        assertNull(parsePlayLink("anistream-tv://play/1"))
        assertNull(parsePlayLink(null))
    }
}
