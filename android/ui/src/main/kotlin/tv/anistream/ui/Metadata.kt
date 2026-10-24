// AniList's metadata as the screens say it, by native-spec/metadata.md: the other forms of a
// title, the facts, the tags, the airing state, related titles and the next unaired episode.

package tv.anistream.ui

import android.text.format.DateFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.FuzzyDate
import tv.anistream.engine.model.LinkedTitle
import tv.anistream.engine.model.Ranking
import tv.anistream.engine.model.RankingKind
import tv.anistream.engine.model.RelationKind
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.SourceMaterial
import tv.anistream.engine.model.TitleTag
import tv.anistream.store.AppClock

/**
 * The title's other forms, for the line under it: Romaji, English, Native in that order, leaving
 * out the shown one, any missing one and any that matches the shown one.
 */
fun Entry.otherForms(): List<String> =
    listOfNotNull(titleRomaji, titleEnglish, titleNative)
        .filter { it.isNotBlank() && !it.equals(title, ignoreCase = true) }
        .distinctBy { it.lowercase(Locale.ROOT) }

/** "Summer", from AniList's season. */
fun Season.label(): String = name.lowercase(Locale.ROOT).replaceFirstChar { it.uppercase() }

/** What a title was adapted from, in words; `null` for Other, which is left out. */
fun SourceMaterial.label(): String? = when (this) {
    SourceMaterial.ORIGINAL -> "Original story"
    SourceMaterial.MANGA -> "Manga"
    SourceMaterial.LIGHT_NOVEL -> "Light novel"
    SourceMaterial.WEB_NOVEL -> "Web novel"
    SourceMaterial.VISUAL_NOVEL -> "Visual novel"
    SourceMaterial.VIDEO_GAME -> "Video game"
    SourceMaterial.NOVEL -> "Novel"
    SourceMaterial.DOUJINSHI -> "Doujinshi"
    SourceMaterial.ANIME -> "Anime"
    SourceMaterial.LIVE_ACTION -> "Live action"
    SourceMaterial.GAME -> "Game"
    SourceMaterial.COMIC -> "Comic"
    SourceMaterial.MULTIMEDIA_PROJECT -> "Multimedia project"
    SourceMaterial.PICTURE_BOOK -> "Picture book"
    SourceMaterial.OTHER -> null
}

/** "20 Aug 2026", "Aug 2026" or "2026", as much of the date as AniList knows. */
fun FuzzyDate.words(locale: Locale = Locale.getDefault()): String {
    val month = month?.toInt()
    val day = day?.toInt()
    if (month == null) return year.toString()
    val short = java.time.Month.of(month).getDisplayName(java.time.format.TextStyle.SHORT, locale)
    return if (day == null) "$short $year" else "$day $short $year"
}

private fun FuzzyDate.earliest(): LocalDate = LocalDate.of(year.toInt(), month?.toInt() ?: 1, day?.toInt() ?: 1)

private fun FuzzyDate.latest(): LocalDate {
    val m = month?.toInt() ?: 12
    val first = LocalDate.of(year.toInt(), m, 1)
    return day?.let { first.withDayOfMonth(it.toInt()) } ?: first.withDayOfMonth(first.lengthOfMonth())
}

/** Where a title stands by its dates, with the label and the words the facts give it. */
data class AiringState(val label: String, val value: String)

/**
 * Aired, Airing or Airs, from the dates until the engine carries a status: finished once the end
 * has passed, airing once the start has, upcoming before; nothing with no dates at all.
 */
fun Entry.airingState(today: LocalDate = LocalDate.ofInstant(AppClock.now(), AppClock.zone)): AiringState? {
    val start = startDate ?: return null
    val end = endDate
    return when {
        end != null && end.latest() < today -> {
            val value = if (end.year == start.year && end.month == start.month && end.day == start.day) {
                start.words()
            } else if (start.year == end.year && start.day != null && end.day != null && start.month != null) {
                // "4 Apr to 22 Jun 2024": the year once when both ends share it.
                "${start.words().removeSuffix(" ${start.year}")} to ${end.words()}"
            } else {
                "${start.words()} to ${end.words()}"
            }
            AiringState("Aired", value)
        }
        start.earliest() <= today -> AiringState("Airing", "Since ${start.words()}")
        else -> AiringState("Airs", "From ${start.words()}")
    }
}

/** "Summer 2026", or "2026" with only a year. */
fun Entry.seasonWords(): String? = when {
    season != null && year != null -> "${season!!.label()} $year"
    year != null -> year.toString()
    else -> null
}

/** "Hikari Stream", or "Hikari Stream and 1 more". */
fun Entry.alsoOn(): String? = when (availableOn.size) {
    0 -> null
    1 -> availableOn.first()
    else -> "${availableOn.first()} and ${availableOn.size - 1} more"
}

/**
 * The phone's facts line: what it is based on, the studio and the airing state, as one line that
 * is cut at the end, so the dates go first when space runs out.
 */
fun Entry.factsLine(): String? = listOfNotNull(
    source?.label(),
    studio,
    airingState()?.let { "${it.label} ${it.value.replaceFirstChar { c -> c.lowercase() }}" },
).joinToString(" · ").ifEmpty { null }

/** The tags without spoilers, by rank. */
fun List<TitleTag>.plain(): List<TitleTag> =
    filter { !it.mediaSpoiler && !it.generalSpoiler }.sortedBy { it.rank?.let { r -> -r.toInt() } ?: 0 }

/** The spoiler tags, by rank, which show only when asked for. */
fun List<TitleTag>.spoilers(): List<TitleTag> =
    filter { it.mediaSpoiler || it.generalSpoiler }.sortedBy { it.rank?.let { r -> -r.toInt() } ?: 0 }

/** The top three tags without spoilers, for the tags line. */
fun Entry.tagsLine(): String? =
    detail?.tags?.plain()?.take(3)?.joinToString(" · ") { it.name }?.ifEmpty { null }

/** The caps line: "18+ · TV · 12 EP · SPRING 2024 · 82%". */
val Entry.capsLine: String
    get() {
        val base = metadataLine
        return if (adult) listOf("18+", base).filter { it.isNotEmpty() }.joinToString(" · ") else base
    }

/** AniList's synopsis without its inline spoiler spans, which the app offers no reveal for yet. */
fun synopsisWithoutSpoilers(text: String): String =
    text.replace(Regex("~!.*?!~", RegexOption.DOT_MATCHES_ALL), "").replace(Regex("\\s{2,}"), " ").trim()

/** A relation in words. */
fun RelationKind.words(): String = when (this) {
    RelationKind.PREQUEL -> "Prequel"
    RelationKind.SEQUEL -> "Sequel"
    RelationKind.PARENT -> "Parent story"
    RelationKind.SIDE_STORY -> "Side story"
    RelationKind.SPIN_OFF -> "Spin-off"
    RelationKind.ALTERNATIVE -> "Alternative"
    RelationKind.SUMMARY -> "Summary"
    RelationKind.COMPILATION -> "Compilation"
    RelationKind.CONTAINS -> "Contains"
    RelationKind.SAME_UNIVERSE -> "Same universe"
    RelationKind.CHARACTER -> "Shares characters"
    RelationKind.ADAPTATION -> "Adaptation"
    RelationKind.SOURCE -> "Source"
    RelationKind.OTHER -> "Related"
}

private val SERIES = setOf("TV", "TV_SHORT", "TV SHORT", "ONA")

/**
 * The Related rail: every relation but the TV prequels and sequels the season picker holds. The
 * parent story first, then prequels and sequels that are not TV, then the rest in the spec's
 * order, each relation by year.
 */
fun Entry.relatedCards(): List<LinkedTitle> {
    val relations = detail?.relations ?: return emptyList()
    // The parent story, then the prequels and sequels the picker leaves out, then the rest.
    val order = listOf(
        RelationKind.SIDE_STORY, RelationKind.SPIN_OFF, RelationKind.ALTERNATIVE, RelationKind.SUMMARY,
        RelationKind.COMPILATION, RelationKind.CONTAINS, RelationKind.SAME_UNIVERSE, RelationKind.CHARACTER,
        RelationKind.ADAPTATION, RelationKind.SOURCE, RelationKind.OTHER,
    )
    fun rank(kind: RelationKind?): Int = when (kind) {
        RelationKind.PARENT -> 0
        RelationKind.PREQUEL, RelationKind.SEQUEL -> 1
        else -> 2 + order.indexOf(kind ?: RelationKind.OTHER)
    }
    return relations
        .filterNot { (it.relation == RelationKind.PREQUEL || it.relation == RelationKind.SEQUEL) && it.format in SERIES }
        .sortedWith(compareBy<LinkedTitle> { rank(it.relation) }.thenBy { it.year?.toInt() ?: Int.MAX_VALUE })
}

/** "Side story · Movie · 2025". */
fun LinkedTitle.cardLine(): String =
    listOfNotNull((relation ?: RelationKind.OTHER).words(), formatLabel(format), year?.toString()).joinToString(" · ")

/** "Movie · 2025", for a recommendation. */
fun LinkedTitle.recommendationLine(): String =
    listOfNotNull(formatLabel(format), year?.toString()).joinToString(" · ")

/**
 * A ranking in words: "Highest rated, Summer 2026", "Most popular, all time". The number goes
 * beside it, set apart.
 */
fun Ranking.words(): String {
    val kind = if (kind == RankingKind.RATED) "Highest rated" else "Most popular"
    val span = when {
        allTime -> "all time"
        season != null && year != null -> "${season!!.label()} $year"
        year != null -> year.toString()
        else -> "all time"
    }
    return "$kind, $span"
}

/** Season before year before all time, and rated before popular within each. */
fun List<Ranking>.inOrder(): List<Ranking> = sortedWith(
    compareBy<Ranking> {
        when {
            it.season != null && !it.allTime -> 0
            !it.allTime -> 1
            else -> 2
        }
    }.thenBy { if (it.kind == RankingKind.RATED) 0 else 1 },
)

/** The next episode still to air, from the schedule: the first broadcast later than now. */
data class NextAiring(val number: UInt, val airsAt: Long, val title: String?)

fun Entry.nextUnaired(rows: List<EpisodeRow>?, now: Long = AppClock.now().epochSecond): NextAiring? {
    val next = detail?.schedule?.filter { it.airsAt > now }?.minByOrNull { it.airsAt } ?: return null
    val title = rows?.firstOrNull { it.number == next.episode.toString() }?.title
    return NextAiring(next.episode, next.airsAt, title)
}

/**
 * When a broadcast airs: "today 16:30", "tomorrow 16:30", "Thursday 16:30" within six days, and
 * "29 Oct, 16:30" later, in the device's own time format.
 */
fun airsWords(
    at: Instant,
    now: Instant = AppClock.now(),
    zone: ZoneId = AppClock.zone,
    twentyFourHour: Boolean = true,
    locale: Locale = Locale.getDefault(),
): String {
    val time = DateTimeFormatter.ofPattern(if (twentyFourHour) "HH:mm" else "h:mm a", locale).format(at.atZone(zone))
    val day = LocalDate.ofInstant(at, zone)
    val today = LocalDate.ofInstant(now, zone)
    val days = ChronoUnit.DAYS.between(today, day)
    return when {
        days == 0L -> "today $time"
        days == 1L -> "tomorrow $time"
        days in 2..6 -> "${day.dayOfWeek.getDisplayName(java.time.format.TextStyle.FULL, locale)} $time"
        else -> "${DateTimeFormatter.ofPattern("d MMM", locale).format(day)}, $time"
    }
}

/** [airsWords] in the device's own time format. */
fun airsWords(context: android.content.Context, at: Instant): String =
    airsWords(at, twentyFourHour = DateFormat.is24HourFormat(context))

/** The flat rect a cover or character waits in: #1F2440 until a cover's own colour is known. */
val NoArtPlate = androidx.compose.ui.graphics.Color(0xFF1F2440)

/**
 * A linked title's plate while its cover loads: AniList's cover colour at 40 % over the ground,
 * #161A2E (T8), or [NoArtPlate] without one.
 */
fun LinkedTitle.plate(): androidx.compose.ui.graphics.Color {
    val hex = coverColor?.removePrefix("#")?.takeIf { it.length == 6 }?.toLongOrNull(16) ?: return NoArtPlate
    val cover = androidx.compose.ui.graphics.Color(0xFF000000 or hex)
    return androidx.compose.ui.graphics.lerp(androidx.compose.ui.graphics.Color(0xFF161A2E), cover, 0.4f)
}
