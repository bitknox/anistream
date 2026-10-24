// Where the TV is: one of the rail's six destinations, and the pages opened over it (a title page
// for now). Each keeps a focus memory, so returning to it puts focus back on what opened the page.

package tv.anistream.tv.shell

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import tv.anistream.engine.core.AnilistId
import tv.anistream.tv.ui.FocusMemory
import tv.anistream.tv.ui.TvIcons

/** The rail's destinations, top to bottom: Search, then the phone's order, then Settings. */
enum class Destination(val label: String, val icon: ImageVector) {
    Search("Search", TvIcons.Search),
    Continue("Continue", TvIcons.Continue),
    Calendar("Calendar", TvIcons.Calendar),
    Seasonal("Seasonal", TvIcons.Seasonal),
    Library("Library", TvIcons.Library),
    Settings("Settings", TvIcons.Settings),
    ;

    companion object {
        /** A destination by the fixtures' section names: continue, search, library and so on. */
        fun named(name: String?): Destination? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

/** A page over a destination, with its own focus memory. */
sealed interface Page {
    /** The key its state is saved under, unique to this opening. */
    val key: String
    val memory: FocusMemory

    data class Title(val id: AnilistId, override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVSignIn for a tracker, over Settings › Accounts. */
    data class SignIn(val tracker: String, override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }
}

/**
 * The TV's place: the destination, the pages over it, and what the screen showing now does when
 * Back asks it to go to its top.
 */
@Stable
class TvNavigator(start: Destination = Destination.Continue) {
    var destination by mutableStateOf(start)
        private set

    /** The pages over the destination, the last on top. */
    val pages = mutableStateListOf<Page>()

    private val memories = Destination.entries.associateWith { FocusMemory() }

    private var opened = 0

    /** Search's text, kept while the TV stays on Search, title pages included. */
    var searchText by mutableStateOf("")
        private set

    /** Where the field's cursor is in [searchText]: the end, unless the system keyboard moved it. */
    var searchCursor by mutableIntStateOf(0)
        private set

    /**
     * The field's text either side of where a phone types, from its first text since the TV last
     * typed: each text it sends, whole, goes between the two, so a lost or repeated one leaves the
     * same field.
     */
    private var phoneAround: Pair<String, String>? = null

    /** The TV typed: [text], the cursor at [cursor] (the end when not given). */
    fun typeSearch(text: String, cursor: Int = text.length) {
        phoneAround = null
        searchText = text
        searchCursor = cursor.coerceIn(0, text.length)
    }

    /** A phone's field reads [text]: put in at the TV field's cursor, which is its end unless moved. */
    fun phoneSearch(text: String) {
        val (before, after) = phoneAround ?: run {
            val at = searchCursor.coerceIn(0, searchText.length)
            (searchText.take(at) to searchText.drop(at)).also { phoneAround = it }
        }
        val head = if (before.isNotEmpty() && !before.endsWith(' ') && text.isNotEmpty() && !text.startsWith(' ')) "$before " else before
        val tail = if (after.isNotEmpty() && !after.startsWith(' ') && text.isNotEmpty() && !text.endsWith(' ')) " $after" else after
        searchText = head + text + tail
        searchCursor = head.length + text.length
    }

    /** The phone's typing ended, by its Enter: what it types next goes in afresh. */
    fun phoneDone() {
        phoneAround = null
    }

    /** Counts a phone's Search submits: each moves focus to the first result. */
    var searchSubmits by mutableIntStateOf(0)
        private set

    /** A phone submitted Search, as its Enter does. */
    fun submitSearch() {
        searchSubmits += 1
    }

    /**
     * Forget the saved state under a key: a page closed, or a destination entered afresh. The root
     * points it at its state holder.
     */
    var forget: (String) -> Unit = {}

    fun memory(destination: Destination): FocusMemory = memories.getValue(destination)

    val top: Page? get() = pages.lastOrNull()

    /** What the content shows: the top page, else the destination. */
    val screenKey: String get() = top?.key ?: destination.name

    /** The focus memory of what shows. */
    val focus: FocusMemory get() = top?.memory ?: memory(destination)

    /**
     * Set by the screen showing: scroll to its top and focus its first focus, true when it was not
     * already there. Back at the top of a screen opens the rail instead.
     */
    var toTop: (() -> Boolean)? = null

    /**
     * Go to [to] from the rail. Another destination starts at its first focus with its pages
     * closed; the same one returns to where its focus was.
     */
    fun go(to: Destination) {
        if (to == destination && pages.isEmpty()) return
        pages.forEach { forget(it.key) }
        pages.clear()
        if (to != destination) {
            if (destination == Destination.Search) typeSearch("")
            forget(to.name)
            memory(to).reset()
            destination = to
        }
    }

    fun openTitle(id: AnilistId) {
        opened += 1
        pages += Page.Title(id, "title/$id/$opened")
    }

    fun openSignIn(tracker: String) {
        opened += 1
        pages += Page.SignIn(tracker, "sign-in/$tracker/$opened")
    }

    /** Close the top page; false when there was none. */
    fun pop(): Boolean {
        val page = pages.removeLastOrNull() ?: return false
        forget(page.key)
        return true
    }
}
