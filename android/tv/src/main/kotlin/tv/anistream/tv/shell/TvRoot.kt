// The TV app's root: the browse UI (the rail over the current destination or page) or, while
// something plays, the player alone, with the browse screens disposed. Back follows the brief's
// table at every level.

package tv.anistream.tv.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.tv.material3.DrawerValue
import androidx.tv.material3.ModalNavigationDrawer
import androidx.tv.material3.rememberDrawerState
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiScreen
import tv.anistream.player.PlayerModel
import tv.anistream.store.AppStore
import tv.anistream.tv.player.TvPlayerScreen
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.remote.PairRequestDialog
import tv.anistream.tv.remote.PairScreen
import tv.anistream.tv.remote.RemoteNotice
import tv.anistream.tv.remote.TvRemoteHost
import tv.anistream.tv.remote.grouped
import tv.anistream.tv.screens.CalendarScreen
import tv.anistream.tv.screens.ContinueScreen
import tv.anistream.tv.screens.LibraryScreen
import tv.anistream.tv.screens.SearchScreen
import tv.anistream.tv.screens.SeasonalScreen
import tv.anistream.tv.screens.SettingsScreen
import tv.anistream.tv.screens.SettingsSection
import tv.anistream.tv.screens.TitleScreen
import tv.anistream.tv.ui.KeyRepeat
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.LocalKeyRepeat

/** The rail's scrim over the content: black at 32 %. */
private const val SCRIM = 0.32f

@Composable
fun TvRoot(
    store: AppStore,
    player: PlayerModel,
    nav: TvNavigator,
    play: (AnilistId, UInt) -> Unit,
    playerScreen: @Composable (PlayerModel) -> Unit = { TvPlayerScreen(it) },
    remote: TvRemoteHost? = null,
) {
    val repeat = remember { KeyRepeat() }
    val states = rememberSaveableStateHolder()
    CompositionLocalProvider(
        LocalAppStore provides store,
        LocalKeyRepeat provides repeat,
        LocalRemoteHost provides remote,
    ) {
        Box(
            Modifier
                .fillMaxSize()
                .background(ObiSilk.Ground)
                .onPreviewKeyEvent(repeat::saw),
        ) {
            if (player.isPresented) {
                playerScreen(player)
            } else if (remote?.pairing == true) {
                PairScreen(remote, onClose = remote::closePairing)
            } else {
                states.SaveableStateProvider("browse") { Browse(nav, play) }
            }
            if (remote != null) RemoteOverlays(remote, playing = player.isPresented, nav = nav)
        }
    }
}

/**
 * What the remote puts over the TV: a phone's request to pair as a dialog, or during playback as a
 * line in the top right so the episode is not covered; and the screen it reports to the phones.
 */
@Composable
private fun BoxScope.RemoteOverlays(remote: TvRemoteHost, playing: Boolean, nav: TvNavigator) {
    val request = remote.request
    if (request != null && !playing && !remote.pairing) {
        PairRequestDialog(request, onCancel = remote::cancelRequest)
    }
    val line = when {
        request != null && playing -> "${request.phone} wants to pair: code ${grouped(request.code)}"
        else -> remote.notice
    }
    if (line != null) RemoteNotice(line, Modifier.align(Alignment.TopEnd))

    val screen = when {
        playing -> FfiScreen.Player
        else -> when (val top = nav.top) {
            is Page.Title -> FfiScreen.Title(top.id)
            else -> when (nav.destination) {
                Destination.Continue -> FfiScreen.Home
                Destination.Search -> FfiScreen.Search(nav.searchText)
                Destination.Settings -> FfiScreen.Settings
                else -> FfiScreen.Other
            }
        }
    }
    LaunchedEffect(remote.remote, screen) { remote.setScreen(screen) }
}

@Composable
private fun Browse(nav: TvNavigator, play: (AnilistId, UInt) -> Unit) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val rail = remember { RailFocus() }
    val screens = rememberSaveableStateHolder()
    SideEffect { nav.forget = screens::removeState }
    val open = drawer.currentValue == DrawerValue.Open

    // Back from the rail on Continue leaves the app: the handler steps aside for the system's.
    BackHandler(enabled = !(open && nav.pages.isEmpty() && nav.destination == Destination.Continue)) {
        when {
            open -> nav.go(Destination.Continue)
            nav.pop() -> Unit
            nav.toTop?.invoke() == true -> Unit
            else -> rail.open(nav.destination)
        }
    }

    ModalNavigationDrawer(
        drawerState = drawer,
        scrimBrush = SolidColor(Color.Black.copy(alpha = SCRIM)),
        drawerContent = {
            Rail(
                current = nav.destination,
                focus = rail,
                onSelect = { destination ->
                    if (destination == nav.destination && nav.pages.isEmpty()) {
                        nav.focus.enter()
                    } else {
                        nav.go(destination)
                    }
                },
                modifier = Modifier.onPreviewKeyEvent { event ->
                    // Right from the rail returns to the content's last focused item.
                    event.type == KeyEventType.KeyDown && event.key == Key.DirectionRight && nav.focus.enter()
                },
            )
        },
    ) {
        val shown: Any = nav.top ?: nav.destination
        Crossfade(targetState = shown, animationSpec = Motion.fadeIn(), label = "screen") { target ->
            val key = (target as? Page)?.key ?: (target as Destination).name
            screens.SaveableStateProvider(key) {
                Box(Modifier.fillMaxSize()) {
                    when (target) {
                        is Page.Title -> TitleScreen(target.id, target.memory, onOpen = nav::openTitle, onPlay = play)
                        Destination.Continue -> ContinueScreen(nav, nav.memory(target as Destination), onOpen = nav::openTitle, onPlay = play)
                        Destination.Search -> SearchScreen(nav, nav.memory(Destination.Search), onOpen = nav::openTitle)
                        Destination.Calendar -> CalendarScreen(nav, nav.memory(Destination.Calendar), onOpen = nav::openTitle)
                        Destination.Seasonal -> SeasonalScreen(nav, nav.memory(Destination.Seasonal), onOpen = nav::openTitle)
                        Destination.Library -> LibraryScreen(
                            nav,
                            nav.memory(Destination.Library),
                            onOpen = nav::openTitle,
                            onAccounts = { nav.go(Destination.Settings) },
                        )
                        Destination.Settings -> SettingsScreen(nav, nav.memory(Destination.Settings), start = SettingsSection.Playback)
                    }
                }
            }
        }
    }
}
