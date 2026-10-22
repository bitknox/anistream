package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.TitleLanguage

/** How Settings names a title language: with its capital, in the preview's note as in the row's line. */
class TitleLanguageWordsTest {
    @Test
    fun a_title_language_is_named_with_its_capital_in_every_sentence() {
        val note = TitleLanguageWords.note(TitleChoice.AniList, signedIn = true, viewer = TitleLanguage.ENGLISH, examples = emptyList())
        assertTrue(note, note.startsWith("Follows your AniList account, which is set to English. "))
        assertEquals("Romaji, from your AniList account", TitleLanguageWords.line(TitleChoice.AniList, signedIn = true, viewer = TitleLanguage.ROMAJI))
        TitleLanguage.entries.forEach { assertTrue(TitleLanguageWords.name(it), TitleLanguageWords.name(it).first().isUpperCase()) }
    }
}
