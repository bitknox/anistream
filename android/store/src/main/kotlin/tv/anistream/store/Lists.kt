// The lists the engine sends, each tagged with the view it answers.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.LibrarySegment
import tv.anistream.engine.model.SearchFilters

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
 * Search, which takes only the answer to what was asked last: a slow answer to an earlier query
 * never replaces the results of a later one.
 */
class SearchModel {
    val list = ListModel()

    /** The query and filters as last submitted. */
    var submitted: Pair<String, SearchFilters>? = null
        private set

    internal fun submit(query: String, filter: SearchFilters) {
        submitted = query to filter
        list.loading()
        list.caption(null)
    }

    internal fun takes(query: String, filter: SearchFilters): Boolean =
        submitted?.let { it.first == query && it.second == filter } ?: false
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
