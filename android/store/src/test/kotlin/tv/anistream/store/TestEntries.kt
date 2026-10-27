package tv.anistream.store

import tv.anistream.engine.model.Entry

/** A bare title for tests that need one. */
object TestEntries {
    fun entry(id: UInt): Entry = Entry(
        id = id, title = "Title $id", secondary = null, format = "TV", episodes = 13u, year = 2026u, score = null,
        studio = null, synopsis = "", coverUrl = null, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = "Title $id", titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )
}
