// An episode's discussion: the AniList thread open in a sheet or panel, and which episodes have
// one, as the engine says.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.DiscussionView

/** Which episode of which title a discussion belongs to. */
data class DiscussionKey(val media: AnilistId, val episode: UInt)

class DiscussionModel {
    private val _open = MutableStateFlow<DiscussionView?>(null)

    /** The discussion that is open, whole, said again on every change; `null` once it closes. */
    val open: StateFlow<DiscussionView?> = _open.asStateFlow()

    private val _found = MutableStateFlow<Map<DiscussionKey, UInt>>(emptyMap())

    /** The episodes the engine found a thread for, with every comment in it. */
    val found: StateFlow<Map<DiscussionKey, UInt>> = _found.asStateFlow()

    private val _target = MutableStateFlow<DiscussionKey?>(null)

    /** The discussion a person asked to read, shown in a sheet (phone) or panel (TV) until dismissed. */
    val target: StateFlow<DiscussionKey?> = _target.asStateFlow()

    fun show(media: AnilistId, episode: UInt) {
        _target.value = DiscussionKey(media, episode)
    }

    fun dismiss() {
        _target.value = null
    }

    internal fun view(view: DiscussionView) {
        _open.value = view
    }

    internal fun found(media: AnilistId, episode: UInt, comments: UInt) {
        _found.value = _found.value + (DiscussionKey(media, episode) to comments)
    }

    internal fun closed() {
        _open.value = null
    }

    /** The comments the engine counts for an episode, `null` when it has no thread or has not said. */
    fun count(media: AnilistId, episode: UInt): UInt? = _found.value[DiscussionKey(media, episode)]
}
