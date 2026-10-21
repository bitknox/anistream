// The app's frame: the navigation bar on a phone, the rail on a tablet or an unfolded foldable,
// and the five destinations behind it, each with its own back stack of title pages.

package tv.anistream.ui

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.ShortNavigationBarItemDefaults
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.WideNavigationRail
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.material3.WideNavigationRailItemDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import tv.anistream.design.Motion
import tv.anistream.store.AppStore
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.NoticeHint

/** The top-level destinations, in the navigation bar's order. */
enum class Destination(val route: String, val label: String, val icon: ImageVector, val selectedIcon: ImageVector) {
    Continue("continue", "Continue", Icons.Continue, Icons.ContinueSelected),
    Calendar("calendar", "Calendar", Icons.Calendar, Icons.CalendarSelected),
    Seasonal("seasonal", "Seasonal", Icons.Seasonal, Icons.SeasonalSelected),
    Library("library", "Library", Icons.Library, Icons.Library),
    Search("search", "Search", Icons.Search, Icons.Search);

    companion object {
        /** The destination [name] names, by its route or label, case aside. */
        fun named(name: String): Destination? =
            entries.firstOrNull { it.route.equals(name, ignoreCase = true) || it.label.equals(name, ignoreCase = true) }
    }
}

/** What the screens' actions open, which later phases fill in. */
data class AppActions(
    /** Connect AniList: a Custom Tab through the engine's sign-in, from A2. */
    val connectAniList: () -> Unit = {},
    /** Join a plugin store: the join sheet, from A2. */
    val addStore: () -> Unit = {},
    /** Accounts and settings: the settings page, from A2. */
    val openSettings: () -> Unit = {},
    /** Play an episode: the player, from A3. */
    val play: (id: AnilistId, episode: String) -> Unit = { _, _ -> },
    /** What a notice's button asks for: the stores, the player, Kitsu's sign-in, from A2 and A3. */
    val hint: (NoticeHint) -> Unit = {},
)

/** Whether the window is wide enough for the rail: 600 dp and up, a tablet or an unfolded foldable. */
@Composable
fun wideWindow(): Boolean = LocalConfiguration.current.screenWidthDp >= 600

/** Whether a NavHost change switches destinations rather than pushing inside one. */
private fun AnimatedContentTransitionScope<NavBackStackEntry>.switching(): Boolean =
    initialState.destinationOf() != targetState.destinationOf()

/**
 * The frame. A switch between destinations fades the page through on the quick spring (T6); a
 * title page pushed inside one keeps the page underneath, and the shared elements do the moving
 * (T1), seeked by predictive back.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun AppRoot(
    store: AppStore,
    start: Destination = Destination.Continue,
    actions: AppActions = AppActions(),
    /** A title page to open on [start] as the app starts: the fixtures' `title` extra. */
    startTitle: AnilistId? = null,
    /** A search to run on Search as the app starts: the fixtures' `query` extra. */
    startQuery: String? = null,
) {
    val nav = rememberNavController()
    val navigator = remember(nav) { Navigator(nav, start) }
    val entry by nav.currentBackStackEntryAsState()
    val current = entry?.destinationOf() ?: start
    val scope = rememberCoroutineScope()
    val snackbars = remember { Snackbars(SnackbarHostState(), scope) }
    val wide = wideWindow()

    CompositionLocalProvider(
        LocalAppStore provides store,
        LocalNavigator provides navigator,
        LocalSnackbars provides snackbars,
    ) {
        EngineNotices(onHint = actions.hint)
        androidx.compose.runtime.LaunchedEffect(startTitle) {
            startTitle?.let { navigator.openTitle(it, "launch") }
        }
        Scaffold(
            containerColor = MaterialTheme.colorScheme.surface,
            contentWindowInsets = WindowInsets(0),
            snackbarHost = { SnackbarHost(snackbars.host) },
            bottomBar = { if (!wide) PhoneBar(current, navigator::go) },
        ) { padding ->
            Row(Modifier.padding(padding).consumeWindowInsets(padding)) {
                if (wide) Rail(current, navigator::go, actions)
                SharedTransitionLayout {
                    CompositionLocalProvider(LocalSharedTransitions provides this) {
                        NavHost(
                            nav,
                            startDestination = graphRoute(start),
                            enterTransition = { if (switching()) fadeIn(Motion.quick()) else EnterTransition.None },
                            exitTransition = { if (switching()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
                            popEnterTransition = { if (switching()) fadeIn(Motion.quick()) else EnterTransition.None },
                            popExitTransition = { if (switching()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
                        ) {
                            for (destination in Destination.entries) {
                                navigation(route = graphRoute(destination), startDestination = destination.route) {
                                    composable(destination.route) {
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            DestinationScreen(destination, actions, startQuery)
                                        }
                                    }
                                    composable(
                                        titlePattern(destination),
                                        arguments = listOf(navArgument("id") { type = NavType.LongType }),
                                    ) { page ->
                                        val id = page.arguments?.getLong("id")?.toUInt() ?: return@composable
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            TitleScreen(id, actions)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DestinationScreen(destination: Destination, actions: AppActions, startQuery: String?) {
    val navigator = LocalNavigator.current
    when (destination) {
        Destination.Continue -> ContinueScreen(
            actions,
            openCalendar = { navigator.go(Destination.Calendar) },
            openSeasonal = { navigator.go(Destination.Seasonal) },
        )
        Destination.Calendar -> CalendarScreen()
        Destination.Seasonal -> SeasonalScreen()
        Destination.Library -> LibraryScreen(actions)
        Destination.Search -> SearchScreen(startQuery)
    }
}

/** The filled icons carry their own two colours; the rest take the item's. */
@Composable
private fun DestinationIcon(destination: Destination, selected: Boolean) {
    val filled = selected && destination.selectedIcon != destination.icon
    Icon(
        if (selected) destination.selectedIcon else destination.icon,
        contentDescription = null,
        tint = if (filled) Color.Unspecified else LocalContentColor.current,
    )
}

/**
 * The phone's navigation bar: the Expressive short bar on surfaceContainer, the selected
 * destination in a primaryContainer pill with an amber label. It never moves and never takes part
 * in a transition.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PhoneBar(current: Destination, go: (Destination) -> Unit) {
    val colors = MaterialTheme.colorScheme
    ShortNavigationBar(containerColor = colors.surfaceContainer) {
        for (destination in Destination.entries) {
            val selected = destination == current
            ShortNavigationBarItem(
                selected = selected,
                onClick = { go(destination) },
                icon = { DestinationIcon(destination, selected) },
                label = { Text(destination.label) },
                colors = ShortNavigationBarItemDefaults.colors(
                    selectedIconColor = colors.onPrimaryContainer,
                    selectedTextColorTopIconPosition = colors.primary,
                    selectedIndicatorColor = colors.primaryContainer,
                    unselectedIconColor = colors.onSurfaceVariant,
                    unselectedTextColor = colors.onSurfaceVariant,
                ),
            )
        }
    }
}

/**
 * The tablet's rail: 96 wide on surface, the five destinations, and the account at its foot,
 * which opens Settings. Its menu button will expand it into the modal drawer with Downloads and
 * Sources once those pages are there (A2).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Rail(current: Destination, go: (Destination) -> Unit, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    Box(Modifier.fillMaxHeight()) {
        WideNavigationRail(
            header = {
                IconButton(onClick = {}, enabled = false, modifier = Modifier.semantics { contentDescription = "Expand navigation" }) {
                    Icon(Icons.Menu, contentDescription = null)
                }
            },
        ) {
            for (destination in Destination.entries) {
                val selected = destination == current
                WideNavigationRailItem(
                    selected = selected,
                    onClick = { go(destination) },
                    icon = { DestinationIcon(destination, selected) },
                    label = { Text(destination.label) },
                    railExpanded = false,
                    colors = WideNavigationRailItemDefaults.colors(
                        selectedIconColor = colors.onPrimaryContainer,
                        selectedTextColorTopIconPosition = colors.primary,
                        selectedIndicatorColor = colors.primaryContainer,
                        unselectedIconColor = colors.onSurfaceVariant,
                        unselectedTextColor = colors.onSurfaceVariant,
                    ),
                )
            }
        }
        IconButton(
            onClick = actions.openSettings,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 16.dp)
                .semantics { contentDescription = user?.let { "$it, accounts and settings" } ?: "Accounts and settings" },
        ) {
            if (user == null) {
                Icon(Icons.Account, contentDescription = null)
            } else {
                Box(Modifier.size(40.dp).background(colors.surfaceContainerHighest, CircleShape), contentAlignment = Alignment.Center) {
                    Text(user.take(1).uppercase(), style = MaterialTheme.typography.bodyLarge)
                }
            }
        }
    }
}
