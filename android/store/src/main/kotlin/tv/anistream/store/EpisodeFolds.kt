// Which title pages show their watched episodes, for as long as the app runs.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId

/**
 * A title page starts its episodes at the last one watched, the ones before it folded under one
 * row; a tap on the row unfolds them. Each title keeps its choice for the session.
 */
class EpisodeFolds {
    private val _unfolded = MutableStateFlow<Set<AnilistId>>(emptySet())

    /** The titles whose watched episodes are showing. */
    val unfolded: StateFlow<Set<AnilistId>> = _unfolded.asStateFlow()

    fun toggle(id: AnilistId) {
        _unfolded.value = if (id in _unfolded.value) _unfolded.value - id else _unfolded.value + id
    }
}
