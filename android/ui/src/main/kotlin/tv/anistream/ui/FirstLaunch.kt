// First launch (spec, First launch): three short pages over the app before it opens on Continue.
// Welcome says what anistream is and that it hosts no content; AniList offers the sign-in; Sources
// takes an invite link to a plugin store, with the own-risk line. Every page can be passed over,
// each step is in Settings afterwards, and the pages never mention torrents. Once they close they
// do not come back; a device that already has an account or a store never sees them.

package tv.anistream.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import tv.anistream.design.Motion
import tv.anistream.design.Nagare
import tv.anistream.store.AppStore
import tv.anistream.store.DevicePreferences
import tv.anistream.store.InviteLinks

/** The pages, in order. */
internal enum class FirstLaunchPage { Welcome, AniList, Sources }

/** The preference that says the pages have been through, on this device. */
internal const val FIRST_LAUNCH_DONE = "firstLaunchDone"

/** How long the pages wait for the engine to say the accounts and stores before deciding. */
private const val DECIDE_MILLIS = 3_000L

/**
 * The pages over the app, while this device has not been through them. Until the engine has said
 * whether anything is set up, the ground shows plain, so the app does not flash under them.
 * [onCover] hears whether they cover the app, which then keeps its own elements from TalkBack.
 */
@Composable
internal fun FirstLaunch(onCover: (Boolean) -> Unit, start: FirstLaunchPage = FirstLaunchPage.Welcome) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val preferences = remember { DevicePreferences.of(context) }
    var done by rememberSaveable { mutableStateOf(preferences.getBoolean(FIRST_LAUNCH_DONE, false)) }
    val phase by store.phase.collectAsStateWithLifecycle()
    // An engine that did not start says so on its own screen; the pages wait for the next launch.
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
    // The stores said means the engine is up and has read what it keeps.
    val known = storesLoaded
    LaunchedEffect(known, waited, setUp) {
        when {
            showing -> {}
            // Set up already, as after an update: the pages have nothing to offer.
            setUp -> finish()
            known || waited -> showing = true
        }
    }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        if (showing) FirstLaunchPages(start, finish)
    }
}

@Composable
private fun FirstLaunchPages(start: FirstLaunchPage, finish: () -> Unit) {
    val store = LocalAppStore.current
    var page by rememberSaveable { mutableStateOf(start) }
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val joining by store.stores.joining.collectAsStateWithLifecycle()
    // Signed in, the next step; a store joined, the app; an invite link opened, its step.
    val anilist = trackers["anilist"]?.connected == true
    LaunchedEffect(anilist) { if (anilist && page == FirstLaunchPage.AniList) page = FirstLaunchPage.Sources }
    LaunchedEffect(stores.isNotEmpty()) { if (stores.isNotEmpty() && page == FirstLaunchPage.Sources) finish() }
    LaunchedEffect(joining) { if (!joining.isNullOrEmpty()) page = FirstLaunchPage.Sources }
    BackHandler(enabled = page != FirstLaunchPage.Welcome) { page = FirstLaunchPage.entries[page.ordinal - 1] }
    AnimatedContent(
        page,
        transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
        label = "first launch",
    ) { shown ->
        when (shown) {
            FirstLaunchPage.Welcome -> WelcomePage { page = FirstLaunchPage.AniList }
            FirstLaunchPage.AniList -> AniListPage(
                signIn = { store.connect("anilist") },
                notNow = { page = FirstLaunchPage.Sources },
            )
            FirstLaunchPage.Sources -> SourcesPage(
                join = { link -> store.stores.openJoin(link) },
                skip = finish,
            )
        }
    }
}

/** Nagare, the name, what anistream is and that it hosts no content, and Continue. */
@Composable
private fun WelcomePage(next: () -> Unit) {
    PageFrame(
        buttons = { Button(onClick = next, modifier = Modifier.fillMaxWidth()) { Text("Continue") } },
    ) {
        Nagare(cell = 5, modifier = Modifier.align(Alignment.CenterHorizontally))
        Text(
            "anistream",
            style = MaterialTheme.typography.headlineMedium,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 24.dp).semantics { heading() },
        )
        PageLine("Keep track of anime and play it from sources you choose.", Modifier.padding(top = 8.dp), center = true)
        PageLine(LEGAL_LINE, Modifier.padding(top = 16.dp), center = true)
    }
}

/** The AniList sign-in, offered and never asked for: Sign in to AniList, or Not now. */
@Composable
private fun AniListPage(signIn: () -> Unit, notNow: () -> Unit) {
    val store = LocalAppStore.current
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val busy = store.accounts.isWorking(working, "anilist")
    PageFrame(
        buttons = {
            Button(onClick = signIn, enabled = !busy, modifier = Modifier.fillMaxWidth()) { WorkLabel("Sign in to AniList", busy) }
            TextButton(
                onClick = notNow,
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text("Not now") }
        },
    ) {
        PageTitle("AniList")
        PageLine("Sign in to AniList to sync your list and where you stopped.", Modifier.padding(top = 8.dp))
        Text(
            "Other trackers are in Accounts.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/**
 * Where sources come from: plugins, from a plugin store joined by invite. The link pasted here
 * opens the join sheet on it, which shows the store before anything is joined.
 */
@Composable
private fun SourcesPage(join: (String) -> Unit, skip: () -> Unit) {
    var link by rememberSaveable { mutableStateOf("") }
    val ready = InviteLinks.looksLikeInvite(link)
    PageFrame(
        buttons = {
            TextButton(
                onClick = skip,
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text("Skip for now") }
        },
    ) {
        PageTitle("Sources")
        PageLine("Sources are plugins, shared through plugin stores by invite.", Modifier.padding(top = 8.dp))
        Row(
            Modifier.fillMaxWidth().padding(top = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SheetField(
                link,
                { link = it },
                "Invite link",
                Modifier.weight(1f),
                keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Go),
                actions = KeyboardActions(onGo = { if (ready) join(link.trim()) }),
            )
            Button(onClick = { join(link.trim()) }, enabled = ready) { Text("Join") }
        }
        Text(
            "Each plugin is someone else's code connecting to someone else's service. Use them at your own risk.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/**
 * A page: its content in a column at most 480 wide, centred on the ground and scrolling when the
 * window is short, with its buttons at the foot.
 */
@Composable
private fun PageFrame(buttons: @Composable ColumnScope.() -> Unit, content: @Composable ColumnScope.() -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 480.dp)
                .fillMaxSize()
                .padding(horizontal = 24.dp, vertical = 24.dp),
        ) {
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()), content = content)
            }
            Column(
                Modifier.fillMaxWidth().padding(top = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                content = buttons,
            )
        }
    }
}

@Composable
private fun PageTitle(text: String) {
    Text(text, style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
}

@Composable
private fun PageLine(text: String, modifier: Modifier = Modifier, center: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = if (center) TextAlign.Center else TextAlign.Start,
        modifier = modifier.fillMaxWidth(),
    )
}
