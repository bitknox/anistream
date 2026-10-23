// Where the TV is: one of the rail's six destinations, and the pages opened over it (a title page
// for now). Each keeps a focus memory, so returning to it puts focus back on what opened the page.

package tv.anistream.tv.shell

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
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
            if (destination == Destination.Search) searchText = ""
            forget(to.name)
            memory(to).reset()
            destination = to
        }
    }

    fun openTitle(id: AnilistId) {
        opened += 1
        pages += Page.Title(id, "title/$id/$opened")
    }

    /** Close the top page; false when there was none. */
    fun pop(): Boolean {
        val page = pages.removeLastOrNull() ?: return false
        forget(page.key)
        return true
    }
}
