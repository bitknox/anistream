// What the source picker shows: the options the engine listed for a title, which of them is
// pinned, and the one notice a pin the engine could not use is owed.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.SourceOption

/** A pinned source that could not be used, for the notice that says so. */
data class PinUnavailableNotice(
    val id: AnilistId,
    val name: String,
    /** Tells two of the same words apart, so each one is said. */
    val serial: Int,
)

/** The picker's options per title, from `Event.SourceOptions`. */
class SourceChoiceModel {
    private val _options = MutableStateFlow<Map<AnilistId, List<SourceOption>>>(emptyMap())

    /**
     * The latest listing of each title, in the order its sources are tried. A listing stays until
     * the next one replaces it, so reopening the picker shows it at once.
     */
    val options: StateFlow<Map<AnilistId, List<SourceOption>>> = _options.asStateFlow()

    private val _asked = MutableStateFlow<Set<AnilistId>>(emptySet())

    /** Titles whose listing was asked for and has not come. */
    val asked: StateFlow<Set<AnilistId>> = _asked.asStateFlow()

    private val _pinNotice = MutableStateFlow<PinUnavailableNotice?>(null)

    /** The newest pin the engine could not use. The shell says it once. */
    val pinNotice: StateFlow<PinUnavailableNotice?> = _pinNotice.asStateFlow()

    private val said = mutableSetOf<String>()
    private var serial = 0

    /** The notice was said; it is not said again. */
    fun told(notice: PinUnavailableNotice) {
        if (_pinNotice.value == notice) _pinNotice.value = null
    }

    /** Whether the picker for [id] has nothing to show yet. */
    fun isLoading(id: AnilistId): Boolean = _options.value[id] == null && id in _asked.value

    internal fun asking(id: AnilistId) {
        _asked.value = _asked.value + id
    }

    /** The picker closed: an answer that never came is not waited for. */
    fun giveUp(id: AnilistId) {
        _asked.value = _asked.value - id
    }

    internal fun apply(id: AnilistId, listed: List<SourceOption>) {
        _asked.value = _asked.value - id
        _options.value = _options.value + (id to listed)
    }

    /**
     * A choice made, shown at once: the pinned flags follow it, so the picker reads right when it
     * opens again before the engine's next listing.
     */
    internal fun chose(id: AnilistId, provider: String?, release: String? = null) {
        // A new choice may meet the same unavailable source again, and says so again.
        said.removeAll { it.startsWith("$id:") }
        val listed = _options.value[id] ?: return
        _options.value = _options.value + (id to listed.map { option ->
            val pinned = option.providerId == provider
            option.copy(pinned = pinned, pinnedRelease = if (pinned) release else null)
        })
    }

    internal fun pinUnavailable(id: AnilistId, providerId: String, name: String) {
        if (!said.add("$id:$providerId")) return
        serial += 1
        _pinNotice.value = PinUnavailableNotice(id, name, serial)
    }
}
