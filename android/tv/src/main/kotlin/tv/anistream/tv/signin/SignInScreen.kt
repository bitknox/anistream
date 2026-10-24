// TVSignIn: signing in to a tracker without a browser. AniList and Kitsu through the TV's own
// sign-in page, opened on a phone from a QR code; Simkl and Trakt with the service's device code.
// MyAnimeList only signs in from a paired phone.

package tv.anistream.tv.signin

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.design.Nagare
import tv.anistream.design.ObiSilk
import tv.anistream.store.AccountsModel
import tv.anistream.store.AppStore
import tv.anistream.tv.remote.LocalRemoteHost
import tv.anistream.tv.remote.QrCode
import tv.anistream.tv.remote.localAddress
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.TonalButton

/** How [tracker] signs in on a TV. */
enum class SignInWay { Page, DeviceCode, PhoneOnly }

fun signInWay(tracker: String): SignInWay = when (tracker) {
    "anilist", "kitsu" -> SignInWay.Page
    "simkl", "trakt" -> SignInWay.DeviceCode
    else -> SignInWay.PhoneOnly
}

/** A device code read in groups of four, as the brief shows it: "8F3K 2QPL". */
fun spaced(code: String): String = code.chunked(4).joinToString(" ")

/** TVSignIn for [tracker], until it is signed in ([onDone]) or Back. */
@Composable
fun SignInScreen(tracker: String, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val remote = LocalRemoteHost.current
    val name = AccountsModel.name(tracker)
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val done by rememberUpdatedState(onDone)
    val wasSignedIn = remember { trackers[tracker]?.connected == true }
    val signedIn = trackers[tracker]?.connected == true
    LaunchedEffect(signedIn) {
        if (signedIn && !wasSignedIn) {
            val user = trackers[tracker]?.user
            remote?.say(if (user != null) "Signed in to $name as $user" else "Signed in to $name")
            done()
        }
    }
    BackHandler(onBack = onDone)

    val cancel = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { cancel.requestFocus() } }
    Row(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Text("Sign in to $name", style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk)
            when (signInWay(tracker)) {
                SignInWay.Page -> PageWay(store, tracker, name)
                SignInWay.DeviceCode -> DeviceCodeWay(store, tracker)
                SignInWay.PhoneOnly -> Text(
                    "$name signs in from a paired anistream phone, with the account the phone uses.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = ObiSilk.Haze,
                    modifier = Modifier.widthIn(max = 640.dp),
                )
            }
            TonalButton(onClick = onDone, modifier = Modifier.focusRequester(cancel)) { ButtonLabel("Cancel") }
        }
    }
}

/** AniList and Kitsu: the TV's own page, its QR code and address, while the screen is open. */
@Composable
private fun PageWay(store: AppStore, tracker: String, name: String) {
    val context = LocalContext.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val request by store.accounts.signIn.collectAsStateWithLifecycle()
    val anilist = tracker == "anilist"
    // AniList's sign-in starts in the engine, which gives the address to open and the state.
    LaunchedEffect(tracker) { if (anilist) store.connect(tracker) }
    val asked = request?.takeIf { it.tracker == tracker }
    val ready = !anilist || asked != null
    var link by remember { mutableStateOf<String?>(null) }
    var sent by remember { mutableStateOf(false) }
    DisposableEffect(ready) {
        if (!ready) return@DisposableEffect onDispose {}
        val page = SignInPage(
            tracker = name,
            form = if (anilist) PageForm.Code else PageForm.Password,
            authorizeUrl = asked?.url,
        ) { answer ->
            scope.launch {
                sent = true
                when (answer) {
                    is PageAnswer.Code -> asked?.let { request ->
                        val state = request.url.toUri().getQueryParameter("state")
                        store.finishSignIn(request, anilistCallback(answer.text, request.callbackScheme, state))
                    }
                    is PageAnswer.Password -> store.connectKitsu(answer.username, answer.password)
                }
            }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val address = localAddress(context)
            link = if (port != null && address != null) "http://$address:$port/${page.path}" else null
        }
        onDispose {
            job.cancel()
            page.stop()
            // Closed before AniList came back: the engine stops waiting for it.
            if (anilist) store.accounts.signIn.value?.takeIf { it.tracker == tracker }?.let(store::cancelSignIn)
        }
    }
    val steps = if (anilist) {
        "Scan with your phone. Open AniList there, allow anistream, then paste the code on this page."
    } else {
        "Scan with your phone, then enter your Kitsu user name and password on the page."
    }
    Text(steps, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = link
        if (shown != null) {
            QrCode(shown, label = "QR code for the sign-in page")
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 520.dp)) {
            Text("Or open this address", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            Text(shown ?: "Starting", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            Text(
                if (sent) "Signing in" else "The page closes with this screen, or in 10 minutes.",
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Haze,
            )
        }
    }
}

/** Simkl and Trakt: the service's code and its address, while the engine polls. */
@Composable
private fun DeviceCodeWay(store: AppStore, tracker: String) {
    val prompt by store.accounts.deviceCode.collectAsStateWithLifecycle()
    LaunchedEffect(tracker) { store.connect(tracker) }
    val shown = prompt?.takeIf { it.tracker == tracker }
    Text(
        "On your phone, open the address below or scan the code, then enter this code.",
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Haze,
        modifier = Modifier.widthIn(max = 640.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        if (shown != null) {
            QrCode(shown.url, label = "QR code for the activation address")
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                shown?.code?.let(::spaced) ?: "Asking for a code",
                style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                color = ObiSilk.Silk,
            )
            shown?.let { Text(it.url, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze) }
        }
    }
}
