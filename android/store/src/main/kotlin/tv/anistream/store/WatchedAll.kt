package tv.anistream.store

import tv.anistream.engine.model.Entry

/**
 * Whether every episode of a show with a known count is watched, so the engine offers episode 1
 * again and the title page's main button says Watch again.
 */
val Entry.watchedAll: Boolean
    get() {
        val total = episodes?.takeIf { it > 0u } ?: return false
        val p = progress ?: return false
        return p.done >= total && p.next == 1u
    }
