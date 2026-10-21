// The launcher's shortcuts: Search and Downloads, static in res/xml/shortcuts.xml, and Continue
// <title>, published here from the Continue queue as it changes.

package tv.anistream.app

import android.content.Context
import android.content.Intent
import android.net.Uri
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

/** The intent that opens the activity on [link]: explicit, so no other app can answer it. */
fun Context.linkIntent(link: AppLink): Intent =
    Intent(Intent.ACTION_VIEW, Uri.parse(link.uri), this, MainActivity::class.java)

/**
 * The launcher's shortcut to continue the first title in the Continue queue with an episode out:
 * "Continue <title>", which opens its page and plays the episode Continue would. One at a time;
 * as the queue's first title changes, the old one is taken off the launcher's list, though a copy
 * pinned to the home screen stays and still opens its title.
 */
class ContinueShortcut(private val context: Context) {
    /** The title the shortcut is for, and the name it goes by. */
    data class Target(val id: UInt, val title: String) {
        val shortcutId: String get() = PREFIX + id
    }

    /**
     * Publish the shortcut for [store]'s queue from now on, in [scope]. While the queue is not
     * known (loading, failed) the last one stands.
     */
    fun follow(store: AppStore, scope: CoroutineScope) {
        scope.launch {
            targets(store).collect(::publish)
        }
    }

    /** Make the launcher's list hold [target]'s shortcut alone among Continue's, or none. */
    fun publish(target: Target?) {
        runCatching {
            val stale = ShortcutManagerCompat.getDynamicShortcuts(context)
                .map { it.id }
                .filter { it.startsWith(PREFIX) && it != target?.shortcutId }
            if (stale.isNotEmpty()) ShortcutManagerCompat.removeDynamicShortcuts(context, stale)
            if (target != null) ShortcutManagerCompat.pushDynamicShortcut(context, info(target))
        }.onFailure {
            // The system limits how often shortcuts change; the next change tries again.
            Log.w(AnistreamApp.TAG, "the Continue shortcut was not published: ${it.message}")
        }
    }

    /** [target]'s shortcut: its title, the Continue glyph, and the link that continues it. */
    fun info(target: Target): ShortcutInfoCompat =
        ShortcutInfoCompat.Builder(context, target.shortcutId)
            .setShortLabel(target.title)
            .setLongLabel(context.getString(R.string.shortcut_continue_long, target.title))
            .setIcon(IconCompat.createWithResource(context, R.drawable.shortcut_continue))
            .setIntent(context.linkIntent(AppLink.Continue(target.id)))
            .setRank(0)
            .build()

    companion object {
        /** What every Continue shortcut's id starts with; the rest is the title's id. */
        const val PREFIX = "continue-"

        /**
         * The title the shortcut is for, each time it changes: the queue's first with an episode
         * out, or `null` when there is none. Nothing while the queue is not known.
         */
        fun targets(store: AppStore): Flow<Target?> =
            combine(store.continuing.list.state, store.continuing.hidden, store.titles.entries) { state, _, titles ->
                when (state) {
                    is ListState.Loaded -> Wanted(store.continuing.queue(titles).nextUp?.let { Target(it.id, it.entry.showTitle) })
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
            is AppLink.Continue -> PREFIX + link.id
            AppLink.Calendar, AppLink.Accounts, is AppLink.Title, is AppLink.Play -> null
        }
    }
}
