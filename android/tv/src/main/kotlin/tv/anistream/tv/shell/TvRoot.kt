// The TV app's root: the browse UI (the rail over the current destination or page) or, while
// something plays, the player alone, with the browse screens disposed. Back follows the brief's
// table at every level.

package tv.anistream.tv.shell

import tv.anistream.store.text
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import tv.anistream.store.UpdateState
import tv.anistream.tv.home.LocalWatchNext
import tv.anistream.tv.home.WatchNext
import tv.anistream.tv.player.LocalAddStore
import tv.anistream.tv.player.TvPlayerScreen
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.remote.PairRequestDialog
import tv.anistream.tv.remote.PairScreen
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
import tv.anistream.tv.screens.UpdateScreen
import tv.anistream.tv.signin.JoinStoreScreen
import tv.anistream.tv.signin.SignInScreen
import tv.anistream.tv.signin.TorrentScreen
import tv.anistream.tv.ui.LocalNotices
import tv.anistream.tv.ui.LocalReducedMotion
import tv.anistream.tv.ui.NoticeView
import tv.anistream.tv.ui.ShownNotice
import tv.anistream.tv.ui.TvNotices
import tv.anistream.tv.ui.rememberReducedMotion
import tv.anistream.tv.ui.ActionsHost
import tv.anistream.tv.ui.ActionsPanel
import tv.anistream.tv.ui.KeyRepeat
import tv.anistream.tv.ui.LocalActions
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.LocalKeyRepeat
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.platform.LocalContext
import tv.anistream.i18n.Strings

/** The rail's scrim over the content: black at 32 %. */
private const val SCRIM = 0.32f

@Composable
fun TvRoot(
    store: AppStore,
    player: PlayerModel,
    nav: TvNavigator,
    play: (AnilistId, UInt) -> Unit,
    playFromStart: (AnilistId, UInt) -> Unit = play,
    playerScreen: @Composable (PlayerModel) -> Unit = { TvPlayerScreen(it) },
    remote: TvRemoteHost? = null,
    watchNext: WatchNext? = null,
    firstLaunch: Boolean = false,
) {
    val repeat = remember { KeyRepeat() }
    val states = rememberSaveableStateHolder()
    val scope = rememberCoroutineScope()
    val notices = remember { TvNotices(scope) }
    var covered by remember { mutableStateOf(firstLaunch) }
    // With no source that can play, Continue's hero and a failed playback offer a plugin store
    // instead: Add closes the player, if it is up, onto the join page.
    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val addStore: (() -> Unit)? = if (providersLoaded && providers.none { it.usable }) {
        remember(player, nav) {
            {
                if (player.isPresented) player.viewerClosed()
                nav.openJoinStore()
            }
        }
    } else {
        null
    }
    CompositionLocalProvider(
        LocalAppStore provides store,
        LocalKeyRepeat provides repeat,
        LocalRemoteHost provides remote,
        LocalWatchNext provides watchNext,
        LocalNotices provides notices,
        LocalReducedMotion provides rememberReducedMotion(),
        LocalAddStore provides addStore,
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
            } else if (!covered) {
                states.SaveableStateProvider("browse") { Browse(nav, play, playFromStart) }
            }
            if (firstLaunch && !player.isPresented) FirstLaunch(onCover = { covered = it })
            if (remote != null) RemoteOverlays(remote, playing = player.isPresented, nav = nav)
            Notices(store, player, remote, notices, onUpdatePage = nav.top is Page.Update)
        }
    }
}

/**
 * The one notice surface, top right: the engine's notices, the player's alerts and the remote's
 * lines, each said for its time. A phone asking to pair during playback holds its line there. An
 * update that failed is not said again over its own page, which says why already.
 */
@Composable
private fun BoxScope.Notices(store: AppStore, player: PlayerModel, remote: TvRemoteHost?, notices: TvNotices, onUpdatePage: Boolean) {
    val current by store.notices.current.collectAsStateWithLifecycle()
    val context = androidx.compose.ui.platform.LocalContext.current
    LaunchedEffect(current?.id) {
        val item = current ?: return@LaunchedEffect
        val failure = (store.updates.state.value as? UpdateState.Failed)?.failure?.text
        val text = item.notice.text(context)
        if (!(onUpdatePage && item.notice.msg == null && item.notice.text == failure)) notices.say(text)
        store.notices.dismiss(item)
    }
    // A source pinned for a title that can't be used: said once, whichever page is up.
    val pin by store.sourceChoices.pinNotice.collectAsStateWithLifecycle()
    LaunchedEffect(pin?.serial) {
        val said = pin ?: return@LaunchedEffect
        notices.say(tv.anistream.i18n.Strings.sourcePinUnavailable(context, said.name))
        store.sourceChoices.told(said)
    }
    val alert = player.alert
    LaunchedEffect(alert?.since) { alert?.let { notices.say(it.text) } }
    // A newer release, when the engine says to tell of it: the update's page is in Settings › About.
    // Held while something plays, as a phone asking to pair is, and said once it stops.
    val update by store.updates.notice.collectAsStateWithLifecycle()
    LaunchedEffect(update, player.isPresented) {
        if (player.isPresented) return@LaunchedEffect
        val offer = update ?: return@LaunchedEffect
        notices.say(tv.anistream.i18n.Strings.updatesAvailableTv(context, offer.name))
        store.updates.told(offer)
    }
    val said = remote?.notices ?: 0
    LaunchedEffect(said) { if (said > 0) remote?.notice?.let(notices::say) }
    val request = remote?.request
    val held = if (request != null && player.isPresented) {
        ShownNotice(-1, androidx.compose.ui.res.stringResource(tv.anistream.i18n.R.string.tv_pair_request_held, request.phone, grouped(request.code)))
    } else {
        null
    }
    NoticeView(held ?: notices.shown, Modifier.align(Alignment.TopEnd))
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
private fun Browse(nav: TvNavigator, play: (AnilistId, UInt) -> Unit, playFromStart: (AnilistId, UInt) -> Unit) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val rail = remember { RailFocus() }
    val screens = rememberSaveableStateHolder()
    SideEffect { nav.forget = screens::removeState }
    val open = drawer.currentValue == DrawerValue.Open
    val actions = remember { ActionsHost() }
    SideEffect { actions.onClosed = { nav.focus.enter() } }

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
        CompositionLocalProvider(LocalActions provides actions) {
            Box(Modifier.fillMaxSize()) {
                val reduced = LocalReducedMotion.current
                AnimatedContent(
                    targetState = shown,
                    transitionSpec = {
                        if (reduced) EnterTransition.None togetherWith ExitTransition.None else fadeIn(Motion.quick()) togetherWith fadeOut(Motion.quick())
                    },
                    label = "screen",
                ) { target ->
                    val key = (target as? Page)?.key ?: (target as Destination).name
                    screens.SaveableStateProvider(key) {
                        Box(Modifier.fillMaxSize()) {
                            when (target) {
                                is Page.Title -> TitleScreen(
                                    target.id,
                                    target.memory,
                                    onOpen = nav::openTitle,
                                    onPlay = play,
                                    onPlayFromStart = playFromStart,
                                    onAddStore = nav::openJoinStore,
                                    onSeason = nav::switchTitle,
                                )
                                is Page.SignIn -> SignInScreen(target.tracker, onDone = { nav.pop() })
                                is Page.JoinStore -> JoinStoreScreen(onDone = { nav.pop() })
                                is Page.Torrents -> TorrentScreen(target.mode, target.start, onDone = { nav.pop() })
                                is Page.Update -> UpdateScreen(target.memory, onDone = { nav.pop() })
                                Destination.Continue -> ContinueScreen(nav, nav.memory(target as Destination), onOpen = nav::openTitle, onPlay = play)
                                Destination.Search -> SearchScreen(nav, nav.memory(Destination.Search), onOpen = nav::openTitle)
                                Destination.Calendar -> CalendarScreen(nav, nav.memory(Destination.Calendar), onOpen = nav::openTitle)
                                Destination.Seasonal -> SeasonalScreen(nav, nav.memory(Destination.Seasonal), onOpen = nav::openTitle)
                                Destination.Library -> LibraryScreen(
                                    nav,
                                    nav.memory(Destination.Library),
                                    onOpen = nav::openTitle,
                                    onAccounts = { nav.goSettings(SettingsSection.Accounts.name) },
                                )
                                Destination.Settings -> key(nav.settingsStart) {
                                    val start = nav.settingsStart?.let { name -> SettingsSection.entries.firstOrNull { it.name == name } }
                                    SettingsScreen(nav, nav.memory(Destination.Settings), start = start ?: SettingsSection.General)
                                }
                            }
                        }
                    }
                }
                ActionsPanel(actions, Modifier.align(Alignment.CenterEnd))
            }
        }
    }
}
