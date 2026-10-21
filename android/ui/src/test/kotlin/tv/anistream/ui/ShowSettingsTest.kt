package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.model.RememberedSubtitles
import tv.anistream.engine.model.RememberedTrack

/** How the show's sheet words what the engine says of it. */
class ShowSettingsTest {
    private fun track(language: String?, title: String) = RememberedTrack(language, title, forced = false, hearingImpaired = false)

    @Test
    fun a_row_that_follows_settings_says_so_and_what_that_is() {
        assertEquals("Use default · On", defaultWords("on"))
        assertEquals("Use default · Off", defaultWords("off"))
        assertEquals("Use default · On", defaultWords("Use Default · On"))
    }

    @Test
    fun audio_is_the_language_a_translation_plays() {
        assertEquals("Japanese", audioWords("sub"))
        assertEquals("English", audioWords("dub"))
        assertEquals("English", audioWords("dub · sub"))
    }

    @Test
    fun a_remembered_track_is_its_language_and_what_its_title_adds() {
        assertEquals("Off", subtitleWords(RememberedSubtitles.Off))
        assertEquals("English (Hotaru)", subtitleWords(RememberedSubtitles.Track(track("eng", "hotaru"))))
        assertEquals("English (Signs)", trackWords(track("eng", "english signs")))
        assertEquals("English", trackWords(track("eng", "english")))
        assertEquals("Signs", trackWords(track(null, "signs")))
        assertEquals("Unnamed track", trackWords(track(null, "")))
    }
}
