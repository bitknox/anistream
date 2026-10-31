// What the match sheet shows: the question a source could not answer for a title, what a search of
// the source by hand found, the pick on its way, and what the title is matched to now.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.CurrentMatch
import tv.anistream.engine.model.MatchCandidate
import tv.anistream.engine.model.Msg

/**
 * The titles a source offered for a show when it could not choose between them, from
 * `Event.MatchChoices` answering the ladder.
 */
data class MatchAsked(
    val providerId: String,
    /** The source's name, as a person reads it. */
    val source: String,
    val candidates: List<MatchCandidate>,
) {
    /** The candidates the ladder did not rule out, which the page's line counts. */
    val possible: Int get() = candidates.count { it.rejected == null }
}

/** A search of a source by hand, from the sheet's field. */
sealed interface MatchSearch {
    val query: String

    data class Searching(override val query: String) : MatchSearch

    /** What the source found. [source] is its name, [providerId] the one to pin. */
    data class Found(
        override val query: String,
        val providerId: String,
        val source: String,
        val candidates: List<MatchCandidate>,
    ) : MatchSearch

    /** The source answered with nothing. */
    data class Empty(override val query: String, val source: String) : MatchSearch

    /** The source could not be searched. [reason] is the engine's message, worded where it is shown. */
    data class Failed(override val query: String, val reason: Msg) : MatchSearch
}

/** A pick that has settled, for the notice that says it and the sheet that closes on it. */
data class MatchSettled(
    val id: AnilistId,
    val title: String,
    val source: String,
    /** Tells two of the same picks apart. */
    val serial: Int,
)

/** Everything the match sheet reads, per title. */
data class MatchState(
    /** The open question per title: the source could not choose, so the page asks. */
    val asked: Map<AnilistId, MatchAsked> = emptyMap(),
    /** What each title is matched to now, from `Event.TitleMatch`. */
    val current: Map<AnilistId, CurrentMatch> = emptyMap(),
    /** Titles whose match the engine has said, matched or not. */
    val known: Set<AnilistId> = emptySet(),
    /** The newest search by hand per title. */
    val searches: Map<AnilistId, MatchSearch> = emptyMap(),
    /** The key picked per title, while the source lists its episodes. */
    val picking: Map<AnilistId, String> = emptyMap(),
    /** Why the last pick for a title could not be used. */
    val pickFailed: Map<AnilistId, Msg> = emptyMap(),
    /** The newest pick that settled. */
    val settled: MatchSettled? = null,
) {
    /** Whether the page for [id] asks for its match in place of Play. */
    fun asks(id: AnilistId): Boolean = id in asked

    /** The source the sheet for [id] searches: the one that asked, else the one matched now. */
    fun source(id: AnilistId): Pair<String, String>? {
        asked[id]?.let { return it.providerId to it.source }
        current[id]?.let { return it.providerId to it.source }
        (searches[id] as? MatchSearch.Found)?.let { return it.providerId to it.source }
        return null
    }
}

/** The title whose match sheet is open, and how it was opened. */
data class MatchPickerRequest(
    val id: AnilistId,
    /** Opened from Fix this match, not by the page's question: the field holds the title and searches at once. */
    val fromMenu: Boolean,
)

/** The match sheet's state per title. */
class MatchPickerModel {
    private val _request = MutableStateFlow<MatchPickerRequest?>(null)

    /** The sheet that is open, if one is. */
    val request: StateFlow<MatchPickerRequest?> = _request.asStateFlow()

    fun open(id: AnilistId, fromMenu: Boolean) {
        _request.value = MatchPickerRequest(id, fromMenu)
    }

    /** Whether the TV's match search is up, so a paired phone's typing goes to it and not to Search. */
    @Volatile
    var tvSearching: Boolean = false

    private val _phone = kotlinx.coroutines.flow.MutableSharedFlow<Pair<String, Boolean>>(extraBufferCapacity = 8)

    /** What a paired phone typed into the TV's match search, and whether it pressed Search. */
    val phone: kotlinx.coroutines.flow.SharedFlow<Pair<String, Boolean>> = _phone

    fun phoneTyped(text: String, submit: Boolean) {
        _phone.tryEmit(text to submit)
    }

    /** Whether there is a source to match a title on, for Fix this match. */
    fun canFix(providers: List<tv.anistream.engine.model.ProviderRow>): Boolean = providers.any { it.usable }

    fun dismiss() {
        _request.value = null
    }

    private val _state = MutableStateFlow(MatchState())
    val state: StateFlow<MatchState> = _state.asStateFlow()

    private val picked = mutableMapOf<AnilistId, Pair<String, String>>()
    private var serial = 0

    fun asked(id: AnilistId, providerId: String, source: String, candidates: List<MatchCandidate>) =
        _state.update { it.copy(asked = it.asked + (id to MatchAsked(providerId, source, candidates))) }

    fun searching(id: AnilistId, query: String) =
        _state.update { it.copy(searches = it.searches + (id to MatchSearch.Searching(query)), pickFailed = it.pickFailed - id) }

    fun found(id: AnilistId, query: String, providerId: String, source: String, candidates: List<MatchCandidate>) =
        _state.update {
            // An answer to a search the sheet has moved on from is not shown.
            if (it.searches[id]?.query != query) return@update it
            val answer = if (candidates.isEmpty()) MatchSearch.Empty(query, source)
            else MatchSearch.Found(query, providerId, source, candidates)
            it.copy(searches = it.searches + (id to answer))
        }

    fun searchFailed(id: AnilistId, query: String, reason: Msg) =
        _state.update {
            if (it.searches[id]?.query != query) return@update it
            it.copy(searches = it.searches + (id to MatchSearch.Failed(query, reason)))
        }

    fun picking(id: AnilistId, key: String, title: String, source: String) {
        picked[id] = title to source
        _state.update { it.copy(picking = it.picking + (id to key), pickFailed = it.pickFailed - id) }
    }

    fun fixFailed(id: AnilistId, reason: Msg) {
        picked.remove(id)
        _state.update { it.copy(picking = it.picking - id, pickFailed = it.pickFailed + (id to reason)) }
    }

    fun matched(id: AnilistId, match: CurrentMatch?) = _state.update { before ->
        var now = before.copy(known = before.known + id, current = if (match == null) before.current - id else before.current + (id to match))
        if (match == null) return@update now
        // Matched: the question is answered.
        now = now.copy(asked = now.asked - id)
        if (id in now.picking) {
            val said = picked.remove(id)
            serial += 1
            now = now.copy(
                picking = now.picking - id,
                searches = now.searches - id,
                settled = MatchSettled(id, match.title ?: said?.first.orEmpty(), match.source, serial),
            )
        }
        now
    }

    /** The person let the engine match [id] again: what they chose is forgotten here at once. */
    fun cleared(id: AnilistId) {
        picked.remove(id)
        _state.update {
            it.copy(current = it.current - id, searches = it.searches - id, picking = it.picking - id, pickFailed = it.pickFailed - id)
        }
    }

    /** The sheet for [id] closed: a search or pick it showed is not kept for the next one. */
    fun closed(id: AnilistId) =
        _state.update { it.copy(searches = it.searches - id, pickFailed = it.pickFailed - id) }
}
