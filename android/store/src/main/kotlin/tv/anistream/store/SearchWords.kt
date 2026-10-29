package tv.anistream.store

import android.content.Context
import tv.anistream.engine.model.AiringStatus
import tv.anistream.engine.model.MediaFormat
import tv.anistream.engine.model.Season
import tv.anistream.engine.model.SearchSort
import tv.anistream.i18n.Strings

/** The words Search puts on the engine's sorts, formats, statuses, seasons and countries. */
object SearchWords {
    fun sort(context: Context, sort: SearchSort): String = when (sort) {
        SearchSort.RELEVANCE -> Strings.sortRelevance(context)
        SearchSort.TRENDING -> Strings.searchSortTrending(context)
        SearchSort.POPULARITY -> Strings.sortPopularity(context)
        SearchSort.SCORE -> Strings.sortScore(context)
        SearchSort.NEWEST -> Strings.searchSortNewest(context)
        SearchSort.OLDEST -> Strings.searchSortOldest(context)
        SearchSort.TITLE -> Strings.searchSortTitle(context)
        SearchSort.FAVOURITES -> Strings.searchSortFavourites(context)
    }

    fun format(context: Context, format: MediaFormat): String = when (format) {
        MediaFormat.TV -> Strings.formatTv(context)
        MediaFormat.TV_SHORT -> Strings.formatTvShort(context)
        MediaFormat.MOVIE -> Strings.formatMovie(context)
        MediaFormat.SPECIAL -> Strings.formatSpecial(context)
        MediaFormat.OVA -> Strings.formatOva(context)
        MediaFormat.ONA -> Strings.formatOna(context)
        MediaFormat.MUSIC -> Strings.formatMusic(context)
    }

    fun status(context: Context, status: AiringStatus): String = when (status) {
        AiringStatus.RELEASING -> Strings.mediaStatusAiring(context)
        AiringStatus.FINISHED -> Strings.mediaStatusAired(context)
        AiringStatus.NOT_YET_RELEASED -> Strings.mediaStatusUpcoming(context)
        AiringStatus.CANCELLED -> Strings.mediaStatusCancelled(context)
        AiringStatus.HIATUS -> Strings.mediaStatusHiatus(context)
    }

    fun season(context: Context, season: Season): String = when (season) {
        Season.WINTER -> Strings.seasonWinter(context)
        Season.SPRING -> Strings.seasonSpring(context)
        Season.SUMMER -> Strings.seasonSummer(context)
        Season.FALL -> Strings.seasonAutumn(context)
    }

    /** A country of origin by name; a code that is not one of the offered four stays as it is. */
    fun country(context: Context, code: String): String = when (code) {
        "JP" -> Strings.countryJapan(context)
        "CN" -> Strings.countryChina(context)
        "KR" -> Strings.countryKorea(context)
        "TW" -> Strings.countryTaiwan(context)
        else -> code
    }
}
