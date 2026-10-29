package tv.anistream.ui

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Test
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.RememberedSubtitles
import tv.anistream.engine.model.RememberedTrack
import tv.anistream.engine.model.ShowSettingKey
import tv.anistream.engine.model.ShowSettingRow

/** How the show's sheet words what the engine says of it. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ShowSettingsTest {
    private val context = RuntimeEnvironment.getApplication()
    private fun track(language: String?, title: String) = RememberedTrack(language, title, forced = false, hearingImpaired = false)

    @Test
    fun a_row_that_follows_settings_says_so_and_what_that_is() {
        assertEquals("Use default · On", defaultWords(context, "on"))
        assertEquals("Use default · Off", defaultWords(context, "off"))
        assertEquals("Use default · On", defaultWords(context, "Use Default · On"))
    }

    @Test
    fun a_show_not_being_watched_is_off_for_new_episodes_until_it_turns_them_on() {
        val row = ShowSettingRow(ShowSettingKey.NOTIFY_NEW_EPISODES, own = null, value = "on", default = "on")
        assertEquals("Use default · On", notifyDefault(context, row, watching = true))
        assertEquals("Off · not watching", notifyDefault(context, row, watching = false))
        // With Settings off, every show is off, watched or not.
        val off = row.copy(value = "off", default = "off")
        assertEquals("Use default · Off", notifyDefault(context, off, watching = false))
    }

    @Test
    fun a_show_is_being_watched_on_the_list_as_watching_or_under_way_here() {
        val planned = entry(900002u, LibrarySegment.PLANNING)
        assertEquals(false, beingWatched(planned, continuing = emptyList()))
        assertEquals(true, beingWatched(planned, continuing = listOf(900002u)))
        assertEquals(true, beingWatched(entry(900001u, LibrarySegment.WATCHING), continuing = emptyList()))
        assertEquals(false, beingWatched(entry(900003u, null), continuing = listOf(900001u)))
    }

    private fun entry(id: UInt, status: LibrarySegment?) = Entry(
        id = id, title = "T$id", secondary = null, format = "TV", episodes = 12u, year = 2026u, score = null,
        studio = null, synopsis = "", coverUrl = null, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = status,
        titleRomaji = "T$id", titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )

    @Test
    fun audio_is_the_language_a_translation_plays() {
        assertEquals("Japanese", audioWords("sub"))
        assertEquals("English", audioWords("dub"))
        assertEquals("English", audioWords("dub · sub"))
    }

    @Test
    fun a_remembered_track_is_its_language_and_what_its_title_adds() {
        assertEquals("Off", subtitleWords(context, RememberedSubtitles.Off))
        assertEquals("English (Hotaru)", subtitleWords(context, RememberedSubtitles.Track(track("eng", "hotaru"))))
        assertEquals("English (Signs)", trackWords(context, track("eng", "english signs")))
        assertEquals("English", trackWords(context, track("eng", "english")))
        assertEquals("Signs", trackWords(context, track(null, "signs")))
        assertEquals("Unnamed track", trackWords(context, track(null, "")))
    }
}
