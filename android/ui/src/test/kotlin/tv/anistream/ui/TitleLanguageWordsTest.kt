package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.TitleLanguage

/** How Settings names a title language: with its capital, in the preview's note as in the row's line. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class TitleLanguageWordsTest {
    private val context = RuntimeEnvironment.getApplication()
    @Test
    fun a_title_language_is_named_with_its_capital_in_every_sentence() {
        val note = TitleLanguageWords.note(context, TitleChoice.AniList, signedIn = true, viewer = TitleLanguage.ENGLISH, examples = emptyList())
        assertTrue(note, note.startsWith("Follows your AniList account, which is set to English. "))
        assertEquals("Romaji, from your AniList account", TitleLanguageWords.line(context, TitleChoice.AniList, signedIn = true, viewer = TitleLanguage.ROMAJI))
        TitleLanguage.entries.forEach { assertTrue(TitleLanguageWords.name(context, it), TitleLanguageWords.name(context, it).first().isUpperCase()) }
    }
}
