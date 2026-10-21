// The app's frame on a phone: the navigation bar, and the five destinations behind it.

package tv.anistream.ui

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.ShortNavigationBarItemDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import tv.anistream.design.Motion
import tv.anistream.store.AppStore

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

/** What the setup card's actions open, which later phases fill in. */
data class AppActions(
    /** Connect AniList: a Custom Tab through the engine's sign-in, from A2. */
    val connectAniList: () -> Unit = {},
    /** Join a plugin store: the join sheet, from A2. */
    val addStore: () -> Unit = {},
    /** Accounts and settings: the settings page, from A2. */
    val openSettings: () -> Unit = {},
)

/**
 * The phone's frame: [Destination]s behind the Expressive short navigation bar, which never moves
 * and never takes part in a transition. A switch between destinations fades through on the quick
 * spring (T6).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AppRoot(store: AppStore, start: Destination = Destination.Continue, actions: AppActions = AppActions()) {
    val nav = rememberNavController()
    val entry by nav.currentBackStackEntryAsState()
    val current = entry?.destination?.route?.let(Destination::named) ?: start
    fun go(destination: Destination) {
        nav.navigate(destination.route) {
            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
    }
    CompositionLocalProvider(LocalAppStore provides store) {
        Scaffold(
            containerColor = MaterialTheme.colorScheme.surface,
            contentWindowInsets = WindowInsets(0),
            bottomBar = {
                ShortNavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer) {
                    for (destination in Destination.entries) {
                        val selected = destination == current
                        ShortNavigationBarItem(
                            selected = selected,
                            onClick = { if (!selected) go(destination) },
                            icon = {
                                Icon(
                                    if (selected) destination.selectedIcon else destination.icon,
                                    contentDescription = null,
                                    // The filled icons carry their own two colours.
                                    tint = if (selected && destination.selectedIcon != destination.icon) {
                                        Color.Unspecified
                                    } else {
                                        androidx.compose.material3.LocalContentColor.current
                                    },
                                )
                            },
                            label = { Text(destination.label) },
                            colors = ShortNavigationBarItemDefaults.colors(
                                selectedIconColor = MaterialTheme.colorScheme.onPrimaryContainer,
                                selectedTextColorTopIconPosition = MaterialTheme.colorScheme.primary,
                                selectedIndicatorColor = MaterialTheme.colorScheme.primaryContainer,
                                unselectedIconColor = MaterialTheme.colorScheme.onSurfaceVariant,
                                unselectedTextColor = MaterialTheme.colorScheme.onSurfaceVariant,
                            ),
                        )
                    }
                }
            },
        ) { padding ->
            NavHost(
                nav,
                startDestination = start.route,
                modifier = Modifier.padding(padding).consumeWindowInsets(padding),
                enterTransition = { fadeIn(Motion.quick()) },
                exitTransition = { fadeOut(Motion.quick()) },
                popEnterTransition = { fadeIn(Motion.quick()) },
                popExitTransition = { fadeOut(Motion.quick()) },
            ) {
                composable(Destination.Continue.route) {
                    ContinueScreen(actions, openCalendar = { go(Destination.Calendar) }, openSeasonal = { go(Destination.Seasonal) })
                }
                composable(Destination.Calendar.route) { NotYet(Destination.Calendar) }
                composable(Destination.Seasonal.route) { SeasonalScreen() }
                composable(Destination.Library.route) { NotYet(Destination.Library, account = actions.openSettings) }
                composable(Destination.Search.route) { NotYet(Destination.Search) }
            }
        }
    }
}
