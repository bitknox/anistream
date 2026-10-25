package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.TitleProgress

/** The amber action marks a dub and leaves a sub as it was. */
class TitleActionTest {
    private fun entry(translation: Translation?, resume: ResumePoint? = null) = Entry(
        id = 900001u,
        title = "Stardust Wayfarer",
        progress = TitleProgress(done = 6u, next = 7u),
        resume = resume,
        watchedIn = translation,
    )

    @Test
    fun a_dub_is_marked_after_its_episode() {
        assertEquals("Start E7 · Dub", TitleAction.of(entry(Translation.DUB)).label)
        assertEquals("Resume E7 · Dub · 10:06", TitleAction.of(entry(Translation.DUB, ResumePoint(606.0, 0.42))).label)
    }

    @Test
    fun a_sub_or_an_unknown_one_is_not() {
        assertEquals("Start E7", TitleAction.of(entry(Translation.SUB)).label)
        assertEquals("Start E7", TitleAction.of(entry(null)).label)
        assertEquals("Resume E7 · 10:06", TitleAction.of(entry(Translation.SUB, ResumePoint(606.0, 0.42))).label)
    }
}
