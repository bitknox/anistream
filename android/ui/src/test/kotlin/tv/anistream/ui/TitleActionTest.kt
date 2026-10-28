package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Test
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.TitleProgress

/** The amber action marks a dub and leaves a sub as it was. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class TitleActionTest {
    private val context = RuntimeEnvironment.getApplication()
    private fun entry(translation: Translation?, resume: ResumePoint? = null) = Entry(
        id = 900001u,
        title = "Stardust Wayfarer",
        progress = TitleProgress(done = 6u, next = 7u),
        resume = resume,
        watchedIn = translation,
    )

    @Test
    fun a_dub_is_marked_after_its_episode() {
        assertEquals("Play episode 7 · Dubbed", TitleAction.of(entry(Translation.DUB)).label(context))
        assertEquals("Resume episode 7 · Dubbed · 10:06", TitleAction.of(entry(Translation.DUB, ResumePoint(606.0, 0.42))).label(context))
    }

    @Test
    fun a_sub_or_an_unknown_one_is_not() {
        assertEquals("Play episode 7", TitleAction.of(entry(Translation.SUB)).label(context))
        assertEquals("Play episode 7", TitleAction.of(entry(null)).label(context))
        assertEquals("Resume episode 7 · 10:06", TitleAction.of(entry(Translation.SUB, ResumePoint(606.0, 0.42))).label(context))
    }
}
