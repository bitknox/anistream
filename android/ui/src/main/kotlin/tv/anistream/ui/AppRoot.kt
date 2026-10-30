// The app's frame: the navigation bar on a phone, the rail on a tablet or an unfolded foldable,
// and the five destinations behind it, each with its own back stack of title pages.

package tv.anistream.ui

import androidx.annotation.StringRes
import tv.anistream.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
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
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.launch
import tv.anistream.design.Motion
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.PlayerCommand
import tv.anistream.store.AppStore
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.NoticeHint
import tv.anistream.engine.model.SearchQuery

/** The top-level destinations, in the navigation bar's order. */
enum class Destination(val route: String, @StringRes val label: Int, val icon: ImageVector, val selectedIcon: ImageVector) {
    Continue("continue", R.string.nav_continue, Icons.Continue, Icons.ContinueSelected),
    Calendar("calendar", R.string.nav_calendar, Icons.Calendar, Icons.CalendarSelected),
    Seasonal("seasonal", R.string.nav_seasonal, Icons.Seasonal, Icons.SeasonalSelected),
    Library("library", R.string.nav_library, Icons.Library, Icons.LibrarySelected),
    Search("search", R.string.nav_search, Icons.Search, Icons.SearchSelected);

    companion object {
        /** The destination [name] names, by its route, case aside. */
        fun named(name: String): Destination? =
            entries.firstOrNull { it.route.equals(name, ignoreCase = true) }
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
    /** Play an episode from its first frame, whatever position it kept: Play from the beginning. */
    val playFromStart: (id: AnilistId, episode: String) -> Unit = { _, _ -> },
    /**
     * What a notice's button asks for: the stores, the player. Kitsu's sign-in [AppRoot] opens
     * itself.
     */
    val hint: (NoticeHint) -> Unit = {},
    /**
     * What Settings' Notifications pane asks of the app: turning new-episode notifications on,
     * which asks for the permission to post first, and the system's own settings for them. `null`
     * turns the setting itself, as the screenshots do.
     */
    val notifications: NotificationActions? = null,
)

/** The app's side of the Notifications pane, and of a show's own New episodes switch. */
class NotificationActions(
    /** Turn new-episode notifications on, once they can show, or off. */
    val turn: (Boolean) -> Unit,
    /** Whether Android did not let them go on, or stay on: the switch's line says so. */
    val refused: StateFlow<Boolean>,
    /**
     * Let notifications show, then do what it is handed: at once where they can, after asking for
     * the permission the first time, and not at all where Android says no. What switching a show
     * on for itself goes through, as Settings' switch does.
     */
    val permit: (then: () -> Unit) -> Unit = { it() },
    /** The system's settings for the channel: sound, vibration and the lock screen. */
    val openSystemSettings: () -> Unit,
)

/** The app's notification actions, for a show's own switch: `null` where there is no app, as in the screenshots. */
val LocalNotificationActions = staticCompositionLocalOf<NotificationActions?> { null }

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
    /** A search to run on Search as the app starts: the fixtures' `query` and `genre` extras. */
    startSearch: SearchQuery? = null,
    /** A sheet to open on [start] as the app starts: the fixtures' `sheet` extra, `filters` on Search. */
    startSheet: String? = null,
    /** A page to open over [start] as the app starts, by its route: the fixtures' `page` extra. */
    startPage: String? = null,
    /** The player, which the frame shows over everything while it is up. */
    player: PlayerSlot? = null,
    /** The links the system opens the app on, followed as they come: shortcuts, notifications. */
    links: Flow<AppLink> = emptyFlow(),
    /** The destination on screen, each time it changes: where the Updated notification comes back to. */
    onSection: (Destination) -> Unit = {},
    /** Whether first launch's pages may show, until this device has been through them. */
    firstLaunch: Boolean = false,
) {
    val nav = rememberNavController()
    val navigator = remember(nav) { Navigator(nav, start) }
    val sheets = remember { AppSheets() }
    // Whether Search is to open its field as it shows: a link to Search asked for it.
    var searchOpening by remember { mutableStateOf(false) }
    val routed = remember(actions, navigator, store) {
        actions.copy(
            openSettings = { navigator.openSettings() },
            connectAniList = { store.connect("anilist") },
            addStore = { store.stores.openJoin() },
            hint = { hint ->
                when (hint) {
                    NoticeHint.KITSU_CREDENTIALS -> sheets.kitsu = true
                    NoticeHint.OPEN_STORES -> navigator.open(STORES_ROUTE)
                    // The engine stops whatever plays, or whatever it is still finding.
                    NoticeHint.STOP_PLAYBACK -> store.send(Command.Player(PlayerCommand.Stop))
                    else -> actions.hint(hint)
                }
            },
        )
    }
    val entry by nav.currentBackStackEntryAsState()
    val current = entry?.destinationOf()
    val sectionChanged by rememberUpdatedState(onSection)
    androidx.compose.runtime.LaunchedEffect(current) { current?.let(sectionChanged) }
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
        LocalNotificationActions provides actions.notifications,
    ) {
        EngineNotices(onHint = routed.hint)
        SourcePinNotice()
        UpdatePrompts()
        AccountSheets()
        TvSheetHost()
        DiscussionHost()
        JoinStoreSheetHost()
        // Two links in a row: the later one's sheet replaces the earlier, never stacks on it.
        val joining by store.stores.joining.collectAsStateWithLifecycle()
        val tvSheet by store.tv.sheet.collectAsStateWithLifecycle()
        androidx.compose.runtime.LaunchedEffect(joining) { if (joining != null) store.tv.sheetClosed() }
        androidx.compose.runtime.LaunchedEffect(tvSheet) { if (tvSheet != null) store.stores.closeJoin() }
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
        val following by rememberUpdatedState(routed)
        androidx.compose.runtime.LaunchedEffect(links, navigator) {
            // A new link takes over from one still waiting to play.
            links.collectLatest { link -> navigator.follow(link, store, following) { searchOpening = true } }
        }
        // First launch's pages stand over the app, which TalkBack then does not reach.
        var covered by remember { mutableStateOf(false) }
        Scaffold(
            modifier = if (covered) Modifier.clearAndSetSemantics {} else Modifier,
            containerColor = MaterialTheme.colorScheme.surface,
            contentWindowInsets = WindowInsets(0),
            // The player says its own notices on the video; the snackbars wait for it to close.
            snackbarHost = {
                if (!onPlayer) {
                    androidx.compose.foundation.layout.Column {
                        UpdateNotice(hidden = entry?.destination?.route?.startsWith(SETTINGS_ROUTE) == true)
                        SnackbarHost(snackbars.host) { NoticeSnackbar(it) }
                    }
                }
            },
            bottomBar = {
                androidx.compose.foundation.layout.Column {
                    if (!onPlayer) TvNowPlayingBar()
                    if (!wide && !onPage && !onPlayer) PhoneBar(current ?: start, navigator::go)
                }
            },
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
                                            DestinationScreen(destination, routed, startSearch, startSheet, searchOpening) { searchOpening = false }
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
        if (firstLaunch) FirstLaunch(onCover = { covered = it })
        EngineDown()
    }
}

@Composable
private fun DestinationScreen(
    destination: Destination,
    actions: AppActions,
    startSearch: SearchQuery?,
    startSheet: String?,
    searchOpening: Boolean,
    searchOpened: () -> Unit,
) {
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
        Destination.Search -> SearchScreen(startSearch, startFilters = startSheet == "filters", open = searchOpening, opened = searchOpened)
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
        modifier = Modifier.fillMaxWidth(),
        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        maxLines = 1,
        softWrap = false,
        // At the smallest size a longer label ends in an ellipsis rather than running off the bar.
        overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
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
                label = { BarLabel(stringResource(destination.label)) },
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
internal enum class DrawerPage(@StringRes val label: Int, val icon: ImageVector, val route: String) {
    Downloads(R.string.nav_downloads, Icons.DownloadsPage, DOWNLOADS_ROUTE),
    Providers(R.string.sources_title, Icons.Providers, PROVIDERS_ROUTE),
    Stores(R.string.nav_plugin_stores, Icons.Stores, STORES_ROUTE),
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
                val collapseLabel = stringResource(if (expanded) R.string.nav_collapse else R.string.nav_expand)
                IconButton(
                    onClick = { scope.launch { rail.toggle() } },
                    modifier = Modifier.semantics { contentDescription = collapseLabel },
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
                    label = { Text(stringResource(destination.label)) },
                    railExpanded = expanded,
                    colors = itemColors,
                )
            }
            if (expanded) {
                val downloads = DrawerPage.Downloads
                DrawerItem(downloads, route, itemColors) { close(); navigator.open(downloads.route) }
                Text(
                    stringResource(R.string.sources_title),
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
                    label = { Text(stringResource(R.string.nav_accounts_and_settings)) },
                    railExpanded = true,
                    colors = itemColors,
                )
            }
        }
        if (!expanded) {
            val accountsAndSettings = user?.let { stringResource(R.string.nav_accounts_and_settings_user, it) } ?: stringResource(R.string.nav_accounts_and_settings)
            IconButton(
                onClick = actions.openSettings,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 16.dp)
                    .semantics { contentDescription = accountsAndSettings },
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
        label = { Text(stringResource(page.label)) },
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
