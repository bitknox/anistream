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
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.material3.ModalWideNavigationRail
import androidx.compose.material3.WideNavigationRailDefaults
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.material3.WideNavigationRailItemDefaults
import androidx.compose.material3.WideNavigationRailValue
import androidx.compose.material3.rememberWideNavigationRailState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.material3.LocalTextStyle
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import kotlinx.coroutines.launch
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
    /** Connect AniList: the engine's sign-in in a Custom Tab, which [AppRoot] starts itself. */
    val connectAniList: () -> Unit = {},
    /** Join a plugin store: the join sheet, which [AppRoot] opens itself. */
    val addStore: () -> Unit = {},
    /** Accounts and settings: Settings, which [AppRoot] opens itself. */
    val openSettings: () -> Unit = {},
    /** Play an episode: the player, from A3. */
    val play: (id: AnilistId, episode: String) -> Unit = { _, _ -> },
    /**
     * What a notice's button asks for: the stores, the player. Kitsu's sign-in [AppRoot] opens
     * itself.
     */
    val hint: (NoticeHint) -> Unit = {},
)

/**
 * The player the app hosts, as the frame shows it: a destination of its own over the navigation
 * bar, grown from the banner it was opened from (T2) and closing back into it (T3).
 */
@Stable
interface PlayerSlot {
    /** Whether the player is up, which puts its destination on screen. */
    val presented: Boolean

    /** The title it plays: the banner it grows from and closes into. */
    val title: AnilistId?

    /** The player itself, filling the screen. */
    @Composable
    fun Content()
}

/** The player's route: outside the destinations' graphs, over whichever is on screen. */
const val PLAYER_ROUTE = "player"

/** Whether the window is wide enough for the rail: 600 dp and up, a tablet or an unfolded foldable. */
@Composable
fun wideWindow(): Boolean = LocalConfiguration.current.screenWidthDp >= 600

/**
 * Whether a NavHost change switches destinations rather than pushing inside one. A page beyond
 * them, Settings and what it opens, fades through as a switch does, going and coming back. The
 * player belongs to no destination: it is pushed over one, as a title page is.
 */
private fun AnimatedContentTransitionScope<NavBackStackEntry>.switching(): Boolean {
    if (initialState.destination.route == PLAYER_ROUTE || targetState.destination.route == PLAYER_ROUTE) {
        return false
    }
    return initialState.destinationOf() != targetState.destinationOf() || initialState.isPage || targetState.isPage
}

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
    /** A page to open over [start] as the app starts, by its route: the fixtures' `page` extra. */
    startPage: String? = null,
    /** The player, which the frame shows over everything while it is up. */
    player: PlayerSlot? = null,
) {
    val nav = rememberNavController()
    val navigator = remember(nav) { Navigator(nav, start) }
    val sheets = remember { AppSheets() }
    val routed = remember(actions, navigator, store) {
        actions.copy(
            openSettings = { navigator.openSettings() },
            connectAniList = { store.connect("anilist") },
            addStore = { store.stores.openJoin() },
            hint = { hint ->
                when (hint) {
                    NoticeHint.KITSU_CREDENTIALS -> sheets.kitsu = true
                    NoticeHint.OPEN_STORES -> navigator.open(STORES_ROUTE)
                    else -> actions.hint(hint)
                }
            },
        )
    }
    val entry by nav.currentBackStackEntryAsState()
    val current = entry?.destinationOf()
    val onPage = entry?.isPage == true
    val fullWindow = entry?.isFullWindow == true
    val onPlayer = entry?.destination?.route == PLAYER_ROUTE
    val scope = rememberCoroutineScope()
    val snackbars = remember { Snackbars(SnackbarHostState(), scope) }
    val wide = wideWindow()

    CompositionLocalProvider(
        LocalAppStore provides store,
        LocalNavigator provides navigator,
        LocalSnackbars provides snackbars,
        LocalAppSheets provides sheets,
    ) {
        EngineNotices(onHint = routed.hint)
        AccountSheets()
        JoinStoreSheetHost()
        androidx.compose.runtime.LaunchedEffect(startTitle) {
            startTitle?.let { navigator.openTitle(it, "launch") }
        }
        androidx.compose.runtime.LaunchedEffect(startPage) {
            startPage?.let(navigator::open)
        }
        // The player comes up over whatever is on screen, and goes back to it as it closes.
        if (player != null) {
            androidx.compose.runtime.LaunchedEffect(player.presented) {
                val there = nav.currentBackStackEntry?.destination?.route == PLAYER_ROUTE
                if (player.presented && !there) nav.navigate(PLAYER_ROUTE) { launchSingleTop = true }
                if (!player.presented && there) nav.popBackStack()
            }
        }
        Scaffold(
            containerColor = MaterialTheme.colorScheme.surface,
            contentWindowInsets = WindowInsets(0),
            snackbarHost = { SnackbarHost(snackbars.host) },
            bottomBar = { if (!wide && !onPage && !onPlayer) PhoneBar(current ?: start, navigator::go) },
        ) { padding ->
            Row(Modifier.padding(padding).consumeWindowInsets(padding)) {
                if (wide && !fullWindow && !onPlayer) Rail(current, entry?.destination?.route, navigator, routed)
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
                            if (player != null) {
                                composable(PLAYER_ROUTE) {
                                    CompositionLocalProvider(LocalPageAnimation provides this) {
                                        val id = player.title
                                        Box(Modifier.fillMaxSize().then(if (id != null) Modifier.shared(Shared.Player, id) else Modifier)) {
                                            player.Content()
                                        }
                                    }
                                }
                            }
                            for (destination in Destination.entries) {
                                navigation(route = graphRoute(destination), startDestination = destination.route) {
                                    composable(destination.route) {
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            DestinationScreen(destination, routed, startQuery)
                                        }
                                    }
                                    composable(
                                        titlePattern(destination),
                                        arguments = listOf(navArgument("id") { type = NavType.LongType }),
                                    ) { page ->
                                        val id = page.arguments?.getLong("id")?.toUInt() ?: return@composable
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            TitleScreen(id, routed)
                                        }
                                    }
                                }
                            }
                            settingsPages(routed)
                            providerPages(routed)
                            storePages()
                            downloadPages(routed)
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

/** A navigation bar label on one line, shrinking to fit as the text grows. */
@Composable
private fun BarLabel(text: String) {
    val style = LocalTextStyle.current
    Text(
        text,
        maxLines = 1,
        softWrap = false,
        autoSize = androidx.compose.foundation.text.TextAutoSize.StepBased(
            minFontSize = 8.sp,
            maxFontSize = if (style.fontSize.isSpecified) style.fontSize else 12.sp,
        ),
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
                // One line however large the text: five labels share the width, so each shrinks
                // to fit before it would break mid-word.
                label = { BarLabel(destination.label) },
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
 * The drawer's pages beyond the five destinations (AT1): Downloads, then Providers and Plugin
 * stores under a Sources header.
 */
internal enum class DrawerPage(val label: String, val icon: ImageVector, val route: String) {
    Downloads("Downloads", Icons.DownloadsPage, DOWNLOADS_ROUTE),
    Providers("Providers", Icons.Providers, PROVIDERS_ROUTE),
    Stores("Plugin stores", Icons.Stores, STORES_ROUTE),
}

/**
 * The tablet's rail: 96 wide on surface, the five destinations, and the account at its foot,
 * which opens Settings. Its menu button expands it into the modal drawer (AT1), 220 wide on
 * surfaceContainerLow over the scrim, which adds Downloads, the Sources header with Providers and
 * Plugin stores, and "Accounts and settings" at the foot. Back, the scrim, the button or a pick
 * closes it; its motion is the component's own.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Rail(current: Destination?, route: String?, navigator: Navigator, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    val rail = rememberWideNavigationRailState()
    val scope = rememberCoroutineScope()
    val expanded = rail.targetValue == WideNavigationRailValue.Expanded
    val close = { scope.launch { rail.collapse() } }
    val itemColors = WideNavigationRailItemDefaults.colors(
        selectedIconColor = colors.onPrimaryContainer,
        selectedTextColorTopIconPosition = colors.primary,
        selectedTextColorStartIconPosition = colors.primary,
        selectedIndicatorColor = colors.primaryContainer,
        unselectedIconColor = colors.onSurfaceVariant,
        unselectedTextColor = colors.onSurfaceVariant,
    )
    Box(Modifier.fillMaxHeight()) {
        ModalWideNavigationRail(
            state = rail,
            hideOnCollapse = false,
            expandedShape = RoundedCornerShape(topEnd = 16.dp, bottomEnd = 16.dp),
            colors = WideNavigationRailDefaults.colors(
                containerColor = colors.surface,
                modalContainerColor = colors.surfaceContainerLow,
                modalScrimColor = Color.Black.copy(alpha = 0.32f),
            ),
            header = {
                IconButton(
                    onClick = { scope.launch { rail.toggle() } },
                    modifier = Modifier.semantics { contentDescription = if (expanded) "Collapse navigation" else "Expand navigation" },
                ) {
                    Icon(if (expanded) Icons.MenuOpen else Icons.Menu, contentDescription = null)
                }
            },
        ) {
            for (destination in Destination.entries) {
                val selected = destination == current
                WideNavigationRailItem(
                    selected = selected,
                    onClick = {
                        close()
                        navigator.go(destination)
                    },
                    icon = { DestinationIcon(destination, selected) },
                    label = { Text(destination.label) },
                    railExpanded = expanded,
                    colors = itemColors,
                )
            }
            if (expanded) {
                val downloads = DrawerPage.Downloads
                DrawerItem(downloads, route, itemColors) { close(); navigator.open(downloads.route) }
                Text(
                    "Sources",
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(start = 28.dp, top = 8.dp).height(32.dp).semantics { heading() },
                )
                for (page in listOf(DrawerPage.Providers, DrawerPage.Stores)) {
                    DrawerItem(page, route, itemColors) { close(); navigator.open(page.route) }
                }
                Spacer(Modifier.height(40.dp))
                WideNavigationRailItem(
                    selected = false,
                    onClick = {
                        close()
                        actions.openSettings()
                    },
                    icon = { RailAvatar(user, 32) },
                    label = { Text("Accounts and settings") },
                    railExpanded = true,
                    colors = itemColors,
                )
            }
        }
        if (!expanded) {
            IconButton(
                onClick = actions.openSettings,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 16.dp)
                    .semantics { contentDescription = user?.let { "$it, accounts and settings" } ?: "Accounts and settings" },
            ) {
                RailAvatar(user, 40)
            }
        }
    }
}

/** One of the drawer's own pages, selected while it is on screen. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun DrawerItem(page: DrawerPage, route: String?, colors: androidx.compose.material3.NavigationItemColors, onClick: () -> Unit) {
    WideNavigationRailItem(
        selected = route == page.route && !page.route.startsWith(SETTINGS_ROUTE),
        onClick = onClick,
        icon = { Icon(page.icon, contentDescription = null) },
        label = { Text(page.label) },
        railExpanded = true,
        colors = colors,
    )
}

/** The signed-in user's initial on a raised circle, or the account glyph signed out. */
@Composable
private fun RailAvatar(user: String?, size: Int) {
    if (user == null) {
        Icon(Icons.Account, contentDescription = null)
    } else {
        Box(Modifier.size(size.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape), contentAlignment = Alignment.Center) {
            Text(user.take(1).uppercase(), style = if (size >= 40) MaterialTheme.typography.bodyLarge else MaterialTheme.typography.labelLarge)
        }
    }
}
