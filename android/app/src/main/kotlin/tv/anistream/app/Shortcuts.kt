// The launcher's shortcuts, as native-layer.md has them: Search and Downloads, static in
// res/xml/shortcuts.xml, and Continue, published here from the Continue queue as it changes. Each,
// like a notification's links, opens the activity on an explicit intent with the link in its
// extras, never a URL.

package tv.anistream.app

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import tv.anistream.store.AppStore
import tv.anistream.store.ListState
import tv.anistream.ui.AppLink
import tv.anistream.ui.showTitle

/** The action of every intent that opens the activity on an app link. */
const val ACTION_OPEN = "tv.anistream.action.OPEN"

/** What a link's fields are called among an intent's extras: `tv.anistream.link.open` and the rest. */
fun linkExtra(field: String): String = "tv.anistream.link.$field"

/**
 * The intent that opens the activity on [link]: explicit, so no other app can answer it, the
 * link's fields in its extras, and the link's key as its identifier, so each link is an intent of
 * its own to the system.
 */
fun Context.linkIntent(link: AppLink): Intent = Intent(ACTION_OPEN, null, this, MainActivity::class.java).apply {
    for ((field, value) in link.fields) putExtra(linkExtra(field), value)
    identifier = link.key
}

/** The app link this intent opens on, or `null` when it is not one. */
fun Intent.appLink(): AppLink? = if (action == ACTION_OPEN) AppLink.of { getStringExtra(linkExtra(it)) } else null

/**
 * The launcher's Continue shortcut, ranked first: "Continue", or in full "Hoshikuzu no Tabibito
 * E7", for the first title in the Continue queue with an episode out, which opens the player on
 * that episode. One shortcut, which follows the queue: a copy pinned to the home screen follows it
 * too, and with nothing to continue the shortcut goes from the launcher's list and a pinned copy
 * is disabled, saying "Nothing to continue", until there is something again.
 */
class ContinueShortcut(private val context: Context) {
    /** The title the shortcut continues, the name it goes by, and the episode it plays. */
    data class Target(val id: UInt, val title: String, val episode: UInt)

    /**
     * Publish the shortcut for [store]'s queue from now on, in [scope]. While the queue is not
     * known (loading, failed) the last one stands.
     */
    fun follow(store: AppStore, scope: CoroutineScope) {
        scope.launch {
            targets(store).collect(::publish)
        }
    }

    /** Make the launcher's Continue continue [target], or say there is nothing to continue. */
    fun publish(target: Target?) {
        runCatching {
            if (target == null) {
                ShortcutManagerCompat.removeDynamicShortcuts(context, listOf(ID))
                ShortcutManagerCompat.disableShortcuts(context, listOf(ID), context.getString(R.string.shortcut_nothing_to_continue))
            } else {
                val info = info(target)
                ShortcutManagerCompat.enableShortcuts(context, listOf(info))
                ShortcutManagerCompat.pushDynamicShortcut(context, info)
            }
        }.onFailure {
            // The system limits how often shortcuts change; the next change tries again.
            Log.w(AnistreamApp.TAG, "the Continue shortcut was not published: ${it.message}")
        }
    }

    /** [target]'s shortcut: "Continue", its title and episode in full, the glyph and the link. */
    fun info(target: Target): ShortcutInfoCompat =
        ShortcutInfoCompat.Builder(context, ID)
            .setShortLabel(context.getString(R.string.shortcut_continue))
            .setLongLabel(longLabel(target.title, target.episode))
            .setIcon(IconCompat.createWithResource(context, R.drawable.shortcut_continue))
            .setIntent(context.linkIntent(AppLink.Continue(target.id)))
            .setRank(0)
            .build()

    companion object {
        /** The shortcut's id: one Continue, whatever it continues. */
        const val ID = "continue"

        /** The longest long label, as native-layer.md has it: under 25 characters. */
        const val LONG_LABEL = 24

        /** "Hoshikuzu no Tabibito E7", the title cut short with an ellipsis where it would not fit. */
        fun longLabel(title: String, episode: UInt): String {
            val suffix = " E$episode"
            val room = LONG_LABEL - suffix.length
            val name = if (title.length <= room) title else title.take(room - 1).trimEnd() + "…"
            return name + suffix
        }

        /**
         * The title the shortcut is for, each time it changes: the queue's first with an episode
         * out, or `null` when there is none. Nothing while the queue is not known.
         */
        fun targets(store: AppStore): Flow<Target?> =
            combine(store.continuing.list.state, store.continuing.hidden, store.titles.entries) { state, _, titles ->
                when (state) {
                    is ListState.Loaded -> Wanted(store.continuing.queue(titles).nextUp?.let { Target(it.id, it.entry.showTitle, it.episode) })
                    ListState.Empty -> Wanted(null)
                    else -> null
                }
            }.filterNotNull().distinctUntilChanged().map { it.target }

        /** The shortcut the queue wants, told apart from not knowing yet. */
        private data class Wanted(val target: Target?)

        /** The shortcut id of the link the activity was opened on, when a shortcut opens it. */
        fun shortcutOf(link: AppLink): String? = when (link) {
            AppLink.Search -> "search"
            AppLink.Downloads -> "downloads"
            is AppLink.Continue -> ID
            AppLink.Calendar, AppLink.Accounts, is AppLink.Title, is AppLink.Play, is AppLink.Download, is AppLink.Section -> null
        }
    }
}
