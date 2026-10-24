// First launch on TV: the spec's three pages, Welcome, AniList and Sources, laid out for focus.
// Each page has one focused way on and one way past; every step is in Settings afterwards. Once
// through they do not come back, and a TV that already has an account or a store never sees them.

package tv.anistream.tv.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import tv.anistream.design.Nagare
import tv.anistream.design.ObiSilk
import tv.anistream.store.AppStore
import tv.anistream.store.DevicePreferences
import tv.anistream.tv.signin.JoinStoreScreen
import tv.anistream.tv.signin.SignInScreen
import tv.anistream.tv.ui.AmberButton
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.enterFade
import tv.anistream.tv.ui.entryFocus
import tv.anistream.tv.ui.exitFade

/** The pages, in order. */
enum class FirstLaunchPage { Welcome, AniList, Sources }

/** The preference that says the pages have been through, on this TV: the phone's name for it. */
const val FIRST_LAUNCH_DONE = "firstLaunchDone"

/** How long the pages wait for the engine to say the accounts and stores before deciding. */
private const val DECIDE_MILLIS = 3_000L

/**
 * The pages over the app while this TV has not been through them; [onCover] hears whether they
 * cover it. Until the engine has said whether anything is set up, the ground shows plain.
 */
@Composable
fun FirstLaunch(onCover: (Boolean) -> Unit, start: FirstLaunchPage = FirstLaunchPage.Welcome) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val preferences = remember { DevicePreferences.of(context) }
    var done by rememberSaveable { mutableStateOf(preferences.getBoolean(FIRST_LAUNCH_DONE, false)) }
    val phase by store.phase.collectAsStateWithLifecycle()
    val stopped = phase is AppStore.Phase.Failed || phase is AppStore.Phase.Stopped
    val covering = !done && !stopped
    LaunchedEffect(covering) { onCover(covering) }
    if (!covering) return
    val finish = {
        preferences.edit().putBoolean(FIRST_LAUNCH_DONE, true).apply()
        done = true
    }
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val storesLoaded by store.stores.loaded.collectAsStateWithLifecycle()
    var waited by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.loadStores()
        delay(DECIDE_MILLIS)
        waited = true
    }
    val setUp = trackers.values.any { it.connected } || stores.isNotEmpty()
    var showing by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(storesLoaded, waited, setUp) {
        when {
            showing -> {}
            // Set up already, as after an update: the pages have nothing to offer.
            setUp -> finish()
            storesLoaded || waited -> showing = true
        }
    }
    Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
        if (showing) Pages(start, finish)
    }
}

@Composable
private fun Pages(start: FirstLaunchPage, finish: () -> Unit) {
    var page by rememberSaveable { mutableStateOf(start) }
    BackHandler(enabled = page == FirstLaunchPage.Sources) { page = FirstLaunchPage.AniList }
    val enter = enterFade()
    val exit = exitFade()
    AnimatedContent(page, transitionSpec = { enter togetherWith exit }, label = "first launch") { shown ->
        when (shown) {
            FirstLaunchPage.Welcome -> Welcome(onNext = { page = FirstLaunchPage.AniList })
            // Signed in or Not now, Sources follows.
            FirstLaunchPage.AniList -> SignInScreen("anilist", onDone = { page = FirstLaunchPage.Sources }, cancelLabel = "Not now")
            FirstLaunchPage.Sources -> JoinStoreScreen(
                onDone = { finish() },
                cancelLabel = "Skip for now",
                title = "Sources",
                line = "Sources are plugins, shared through plugin stores by invite.",
            )
        }
    }
}

/** Welcome: Nagare, what anistream is, the legal line, and Continue focused. */
@Composable
private fun Welcome(onNext: () -> Unit) {
    val next = remember { FocusRequester() }
    Row(
        Modifier
            .entryFocus(next)
            .fillMaxSize()
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 120.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 6) }
        Column(verticalArrangement = Arrangement.spacedBy(16.dp), modifier = Modifier.widthIn(max = 560.dp)) {
            Text("anistream", style = MaterialTheme.typography.displaySmall, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            Text("Keep track of anime and play it from sources you choose.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Text("anistream hosts no content. Every source is run by someone else.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            AmberButton(onClick = onNext, icon = null, modifier = Modifier.padding(top = 8.dp).focusRequester(next)) { ButtonLabel("Continue") }
        }
    }
}
