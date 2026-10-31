// The links the system opens the app on from outside its screens: launcher shortcuts,
// notifications and the home-screen widgets. Each is an explicit intent to the activity with the
// link's fields in its extras, not a URL, which the activity hands to the frame to follow through
// the app's own navigation.

package tv.anistream.ui

import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.DownloadState
import tv.anistream.store.AppStore
import tv.anistream.store.ContinueItem

/** Where a link from outside the screens asks the app to go. */
sealed interface AppLink {
    /** Search, with its field open for typing. */
    data object Search : AppLink

    /** Downloads, over Library, as Library's row opens it. */
    data object Downloads : AppLink

    /** The Calendar, on today: Airing today's header. */
    data object Calendar : AppLink

    /** Settings › Accounts, to sign in: Continue watching's, signed out. */
    data object Accounts : AppLink

    /** A title's page, in Continue's back stack. */
    data class Title(val id: AnilistId) : AppLink

    /**
     * Continue a title: the player on the episode Continue would play next, over Continue, as its
     * amber action plays it; the title's page where nothing is out to play. The episode is worked
     * out as the link is followed, so a shortcut made days ago still plays the right one.
     */
    data class Continue(val id: AnilistId) : AppLink

    /** Play [episode] of a title, from its page in Continue's back stack. */
    data class Play(val id: AnilistId, val episode: String) : AppLink

    /** Play a finished download from its file, over Downloads, by the download's id. */
    data class Download(val id: Long) : AppLink

    /** A destination, as it was left: the Updated notification's, back where the update was taken. */
    data class Section(val destination: Destination) : AppLink

    /** The link's fields, as an intent's extras carry them: what it opens, and what of. */
    val fields: Map<String, String>
        get() = when (this) {
            Search -> mapOf(OPEN to "search")
            Downloads -> mapOf(OPEN to "downloads")
            Calendar -> mapOf(OPEN to "calendar")
            Accounts -> mapOf(OPEN to "accounts")
            is Title -> mapOf(OPEN to "title", SHOW to "$id")
            is Continue -> mapOf(OPEN to "continue", SHOW to "$id")
            is Play -> mapOf(OPEN to "play", SHOW to "$id", EPISODE to episode)
            is Download -> mapOf(OPEN to "download", DOWNLOAD to "$id")
            is Section -> mapOf(OPEN to "section", SECTION to destination.route)
        }

    /** A key of the link's own, "play/900001/8": two links are one intent only when they are equal. */
    val key: String get() = listOf(OPEN, SHOW, EPISODE, DOWNLOAD, SECTION).mapNotNull(fields::get).joinToString("/")

    companion object {
        /**
         * What the link opens: `search`, `downloads`, `calendar`, `accounts`, `title`, `continue`,
         * `play`, `download` or `section`.
         */
        const val OPEN = "open"

        /** The show, by its AniList id. */
        const val SHOW = "show"

        /** The episode, as the episode table numbers it. */
        const val EPISODE = "episode"

        /** The download, by its id. */
        const val DOWNLOAD = "download"

        /** The destination, by its route. */
        const val SECTION = "section"

        /** The link [field] spells out, or `null` when it is not one of the app's. */
        fun of(field: (String) -> String?): AppLink? {
            val show = field(SHOW)?.toUIntOrNull()
            return when (field(OPEN)) {
                "search" -> Search
                "downloads" -> Downloads
                "calendar" -> Calendar
                "accounts" -> Accounts
                "title" -> show?.let(::Title)
                "continue" -> show?.let(::Continue)
                "play" -> field(EPISODE)?.takeIf { it.isNotBlank() }?.let { episode -> show?.let { Play(it, episode) } }
                "download" -> field(DOWNLOAD)?.toLongOrNull()?.let(::Download)
                "section" -> field(SECTION)?.let(Destination::named)?.let(::Section)
                else -> null
            }
        }
    }
}

/**
 * Follow [link] through the app's own navigation, as a tap on the screens would go: a title in
 * Continue's back stack, Downloads over Library, Search with its field open, the player over
 * Continue. A link to continue waits for the title's row, at most [LINK_WAIT_MILLIS], and opens
 * the title's page where nothing is out to play.
 */
internal suspend fun Navigator.follow(link: AppLink, store: AppStore, actions: AppActions, openSearch: () -> Unit) {
    when (link) {
        AppLink.Search -> {
            go(Destination.Search)
            openSearch()
        }
        AppLink.Downloads -> {
            go(Destination.Library)
            open(DOWNLOADS_ROUTE)
        }
        AppLink.Calendar -> go(Destination.Calendar)
        is AppLink.Section -> go(link.destination)
        AppLink.Accounts -> openSettings(SettingsPane.Accounts)
        is AppLink.Title -> openTitleFromLink(link.id)
        is AppLink.Play -> {
            openTitleFromLink(link.id)
            actions.play(link.id, link.episode)
        }
        is AppLink.Download -> {
            go(Destination.Library)
            open(DOWNLOADS_ROUTE)
            store.loadDownloads()
            val row = withTimeoutOrNull(LINK_WAIT_MILLIS) {
                store.downloads.rows.map { rows -> rows.firstOrNull { it.id == link.id } }.filterNotNull().first()
            }
            if (row?.state == DownloadState.DONE) store.playDownload(row)
        }
        is AppLink.Continue -> {
            // The player, over Continue, on the episode Continue would play.
            go(Destination.Home)
            val entry = withTimeoutOrNull(LINK_WAIT_MILLIS) {
                store.titles.entries.map { it[link.id] }.filterNotNull().first()
            }
            val item = entry?.let { ContinueItem.of(it, store.titles.episodes.value[it.id]) }
            if (item != null && item.playable) actions.play(link.id, item.episode.toString()) else openTitleFromLink(link.id)
        }
    }
}

/** A title's page in Continue's back stack, over Continue's start. */
private fun Navigator.openTitleFromLink(id: AnilistId) {
    go(Destination.Home)
    openTitle(id, from = "link")
}

/** How long a link to continue a title waits for the title's row before it only shows the page. */
internal const val LINK_WAIT_MILLIS = 15_000L
