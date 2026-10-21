package tv.anistream.store

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.anistream_model.Entry
import uniffi.anistream_model.EpisodeRow
import uniffi.anistream_model.TitleProgress

/** The wide art a title's page and hero show, by the Apple app's rule. */
class HeroArtTest {
    private fun entry(banner: String? = null, done: UInt? = null) = Entry(
        id = 900005u, title = "The Steel Gardener", secondary = "Hagane no Niwashi", format = "ONA", episodes = 10u,
        year = 2026u, score = null, studio = null, synopsis = "", coverUrl = null, bannerUrl = banner,
        genres = emptyList(), availableOn = emptyList(), progress = done?.let { TitleProgress(it, it + 1u) },
        airingIn = null, airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null,
        resume = null, related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = null, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
    )

    private fun row(number: Int, still: String?, completed: Boolean = false) = EpisodeRow(
        number = number.toString(), title = null, thumbnail = still, description = null, durationSecs = null,
        watched = if (completed) 1.0 else 0.0, completed = completed, kind = null, skippable = false, airsAt = null,
    )

    @Test
    fun the_banner_wins() {
        assertEquals(HeroArt.Banner("b"), HeroArt.of(entry(banner = "b"), null))
    }

    @Test
    fun without_one_it_waits_for_the_episodes_then_takes_the_latest_seen_still() {
        assertEquals(HeroArt.Waiting, HeroArt.of(entry(), null))
        val rows = listOf(row(1, "s1"), row(2, "s2"), row(3, "s3"), row(4, "s4"))
        assertEquals(HeroArt.Still("s2"), HeroArt.of(entry(done = 2u), rows))
        // Never a still from an episode not yet seen past the first.
        assertEquals(HeroArt.Still("s1"), HeroArt.of(entry(), rows))
        assertEquals(HeroArt.Compact, HeroArt.of(entry(), listOf(row(1, null), row(2, null))))
    }
}
