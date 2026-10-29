// TVSignIn: signing in to a tracker without a browser. AniList and Kitsu through the TV's own
// sign-in page, opened on a phone from a QR code; Simkl and Trakt with the service's device code.
// MyAnimeList has no way in on a TV: its redirect must be the one address it has registered, so
// Accounts leaves it out.

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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
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
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.store.AppStore
import tv.anistream.store.CodeCountdown
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import tv.anistream.tv.ui.LocalNotices
import tv.anistream.tv.remote.QrCode
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.LocalAppStore
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.entryFocus

/** How [tracker] signs in on a TV, or that it does not ([None]). */
enum class SignInWay { Page, DeviceCode, None }

fun signInWay(tracker: String): SignInWay = when (tracker) {
    "anilist", "kitsu" -> SignInWay.Page
    "simkl", "trakt" -> SignInWay.DeviceCode
    else -> SignInWay.None
}

/** A device code read in groups of four, as the brief shows it: "8F3K 2QPL". */
fun spaced(code: String): String = code.chunked(4).joinToString(" ")

/** TVSignIn for [tracker], until it is signed in ([onDone]) or Back. */
@Composable
fun SignInScreen(tracker: String, onDone: () -> Unit, cancelLabel: String? = null) {
    val context = LocalContext.current
    val cancelText = cancelLabel ?: stringResource(R.string.action_cancel)
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val name = AccountsModel.name(tracker)
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val done by rememberUpdatedState(onDone)
    val wasSignedIn = remember { trackers[tracker]?.connected == true }
    val signedIn = trackers[tracker]?.connected == true
    LaunchedEffect(signedIn) {
        if (signedIn && !wasSignedIn) {
            val user = trackers[tracker]?.user
            notices?.say(if (user != null) Strings.tvSigninDoneAs(context, name, user) else Strings.tvSigninDone(context, name))
            done()
        }
    }
    BackHandler(onBack = onDone)

    val cancel = remember { FocusRequester() }
    Row(
        Modifier
            .entryFocus(cancel)
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(stringResource(R.string.accounts_sign_in_to, name), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            when (signInWay(tracker)) {
                SignInWay.Page -> PageWay(store, tracker, name)
                SignInWay.DeviceCode -> DeviceCodeWay(store, tracker, cancel, onCancel = onDone, cancelLabel = cancelText)
                SignInWay.None -> Text(
                    stringResource(R.string.tv_signin_not_on_tv, name),
                    style = MaterialTheme.typography.bodyLarge,
                    color = ObiSilk.Haze,
                    modifier = Modifier.widthIn(max = 640.dp),
                )
            }
            if (signInWay(tracker) != SignInWay.DeviceCode) {
                TonalButton(onClick = onDone, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(cancelText) }
            }
        }
    }
}

/** AniList and Kitsu: the TV's own page, its QR code and address, while the screen is open. */
@Composable
private fun PageWay(store: AppStore, tracker: String, name: String) {
    val context = LocalContext.current
    val pageHost = LocalPageHost.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val request by store.accounts.signIn.collectAsStateWithLifecycle()
    val anilist = tracker == "anilist"
    // AniList's sign-in starts in the engine, which gives the address to open and the state.
    LaunchedEffect(tracker) { if (anilist) store.connect(tracker) }
    val asked = request?.takeIf { it.tracker == tracker }
    val ready = !anilist || asked != null
    var link by remember { mutableStateOf<String?>(null) }
    var sent by remember { mutableStateOf(false) }
    // A sign-in that ran and ended with no account: the answer was turned down. A success closes
    // the screen before this is seen.
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val busy = store.accounts.isWorking(working, tracker)
    var ran by remember { mutableStateOf(false) }
    LaunchedEffect(busy) { if (busy) ran = true }
    val failed = sent && ran && !busy
    DisposableEffect(ready) {
        if (!ready) return@DisposableEffect onDispose {}
        val page = SignInPage(
            context = context,
            tracker = name,
            form = if (anilist) PageForm.Code else PageForm.Password,
            authorizeUrl = asked?.url,
        ) { answer ->
            scope.launch {
                sent = true
                ran = false
                when (answer) {
                    is PageAnswer.Code -> asked?.let { request ->
                        val state = request.url.toUri().getQueryParameter("state")
                        store.finishSignIn(request, anilistCallback(answer.text, request.callbackScheme, state))
                    }
                    is PageAnswer.Password -> store.connectKitsu(answer.username, answer.password)
                    is PageAnswer.Link, is PageAnswer.Torrent -> Unit
                }
            }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val address = pageHost(context)
            link = if (port != null && address != null) page.address(address, port) else null
        }
        onDispose {
            job.cancel()
            page.stop()
            // Closed before AniList came back: the engine stops waiting for it.
            if (anilist) store.accounts.signIn.value?.takeIf { it.tracker == tracker }?.let(store::cancelSignIn)
        }
    }
    val steps = if (anilist) {
        stringResource(R.string.accounts_scan_code, name)
    } else {
        stringResource(R.string.accounts_scan_password, name)
    }
    Text(steps, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = link
        if (shown != null) {
            QrCode(shown, label = stringResource(R.string.tv_signin_qr_page))
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 520.dp)) {
            Text(stringResource(R.string.stores_or_open_address), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            Text(shown ?: stringResource(R.string.downloads_notice_starting), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            // A failure leads with the error's dot, its words in silk: vermilion is never text.
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (failed) Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                Text(
                    when {
                        !sent -> stringResource(R.string.stores_page_closes)
                        failed -> stringResource(R.string.tv_signin_did_not, name)
                        else -> stringResource(R.string.accounts_signing_in_action)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (failed) ObiSilk.Silk else ObiSilk.Haze,
                )
            }
        }
    }
}

/**
 * Simkl and Trakt: the service's code and its address, while the engine polls, with Get a new code
 * and Cancel side by side under it. A code that runs out stays on screen, dimmed, under "Code
 * expired", and focus moves to Get a new code: nothing swaps the code by itself while someone
 * may be typing it.
 */
@Composable
private fun DeviceCodeWay(store: AppStore, tracker: String, again: FocusRequester, onCancel: () -> Unit, cancelLabel: String) {
    val prompt by store.accounts.deviceCode.collectAsStateWithLifecycle()
    LaunchedEffect(tracker) { store.connect(tracker) }
    val shown = prompt?.takeIf { it.tracker == tracker }
    var expired by remember(shown?.code) { mutableStateOf(false) }
    LaunchedEffect(expired) { if (expired) runCatching { again.requestFocus() } }
    Text(
        stringResource(R.string.tv_signin_device_steps),
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Silk,
        modifier = Modifier.widthIn(max = 640.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        if (shown != null) {
            QrCode(shown.url, label = stringResource(R.string.tv_signin_qr_activation))
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val code = shown?.code
            if (code != null) {
                Text(
                    spaced(code),
                    style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                    color = if (expired) ObiSilk.Haze else ObiSilk.Silk,
                )
            } else {
                Text(stringResource(R.string.tv_signin_asking_code), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
            shown?.let { Text(it.url, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze) }
            shown?.let { CodeExpiry(it, onExpired = { expired = true }) }
        }
    }
    // The code waits, so focus rests on asking for a fresh one.
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = { store.connect(tracker) }, modifier = Modifier.focusRequester(again)) { ButtonLabel(stringResource(R.string.action_new_code)) }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/**
 * How long the code has left under it, in the engine's words, ticking each second, then "Code
 * expired" in silk beside an error's dot, when [onExpired] tells the screen. Nothing when the
 * service gave no time.
 */
@Composable
private fun CodeExpiry(prompt: DeviceCodePrompt, onExpired: () -> Unit) {
    val expiresAt = prompt.expiresAt ?: return
    val expired by rememberUpdatedState(onExpired)
    var left by remember(prompt.code, expiresAt) { mutableLongStateOf(CodeCountdown.secondsLeft(expiresAt, System.currentTimeMillis())) }
    LaunchedEffect(prompt.code, expiresAt) {
        CodeCountdown.ticks(expiresAt).collect { left = it }
        expired()
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (left == 0L) Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
        Text(
            CodeCountdown.line(left),
            style = MaterialTheme.typography.labelLarge.copy(fontFeatureSettings = "tnum"),
            color = if (left == 0L) ObiSilk.Silk else ObiSilk.Haze,
        )
    }
}
