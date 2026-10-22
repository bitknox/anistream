package tv.anistream.widgets

import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.WidgetAiring
import tv.anistream.engine.model.WidgetContinue
import tv.anistream.engine.model.WidgetData
import tv.anistream.engine.model.WidgetNextAiring
import tv.anistream.testkit.FixtureArt

/**
 * The canvas's fixtures, as the widget boards draw them: Tuesday 20 October 2026 at 19:30, Mika's
 * Continue, and MacCalendar's day with two shows from outside the list, Kanade Ropeway and
 * Tsurara Tantei, for Airing today to leave unmarked.
 */
object Fixtures {
    val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

    fun at(day: Int, hour: Int, minute: Int): Instant = LocalDateTime.of(2026, 10, day, hour, minute).atZone(ZONE).toInstant()

    val NOW: Instant = at(20, 19, 30)

    /** The fixture titles' art, as the fake AniList serves it. */
    fun banner(id: UInt) = "${FixtureArt.HOST}banner/$id.jpg"
    fun poster(id: UInt) = "${FixtureArt.HOST}cover/$id-xl.jpg"

    const val HOSHIKUZU = 900001u
    const val KUMO = 900002u
    const val HAGANE = 900005u

    val continuing = listOf(
        WidgetContinue(
            id = HOSHIKUZU, title = "Hoshikuzu no Tabibito", episode = 7u, watched = 6u, episodes = 12u, fraction = 0.42,
            coverUrl = poster(HOSHIKUZU), resume = true, positionSecs = 600u, durationSecs = 1440u, minutesLeft = 14u,
            bannerUrl = banner(HOSHIKUZU), coverColor = "#1a1f3d",
        ),
        WidgetContinue(
            id = HAGANE, title = "Hagane no Niwashi", episode = 2u, watched = 1u, episodes = 10u, fraction = 0.75,
            coverUrl = poster(HAGANE), resume = true, positionSecs = 1080u, durationSecs = 1440u, minutesLeft = 6u,
            bannerUrl = banner(HAGANE), coverColor = "#243b2c",
        ),
        WidgetContinue(
            id = KUMO, title = "Kumo no Ue no Toshokan", episode = 4u, watched = 3u, episodes = 24u,
            coverUrl = poster(KUMO), resume = false, durationSecs = 1440u, minutesLeft = 24u, coverColor = "#c9d6e3",
        ),
    )

    private fun airing(id: UInt, title: String, episode: UInt, hour: Int, minute: Int, onList: Boolean, watched: Boolean = false, translation: Translation? = null, day: Int = 20) =
        WidgetAiring(id = id, title = title, episode = episode, airsAt = at(day, hour, minute).epochSecond, onList = onList, watched = watched, translation = translation)

    val today = listOf(
        airing(910001u, "Shiosai Shashinbu", 3u, 13, 0, onList = true, watched = true),
        airing(910002u, "Natsukaze Relay", 9u, 17, 30, onList = true),
        airing(910003u, "Kanade Ropeway", 5u, 19, 0, onList = false),
        airing(910002u, "Natsukaze Relay", 9u, 21, 0, onList = true, translation = Translation.DUB),
        airing(910004u, "Ame no Kajiya", 4u, 22, 0, onList = true),
        airing(910005u, "Tsurara Tantei", 2u, 22, 30, onList = false),
        airing(910006u, "Kazamachi Shokudo", 8u, 23, 45, onList = true),
    )

    val nextDays = listOf(
        airing(HOSHIKUZU, "Hoshikuzu no Tabibito", 10u, 16, 30, onList = true, day = 22),
    )

    val next = WidgetNextAiring(id = HOSHIKUZU, title = "Hoshikuzu no Tabibito", episode = 10u, airsAt = at(22, 16, 30).epochSecond)

    /** Mika's record, made a moment ago. */
    fun record(
        generatedAt: Instant = NOW.minusSeconds(600),
        signedIn: Boolean = true,
        continuing: List<WidgetContinue> = this.continuing,
        today: List<WidgetAiring> = this.today,
        nextDays: List<WidgetAiring> = this.nextDays,
        next: WidgetNextAiring? = this.next,
    ) = WidgetData(
        version = 2u,
        generatedAt = generatedAt.epochSecond,
        continueWatching = continuing,
        airingToday = today,
        signedIn = signedIn,
        tracker = if (signedIn) "anilist" else null,
        nextAiring = next,
        nextDays = nextDays,
    )

    /** The same, as a record of the first shape says it: no sign-in, no week, no time left. */
    fun firstShape(generatedAt: Instant = NOW.minusSeconds(600)) = WidgetData(
        version = 1u,
        generatedAt = generatedAt.epochSecond,
        continueWatching = continuing.map { it.copy(resume = false, minutesLeft = null, bannerUrl = null, coverColor = null, positionSecs = null, durationSecs = null) },
        airingToday = today.map { it.copy(watched = false, translation = null) },
    )
}
