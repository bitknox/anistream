// The links the system opens the app on from outside its screens: launcher shortcuts,
// notifications and the home-screen widgets. Each is an `anistream://open/…` URI on an explicit intent to the activity, which
// hands it to the frame to follow through the app's own navigation.

package tv.anistream.ui

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withTimeoutOrNull
import tv.anistream.engine.core.AnilistId
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
     * Continue a title: its page in Continue's back stack, and the episode Continue would play
     * next, as its amber action plays it. The episode is worked out as the link is followed, so a
     * shortcut made days ago still plays the right one.
     */
    data class Continue(val id: AnilistId) : AppLink

    /** Play [episode] of a title, from its page in Continue's back stack. */
    data class Play(val id: AnilistId, val episode: String) : AppLink

    /** The link as a URI, for an intent's data. Two links are one intent only when they are equal. */
    val uri: String
        get() = PREFIX + when (this) {
            Search -> "search"
            Downloads -> "downloads"
            Calendar -> "calendar"
            Accounts -> "settings/accounts"
            is Title -> "title/$id"
            is Continue -> "continue/$id"
            is Play -> "play/$id/${URLEncoder.encode(episode, Charsets.UTF_8)}"
        }

    companion object {
        /** What every link starts with. */
        const val PREFIX = "anistream://open/"

        /** The link [uri] is, or `null` when it is not one of the app's. */
        fun parse(uri: String?): AppLink? {
            val parsed = uri?.let { runCatching { URI(it.trim()) }.getOrNull() } ?: return null
            if (parsed.scheme?.lowercase() != "anistream" || parsed.host != "open") return null
            val parts = parsed.rawPath.orEmpty().trim('/').split('/')
            val id = parts.getOrNull(1)?.toUIntOrNull()
            return when (parts.first()) {
                "search" -> Search.takeIf { parts.size == 1 }
                "downloads" -> Downloads.takeIf { parts.size == 1 }
                "calendar" -> Calendar.takeIf { parts.size == 1 }
                "settings" -> Accounts.takeIf { parts.size == 2 && parts[1] == "accounts" }
                "title" -> id?.takeIf { parts.size == 2 }?.let(::Title)
                "continue" -> id?.takeIf { parts.size == 2 }?.let(::Continue)
                "play" -> {
                    val episode = parts.getOrNull(2)?.let { URLDecoder.decode(it, Charsets.UTF_8) }
                    if (id != null && parts.size == 3 && !episode.isNullOrBlank()) Play(id, episode) else null
                }
                else -> null
            }
        }
    }
}

/**
 * Follow [link] through the app's own navigation, as a tap on the screens would go: a title in
 * Continue's back stack, Downloads over Library, Search with its field open. A link to continue or
 * play waits for the title's row, at most [LINK_WAIT_MILLIS], and plays only while its page is
 * still the one on screen.
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
        AppLink.Accounts -> openSettings(SettingsPane.Accounts)
        is AppLink.Title -> openTitleFromLink(link.id)
        is AppLink.Play -> {
            openTitleFromLink(link.id)
            actions.play(link.id, link.episode)
        }
        is AppLink.Continue -> {
            openTitleFromLink(link.id)
            val entry = withTimeoutOrNull(LINK_WAIT_MILLIS) {
                store.titles.entries.map { it[link.id] }.filterNotNull().first()
            } ?: return
            val item = ContinueItem.of(entry)
            if (item.playable && showing(link.id)) actions.play(link.id, item.episode.toString())
        }
    }
}

/** A title's page in Continue's back stack, over Continue's start. */
private fun Navigator.openTitleFromLink(id: AnilistId) {
    go(Destination.Continue)
    openTitle(id, from = "link")
}

/** How long a link to continue a title waits for the title's row before it only shows the page. */
internal const val LINK_WAIT_MILLIS = 15_000L
