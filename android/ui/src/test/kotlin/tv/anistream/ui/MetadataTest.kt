package tv.anistream.ui

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.FuzzyDate
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.engine.model.Ranking
import tv.anistream.engine.model.RankingKind
import tv.anistream.engine.model.RelationKind
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.SourceMaterial
import tv.anistream.engine.model.TitleTag
import tv.anistream.testkit.Scene

/** The metadata rules of native-spec/metadata.md, on a fixture title from the watching scene. */
class MetadataTest {
    /** 900001's page as the engine sends it, for each test to change what it is about. */
    private val entry: Entry = Scene.named("watching").events
        .filterIsInstance<Event.Detail>()
        .first { it.v1.id == 900001u }
        .v1

    // The dates as the UK writes them, wherever the tests run.
    private val locale = Locale.getDefault()

    @Before
    fun setUp() = Locale.setDefault(Locale.UK)

    @After
    fun tearDown() = Locale.setDefault(locale)

    private fun date(year: Int, month: Int? = null, day: Int? = null) =
        FuzzyDate(year.toUShort(), month?.toUByte(), day?.toUByte())

    @Test
    fun the_other_forms_follow_romaji_english_native_without_the_shown_one() {
        val forms = entry.copy(title = "Stardust Wayfarer", titleRomaji = "Hoshikuzu no Tabibito", titleEnglish = "Stardust Wayfarer", titleNative = "星屑の旅人")
        assertEquals(listOf("Hoshikuzu no Tabibito", "星屑の旅人"), forms.otherForms())
        // A missing form, and one that only differs in case from the shown one, are left out.
        assertEquals(listOf("星屑の旅人"), forms.copy(title = "Hoshikuzu no Tabibito", titleEnglish = "HOSHIKUZU NO TABIBITO").otherForms())
        assertEquals(emptyList<String>(), forms.copy(titleRomaji = null, titleNative = null).otherForms())
    }

    @Test
    fun dates_say_as_much_as_anilist_knows() {
        assertEquals("20 Aug 2026", date(2026, 8, 20).words(Locale.UK))
        assertEquals("Aug 2026", date(2026, 8).words(Locale.UK))
        assertEquals("2026", date(2026).words(Locale.UK))
    }

    @Test
    fun the_airing_state_comes_from_the_dates() {
        val today = LocalDate.of(2026, 10, 21)
        val finished = entry.copy(startDate = date(2024, 4, 6), endDate = date(2024, 6, 22))
        assertEquals(AiringState("Aired", "6 Apr – 22 Jun 2024"), finished.airingState(today))
        val film = entry.copy(startDate = date(2025, 3, 1), endDate = date(2025, 3, 1))
        assertEquals("1 Mar 2025", film.airingState(today)?.value)
        val airing = entry.copy(startDate = date(2026, 8, 20), endDate = null)
        assertEquals(AiringState("Airing", "Since 20 Aug 2026"), airing.airingState(today))
        // An end still to come is airing too.
        assertEquals("Airing", airing.copy(endDate = date(2026, 12)).airingState(today)?.label)
        val upcoming = entry.copy(startDate = date(2027, 1), endDate = null)
        assertEquals(AiringState("Airs", "From Jan 2027"), upcoming.airingState(today))
        assertNull(entry.copy(startDate = null, endDate = null).airingState(today))
    }

    @Test
    fun the_phone_facts_line_has_the_source_the_studio_and_the_dates() {
        val facts = entry.copy(
            source = SourceMaterial.MANGA,
            studio = "Atelier Hotaru",
            startDate = date(2020, 8, 20),
            endDate = null,
        )
        assertEquals("Manga · Atelier Hotaru · Airing since 20 Aug 2020", facts.factsLine())
        assertEquals("Atelier Hotaru · Airing since 20 Aug 2020", facts.copy(source = SourceMaterial.OTHER).factsLine())
        assertNull(facts.copy(source = null, studio = null, startDate = null).factsLine())
    }

    @Test
    fun the_tags_line_is_the_top_three_without_spoilers() {
        val tags = listOf(
            TitleTag("Lamplighters", 70u, false, false, null),
            TitleTag("Travel", 95u, false, false, null),
            TitleTag("Twist", 99u, true, false, null),
            TitleTag("Found Family", 88u, false, false, null),
            TitleTag("Night", 40u, false, false, null),
        )
        val tagged = entry.copy(detail = entry.detail!!.copy(tags = tags))
        assertEquals("Travel · Found Family · Lamplighters", tagged.tagsLine())
        assertEquals(listOf("Twist"), tags.spoilers().map { it.name })
    }

    @Test
    fun an_adult_title_says_so_first_in_its_caps_line() {
        assertEquals("18+ · " + entry.metadataLine, entry.copy(adult = true).capsLine)
        assertEquals(entry.metadataLine, entry.copy(adult = false).capsLine)
    }

    @Test
    fun related_cards_leave_the_season_chain_to_the_picker() {
        fun linked(id: UInt, relation: RelationKind, format: String, year: Int) =
            LinkedTitle(
                id, "T$id", null, null, null, relation, format, coverUrl = null, coverColor = null, season = null,
                year = year.toUShort(), listStatus = null,
            )
        val relations = listOf(
            linked(1u, RelationKind.SEQUEL, "TV", 2025),
            linked(2u, RelationKind.SPIN_OFF, "ONA", 2026),
            linked(3u, RelationKind.SIDE_STORY, "OVA", 2026),
            linked(4u, RelationKind.SIDE_STORY, "MOVIE", 2025),
            linked(5u, RelationKind.PREQUEL, "MOVIE", 2023),
            linked(6u, RelationKind.PARENT, "TV", 2020),
            linked(7u, RelationKind.OTHER, "SPECIAL", 2024),
        )
        val related = entry.copy(detail = entry.detail!!.copy(relations = relations)).relatedCards()
        assertEquals(listOf(6u, 5u, 4u, 3u, 2u, 7u), related.map { it.id })
        assertEquals("Side story · Movie · 2025", related[2].cardLine())
        assertEquals("Related · Special · 2024", related.last().cardLine())
    }

    @Test
    fun the_picker_holds_the_franchises_seasons_as_the_engine_walked_them() {
        // The scene's page for the first season, whose sequel is the second: both, this one first.
        val family = TitleFamily.of(entry)
        assertEquals(listOf(900001u, 900004u), family.seasons.map { it.id })
        assertEquals(listOf(true, false), family.seasons.map { it.isThis })
        // The sequel's own page has the same seasons, numbered the same.
        assertEquals(listOf(false, true), TitleFamily.of(entry.copy(id = 900004u)).seasons.map { it.isThis })
        // While the engine is still walking them: no picker, rather than one it would renumber.
        val walking = entry.copy(detail = entry.detail!!.copy(seasons = null))
        assertEquals(emptyList<TitleFamily.Season>(), TitleFamily.of(walking).seasons)
    }

    @Test
    fun rankings_go_season_year_all_time_rated_before_popular() {
        val rankings = listOf(
            Ranking(31u, RankingKind.POPULAR, "", true, null, null),
            Ranking(19u, RankingKind.RATED, "", false, null, 2026u),
            Ranking(6u, RankingKind.POPULAR, "", false, Season.SUMMER, 2026u),
            Ranking(4u, RankingKind.RATED, "", false, Season.SUMMER, 2026u),
        ).inOrder()
        assertEquals(
            listOf("Highest rated, Summer 2026", "Most popular, Summer 2026", "Highest rated, 2026", "Most popular, all time"),
            rankings.map { it.words() },
        )
    }

    @Test
    fun a_broadcast_says_when_in_the_devices_own_time() {
        val zone = ZoneId.of("Europe/Copenhagen")
        val now = Instant.parse("2026-10-21T10:00:00Z") // a Wednesday
        fun airs(at: String, twentyFour: Boolean = true) =
            airsWords(Instant.parse(at), now, zone, twentyFourHour = twentyFour, locale = Locale.UK)
        assertEquals("today 16:30", airs("2026-10-21T14:30:00Z"))
        assertEquals("tomorrow 16:30", airs("2026-10-22T14:30:00Z"))
        assertEquals("Monday 16:30", airs("2026-10-26T15:30:00Z"))
        assertEquals("29 Oct, 16:30", airs("2026-10-29T15:30:00Z"))
        assertEquals("today 4:30 pm", airs("2026-10-21T14:30:00Z", twentyFour = false).lowercase(Locale.UK))
    }

    @Test
    fun the_synopsis_loses_its_spoiler_spans() {
        assertEquals(
            "A lamplighter walks the night roads. She keeps going.",
            synopsisWithoutSpoilers("A lamplighter walks the night roads. ~!She is the ferryman.!~ She keeps going."),
        )
    }
}
