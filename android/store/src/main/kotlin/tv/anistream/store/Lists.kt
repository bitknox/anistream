// The lists the engine sends, each tagged with the view it answers.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Bounds
import tv.anistream.engine.model.BrowseRail
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SearchFacets
import tv.anistream.engine.model.SearchQuery
import tv.anistream.engine.model.SearchSort
import tv.anistream.engine.model.Season

/** Where a list stands. */
sealed interface ListState {
    /** Never asked for. */
    data object Idle : ListState

    /** Asked for, with nothing to show yet. */
    data object Loading : ListState

    /** These titles, in the engine's order; the rows are in the title cache. */
    data class Loaded(override val ids: List<AnilistId>) : ListState

    /** Nothing there at all: for Continue, nothing watched yet. */
    data object Empty : ListState

    /** It could not be loaded, and why, in words to show. */
    data class Failed(val reason: String) : ListState

    /** The titles, when loaded. */
    val ids: List<AnilistId> get() = emptyList()
}

/** One list: its state and the caption the engine says above it. */
class ListModel(private val clock: () -> Long = System::nanoTime) {
    private val _state = MutableStateFlow<ListState>(ListState.Idle)
    val state: StateFlow<ListState> = _state.asStateFlow()

    private val _caption = MutableStateFlow<String?>(null)

    /** What the list is, said above it: which season Seasonal is, or that it stopped at a page. */
    val caption: StateFlow<String?> = _caption.asStateFlow()

    /** When the engine last answered with the list, or that it is empty, on [clock]. */
    var answeredAt: Long? = null
        private set

    /**
     * Whether the list was answered less than [windowMillis] ago, so asking again would only spend
     * a request on what is on screen.
     */
    fun answered(withinMillis: Long): Boolean =
        answeredAt?.let { (clock() - it) / 1_000_000 < withinMillis } ?: false

    /** The list has been asked for. Keeps what is on screen until the answer replaces it. */
    fun asked() {
        when (_state.value) {
            ListState.Idle, is ListState.Failed -> _state.value = ListState.Loading
            else -> {}
        }
    }

    internal fun loading() {
        _state.value = ListState.Loading
    }

    internal fun idle() {
        _state.value = ListState.Idle
    }

    internal fun loaded(ids: List<AnilistId>) {
        _state.value = ListState.Loaded(ids)
    }

    internal fun caption(text: String?) {
        _caption.value = text
    }

    internal fun apply(content: ContentList, titles: TitleCache) {
        when (content) {
            ContentList.Empty -> {
                _state.value = ListState.Empty
                answeredAt = clock()
            }
            ContentList.Loading -> _state.value = ListState.Loading
            is ContentList.Entries -> {
                titles.merge(content.v1)
                _state.value = ListState.Loaded(content.v1.map { it.id })
                answeredAt = clock()
            }
            is ContentList.Failed -> _state.value = ListState.Failed(content.v1)
        }
    }
}

/** The broadcasts around now, and how to page back through earlier ones. */
class CalendarModel {
    val list = ListModel()

    /** The calendar's anchor, its pages, whether AniList has more behind them, and its rows. */
    data class Window(
        val anchor: Long? = null,
        val pages: UInt = 0u,
        val morePast: Boolean = false,
        /**
         * The rows, oldest first: the broadcasts, each a title on a day, so not unique by title
         * the way another list's are.
         */
        val rows: List<Entry> = emptyList(),
    )

    private val _window = MutableStateFlow(Window())
    val window: StateFlow<Window> = _window.asStateFlow()

    internal fun apply(content: ContentList, titles: TitleCache) {
        list.apply(content, titles)
        _window.value = _window.value.copy(rows = (content as? ContentList.Entries)?.v1.orEmpty())
    }

    internal fun window(anchor: Long, pages: UInt, morePast: Boolean) {
        _window.value = _window.value.copy(anchor = anchor, pages = pages, morePast = morePast)
    }

    /**
     * Older broadcasts, above what is there. A page of `null` failed: nothing to add, and the same
     * page is asked for again on the next scroll.
     */
    internal fun backfill(page: UInt?, entries: List<Entry>, morePast: Boolean, titles: TitleCache) {
        page ?: return
        titles.merge(entries)
        val now = _window.value
        _window.value = now.copy(
            rows = entries + now.rows,
            pages = maxOf(now.pages, page + 1u),
            morePast = morePast,
        )
    }
}

/**
 * Search: one query, what is typed and every filter, and the answer to it. It takes only the
 * answer to what was asked last: a slow answer to an earlier query never replaces the results of
 * a later one.
 */
class SearchModel {
    val list = ListModel()

    /** Where a search stands besides its list: its query, its paging, and Search's start. */
    data class Status(
        /** The search as it stands: what is typed and every filter. Blank is Search's start. */
        val query: SearchQuery = SearchQuery(),
        /** What the list answers, as the engine tags it: [query] trimmed. `null` at the start. */
        val asked: SearchQuery? = null,
        /** Whether AniList has more pages behind those on screen. */
        val hasNext: Boolean = false,
        /** Whether a further page is on its way. */
        val loadingMore: Boolean = false,
        /** How many pages of results are on screen. */
        val pages: UInt = 0u,
        /** Search's start: rails of titles, each a query of its own. `null` until sent. */
        val rails: List<BrowseRail>? = null,
        /** What the filters choose from. `null` until sent. */
        val facets: SearchFacets? = null,
        /** The recent searches, newest first, each with its filters. */
        val recents: List<SearchQuery> = emptyList(),
        /** Whether typing waits for the search key: the AniList budget ran low. */
        val livePaused: Boolean = false,
    ) {
        /** Whether Search shows its start: rails and recent searches rather than results. */
        val atStart: Boolean get() = query.isBlank()
    }

    private val _status = MutableStateFlow(Status())
    val status: StateFlow<Status> = _status.asStateFlow()

    /** What the list answers, `null` at the start. */
    val asked: SearchQuery? get() = _status.value.asked

    private fun update(change: (Status) -> Status) {
        _status.value = change(_status.value)
    }

    /**
     * The search changed. Whether it asks anything new of the engine: not at the start, and not
     * when only spaces around the text changed.
     */
    internal fun edit(query: SearchQuery): Boolean {
        val normal = query.normalized
        if (normal.isBlank()) {
            update { it.copy(query = query, asked = null, hasNext = false, loadingMore = false, pages = 0u) }
            list.idle()
            list.caption(null)
            return false
        }
        if (normal == _status.value.asked) {
            update { it.copy(query = query) }
            return false
        }
        update { it.copy(query = query, asked = normal, hasNext = false, loadingMore = false, pages = 0u) }
        // What is on screen stays until the answer replaces it, so typing does not flash.
        list.asked()
        return true
    }

    /** Search again for what is asked, as when titles are named another way. */
    internal fun again(): SearchQuery? {
        val asked = _status.value.asked ?: return null
        list.asked()
        return asked
    }

    /** Whether an answer tagged with [query] is for what is on screen. */
    internal fun takes(query: SearchQuery): Boolean = _status.value.asked == query

    internal fun paging(query: SearchQuery, hasNext: Boolean) {
        if (!takes(query)) return
        update { it.copy(hasNext = hasNext, pages = 1u, loadingMore = false) }
    }

    /** The next page to ask for, when there is one and none is on its way. */
    internal fun nextPage(): Pair<SearchQuery, UInt>? {
        val now = _status.value
        val asked = now.asked ?: return null
        if (!now.hasNext || now.loadingMore || list.state.value !is ListState.Loaded) return null
        update { it.copy(loadingMore = true) }
        return asked to now.pages + 1u
    }

    /**
     * A further page, after what is there. An empty page failed, and is asked for again on the
     * next scroll.
     */
    internal fun more(query: SearchQuery, page: UInt, entries: List<Entry>, hasNext: Boolean, titles: TitleCache) {
        if (!takes(query)) return
        if (entries.isEmpty()) {
            update { it.copy(loadingMore = false) }
            return
        }
        titles.merge(entries)
        val ids = list.state.value.ids
        val seen = ids.toSet()
        list.loaded(ids + entries.map { it.id }.filterNot { it in seen })
        update { it.copy(loadingMore = false, pages = maxOf(it.pages, page), hasNext = hasNext) }
    }

    internal fun rails(rails: List<BrowseRail>, titles: TitleCache) {
        for (rail in rails) titles.merge(rail.entries)
        update { it.copy(rails = rails) }
    }

    internal fun facets(facets: SearchFacets) = update { it.copy(facets = facets) }

    internal fun recents(recents: List<SearchQuery>) = update { it.copy(recents = recents) }

    internal fun live(paused: Boolean) = update { it.copy(livePaused = paused) }
}

/** The query as the engine searches for it and tags its answer: its text without the spaces around. */
val SearchQuery.normalized: SearchQuery get() = copy(text = text.trim())

/** The season on now, its most popular first: what Seasonal shows, as a search. In UTC, as the engine reckons it. */
fun seasonQuery(at: java.time.Instant = java.time.Instant.now()): SearchQuery {
    val date = at.atZone(java.time.ZoneOffset.UTC)
    val season = when (date.monthValue) {
        in 1..3 -> Season.WINTER
        in 4..6 -> Season.SPRING
        in 7..9 -> Season.SUMMER
        else -> Season.FALL
    }
    val year = date.year.toUInt()
    return SearchQuery(sort = SearchSort.POPULARITY, season = season, years = Bounds(min = year, max = year))
}

/** The tracker's lists, a segment each. */
class LibraryModel {
    val segments: Map<LibrarySegment, ListModel> = order.associateWith { ListModel() }

    fun segment(segment: LibrarySegment): ListModel = segments.getValue(segment)

    companion object {
        /** The segments in the tracker's order. */
        val order = listOf(
            LibrarySegment.WATCHING,
            LibrarySegment.PLANNING,
            LibrarySegment.COMPLETED,
            LibrarySegment.PAUSED,
            LibrarySegment.DROPPED,
        )
    }
}
