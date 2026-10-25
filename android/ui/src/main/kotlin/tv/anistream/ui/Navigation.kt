// Where the screens go: each destination's own back stack with title pages pushed on it, and the
// shared elements that carry a cover or banner into its title page (T1).

package tv.anistream.ui

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.layout.ContentScale
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import tv.anistream.design.Motion
import tv.anistream.engine.core.AnilistId

/** A title page's route inside [destination]'s back stack. */
internal fun titleRoute(destination: Destination, id: AnilistId) = "${destination.route}/title/$id"

/** The route pattern of the title page in [destination]'s back stack. */
internal fun titlePattern(destination: Destination) = "${destination.route}/title/{id}"

/** The graph that holds [destination]'s back stack. */
internal fun graphRoute(destination: Destination) = "${destination.route}-tab"

/** The top-level destination an entry belongs to, by the graph it sits in. */
internal fun NavBackStackEntry.destinationOf(): Destination? =
    destination.parent?.route?.removeSuffix("-tab")?.let(Destination::named)

/**
 * Whether the entry is a page beyond the five destinations, Settings and what it opens, which
 * sits outside every destination's graph.
 */
internal val NavBackStackEntry.isPage: Boolean get() = destinationOf() == null

/**
 * Whether the entry takes the whole window, with no navigation bar or rail: Settings, as
 * AndroidSettings and AndroidTabletSettings draw it.
 */
internal val NavBackStackEntry.isFullWindow: Boolean
    get() = isPage && destination.route?.startsWith(SETTINGS_ROUTE) == true

/**
 * What the screens navigate with: the destinations, title pages, and which element a title page
 * was opened from, so that only that element flies into the page and back.
 */
@Stable
class Navigator(private val nav: NavHostController, private val start: Destination) {
    /**
     * The element a title page was last opened from, as `<where>:<id>`: only it carries the shared
     * keys, since the same title can show twice on one screen and a key must be unique there.
     */
    var source: String? by mutableStateOf(null)
        private set

    /**
     * The title a fold put on screen (AT3): folding a tablet layout with a title selected opens
     * its page, and unfolding again goes back to the list and detail it came from.
     */
    var openedByFold: AnilistId? = null

    /** The destination on screen. */
    fun current(): Destination = nav.currentBackStackEntry?.destinationOf() ?: start

    /** Open [id]'s page in the current destination's back stack, flying from [from]. */
    fun openTitle(id: AnilistId, from: String) {
        source = "$from:$id"
        nav.navigate(titleRoute(current(), id))
    }

    /**
     * Switch to [destination] as it was left; on the current one, back to its start, scrolled to
     * the top by the screen.
     */
    fun go(destination: Destination) {
        if (destination == current()) {
            nav.popBackStack(destination.route, inclusive = false)
            return
        }
        nav.navigate(graphRoute(destination)) {
            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
    }

    fun back() {
        nav.popBackStack()
    }

    /** Open Settings at [pane], or at its list. */
    fun openSettings(pane: SettingsPane? = null) {
        nav.navigate(settingsRoute(pane)) { launchSingleTop = true }
    }

    /** Push the page at [route] over whatever is on screen. */
    fun open(route: String) {
        nav.navigate(route) { launchSingleTop = true }
    }
}

val LocalNavigator = staticCompositionLocalOf<Navigator> { error("no Navigator in this composition") }

/** The layout the shared elements fly in, around the NavHost. */
@OptIn(ExperimentalSharedTransitionApi::class)
val LocalSharedTransitions = compositionLocalOf<SharedTransitionScope?> { null }

/** The NavHost entry a screen is drawn in, which its shared elements animate with. */
val LocalPageAnimation = compositionLocalOf<AnimatedVisibilityScope?> { null }

/** The three things that fly into a title page, by the keys motion-android.md gives them. */
enum class Shared(internal val prefix: String) {
    /**
     * The page's bounds, growing out of the element tapped. They fly as two layers, the page's
     * ground and its content, so that a banner flying with them draws between the two.
     */
    Page("page"),

    /** The banner art. */
    Art("art"),

    /** The poster, drawn above the flying art. */
    Cover("cover"),

    /** The player's bounds, growing out of the banner it was opened from and closing into it (T2, T3). */
    Player("player"),
}

/**
 * This element as one end of [shared]'s flight for [id]. On a source screen pass [from], where
 * the element is (`hero`, `season` …): it joins only when it is the one the page was opened from.
 * On the title page pass none.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun Modifier.shared(shared: Shared, id: AnilistId, from: String? = null, ground: Color = Color.Unspecified): Modifier {
    val layout = LocalSharedTransitions.current
    val page = LocalPageAnimation.current
    if (layout == null || page == null) return if (ground.isSpecified) background(ground) else this
    if (from != null && LocalNavigator.current.source != "$from:$id") return this
    return with(layout) {
        val state = rememberSharedContentState("${shared.prefix}-$id")
        when (shared) {
            // One sharedBounds draws all it holds at one depth in the overlay, so the page flies
            // as two: its ground ([ground], painted on the title page) and, nested in it, its
            // content. The art then draws between them, under the title and the scrim. What the
            // page grows out of draws under the ground, which covers it as the page fades in.
            Shared.Page -> sharedBounds(
                rememberSharedContentState("ground-$id"),
                page,
                enter = fadeIn(Motion.fadeIn()),
                exit = fadeOut(Motion.fadeOut()),
                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.TopCenter),
                zIndexInOverlay = GROUND_Z,
            )
                .then(if (ground.isSpecified) Modifier.background(ground) else Modifier)
                .sharedBounds(
                    state,
                    page,
                    enter = fadeIn(Motion.fadeIn()),
                    exit = fadeOut(Motion.fadeOut()),
                    resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.TopCenter),
                    zIndexInOverlay = if (from != null) SOURCE_Z else 0f,
                )
            Shared.Player -> sharedBounds(
                state,
                page,
                enter = fadeIn(Motion.fadeIn()),
                exit = fadeOut(Motion.fadeOut()),
                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.Center),
            )
            Shared.Art -> sharedElement(state, page, zIndexInOverlay = ART_Z)
            // The poster overlaps the banner, so it draws above the flying art.
            Shared.Cover -> sharedElement(state, page, zIndexInOverlay = COVER_Z)
        }
    }
}

/**
 * The page's own [ground] behind what this holds, except while the page flies: then the flight's
 * ground layer paints it, under the art, and this one would hide the art.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun Modifier.pageGround(ground: Color): Modifier {
    val layout = LocalSharedTransitions.current
    // Read as the layer draws, so the flight starting and ending redraws without recomposing.
    return drawBehind { if (layout?.isTransitionActive != true) drawRect(ground) }
}

// Depths in the overlay, back to front: the source the page grows out of, the ground, the art,
// the page's content, the poster.
private const val SOURCE_Z = -3f
private const val GROUND_Z = -2f
private const val ART_Z = -1f
private const val COVER_Z = 1f
