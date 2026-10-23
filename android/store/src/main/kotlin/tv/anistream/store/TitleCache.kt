// Every title the app has been told of, once, so every list and page shows a title as it now
// stands: a mark or a watch moves it everywhere at once.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.EpisodeProgress
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.ResumePoint
import tv.anistream.engine.model.TitleAired
import tv.anistream.engine.model.TitleProgress

/**
 * The titles by id, merged from every list, page and progress the engine sends. Lists hold ids
 * and read their rows from here.
 */
class TitleCache {
    private val _entries = MutableStateFlow<Map<AnilistId, Entry>>(emptyMap())
    val entries: StateFlow<Map<AnilistId, Entry>> = _entries.asStateFlow()

    private val _episodes = MutableStateFlow<Map<AnilistId, List<EpisodeRow>>>(emptyMap())

    /** Each title's episode table, as last loaded. */
    val episodes: StateFlow<Map<AnilistId, List<EpisodeRow>>> = _episodes.asStateFlow()

    /** The titles whose page has been loaded, so their detail-only fields are known. */
    var detailed: Set<AnilistId> = emptySet()
        private set

    private val _failed = MutableStateFlow<Set<AnilistId>>(emptySet())

    /** The titles whose page was asked for and did not come: what "AniList did not answer" says. */
    val failed: StateFlow<Set<AnilistId>> = _failed.asStateFlow()

    internal fun detailFailed(id: AnilistId) {
        _failed.value = _failed.value + id
    }

    internal fun detailAsked(id: AnilistId) {
        if (id in _failed.value) _failed.value = _failed.value - id
    }

    operator fun get(id: AnilistId): Entry? = _entries.value[id]

    private fun update(change: (MutableMap<AnilistId, Entry>) -> Unit) {
        _entries.value = _entries.value.toMutableMap().also(change)
    }

    /**
     * A list's rows. A list carries no detail-only fields, so a page's stay where one was loaded,
     * and so does a known list status the list does not know.
     */
    internal fun merge(list: List<Entry>) = update { entries ->
        for (entry in list) {
            val kept = entries[entry.id]
            if (kept == null) {
                entries[entry.id] = entry
                continue
            }
            var merged = entry.copy(
                listStatus = entry.listStatus ?: kept.listStatus,
                // A list arrives ahead of the broadcast line its own follow-up brings.
                lastAired = entry.lastAired ?: kept.lastAired,
            )
            if (entry.id in detailed) {
                merged = merged.copy(
                    related = kept.related,
                    recommended = kept.recommended,
                    trailerUrl = kept.trailerUrl,
                    availableOn = kept.availableOn,
                    // The page's own fields, its seasons among them: a list refreshed under an open
                    // page would otherwise take its picker and related titles away.
                    detail = kept.detail,
                )
            }
            entries[entry.id] = merged
        }
    }

    /** A title's page, whole. */
    internal fun merge(detail: Entry) {
        update { entries ->
            val kept = entries[detail.id]
            entries[detail.id] = if (kept == null) detail else detail.copy(
                listStatus = detail.listStatus ?: kept.listStatus,
                lastAired = detail.lastAired ?: kept.lastAired,
            )
        }
        detailed = detailed + detail.id
        if (detail.id in _failed.value) _failed.value = _failed.value - detail.id
    }

    /** Where a title stands after something moved it. */
    internal fun progress(
        id: AnilistId,
        progress: TitleProgress?,
        resume: ResumePoint?,
        rows: List<EpisodeProgress>,
    ) {
        update { entries -> entries[id]?.let { entries[id] = it.copy(progress = progress, resume = resume) } }
        if (rows.isEmpty()) return
        val table = _episodes.value[id] ?: return
        val marked = rows.associateBy { it.number }
        _episodes.value = _episodes.value + (id to table.map { row ->
            marked[row.number]?.let { row.copy(watched = it.watched, completed = it.completed) } ?: row
        })
    }

    /** The latest broadcast of each airing title, said after the list it annotates. */
    internal fun lastAired(rows: List<TitleAired>) = update { entries ->
        for (row in rows) {
            entries[row.id]?.let { entries[row.id] = it.copy(lastAired = row.aired) }
        }
    }

    internal fun episodes(id: AnilistId, rows: List<EpisodeRow>) {
        _episodes.value = _episodes.value + (id to rows)
    }

    /**
     * Whether [id]'s episodes, as last loaded, are AniList's alone: no source set up has it, so it
     * can be marked watched but there is nothing to play or download it from.
     */
    fun unlisted(id: AnilistId): Boolean = _episodes.value[id]?.firstOrNull()?.metadataOnly == true

    /** The titles whose episodes, as last loaded, are AniList's alone. */
    internal fun unlistedTitles(): List<AnilistId> =
        _episodes.value.filterValues { it.firstOrNull()?.metadataOnly == true }.keys.toList()

    /** A title put on a list here, ahead of the tracker's word. */
    fun setStatus(id: AnilistId, segment: LibrarySegment) = update { entries ->
        entries[id]?.let { entries[id] = it.copy(listStatus = segment) }
    }
}
