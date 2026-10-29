// The TV's side of the remote: phones on the network find it, pair with a code, and drive it. The
// engine runs the protocol; this starts and stops it with the app, advertises it, turns what it
// asks into the TV's own actions, and keeps what the pairing screens show.

package tv.anistream.tv.remote

import tv.anistream.i18n.Strings
import android.content.Context
import android.media.AudioManager
import android.net.ConnectivityManager
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings as SystemSettings
import android.util.Log
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.core.content.edit
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import java.net.Inet4Address
import kotlin.math.roundToInt
import kotlin.math.sign
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import tv.anistream.tv.ui.noticeMillis
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.engine.ffi.FfiPairedPhone
import tv.anistream.engine.ffi.FfiScreen
import tv.anistream.engine.ffi.FfiTvRemote
import tv.anistream.engine.model.FrontIntent
import tv.anistream.store.AppStore

/** This TV's IPv4 address on its network, for QR codes: the pairing link, the sign-in page. */
fun localAddress(context: Context): String? {
    val connectivity = context.getSystemService(ConnectivityManager::class.java) ?: return null
    val links = connectivity.getLinkProperties(connectivity.activeNetwork) ?: return null
    return links.linkAddresses.map { it.address }
        .firstOrNull { it is Inet4Address && !it.isLoopbackAddress }
        ?.hostAddress
}

/** A phone asked to pair: the code to show it, and what it is called. */
data class PairRequest(val code: String, val phone: String)

/** The remote, as the TV's screens see it; `null` in a composition with none, as in the tests. */
val LocalRemoteHost = staticCompositionLocalOf<TvRemoteHost?> { null }

/**
 * The TV's remote, for the life of the process.
 *
 * It listens while the app is on screen and for [BACKGROUND_MILLIS] after it leaves, as the brief
 * says, then stops and withdraws its advert; back on screen, it listens again. "Let phones on this
 * network find this TV" off, it does not listen at all, unless Pair a phone starts it for a while.
 */
@Stable
class TvRemoteHost(
    private val context: Context,
    private val store: AppStore,
    private val scope: CoroutineScope,
) : DefaultLifecycleObserver {
    private val prefs = context.getSharedPreferences("remote", Context.MODE_PRIVATE)

    /** The remote listening now, if any. */
    var remote by mutableStateOf<FfiTvRemote?>(null)
        private set

    /** Whether phones on the network may find this TV: the Remotes switch, on by default. */
    var findable by mutableStateOf(prefs.getBoolean(FINDABLE, true))
        private set

    /** A phone's request to pair, for the dialog, until it pairs or is cancelled. */
    var request by mutableStateOf<PairRequest?>(null)
        private set

    /** Whether the TV's own Pair a phone screen is open, and the code it shows. */
    var pairing by mutableStateOf(false)
        private set
    var ownCode by mutableStateOf<String?>(null)
        private set

    /** The phones paired with this TV, newest first. */
    var phones by mutableStateOf<List<FfiPairedPhone>>(emptyList())
        private set

    /** A line for the top right, such as "Mika's phone is paired", until it times out. */
    var notice by mutableStateOf<String?>(null)
        private set

    /** Counts the lines said, so the same words twice still show again. */
    var notices by mutableIntStateOf(0)
        private set

    /** Whether a phone is typing into Search, for the line under the field. */
    var typing by mutableStateOf(false)

    private val _intents = MutableSharedFlow<FrontIntent>(extraBufferCapacity = 16)

    /** What the browse UI acts on: titles to open, text to type, keys to press. */
    val intents: SharedFlow<FrontIntent> = _intents.asSharedFlow()

    /** Asks for `ACCESS_LOCAL_NETWORK` where the system wants it, set by the activity. */
    var askLocalNetwork: (() -> Unit)? = null

    private var foreground = false
    private var starting = false
    private var stopLater: Job? = null
    private var refresh: Job? = null
    private var noticeJob: Job? = null
    private var registration: NsdManager.RegistrationListener? = null
    private var multicast: WifiManager.MulticastLock? = null

    /** What phones call this TV: the system's device name. */
    val name: String
        get() = runCatching { SystemSettings.Global.getString(context.contentResolver, SystemSettings.Global.DEVICE_NAME) }
            .getOrNull()?.takeIf { it.isNotBlank() } ?: "anistream TV"

    init {
        store.onRemote = ::handle
    }

    override fun onStart(owner: LifecycleOwner) {
        foreground = true
        stopLater?.cancel()
        stopLater = null
        if (findable || pairing) listen()
    }

    override fun onStop(owner: LifecycleOwner) {
        foreground = false
        stopLater?.cancel()
        stopLater = scope.launch {
            delay(BACKGROUND_MILLIS)
            stop()
        }
    }

    /** Turn finding this TV on or off, as the Remotes switch does. */
    fun allowFinding(on: Boolean) {
        findable = on
        prefs.edit { putBoolean(FINDABLE, on) }
        if (on) {
            askLocalNetwork?.invoke()
            if (foreground) listen()
        } else if (!pairing) {
            stop()
        }
    }

    /** Open the TV's Pair a phone screen: a code any phone pairs with, new every five minutes. */
    fun openPairing() {
        pairing = true
        listen()
        refresh?.cancel()
        refresh = scope.launch {
            val listening = remoteWhenListening() ?: return@launch
            while (true) {
                ownCode = listening.openPairing()
                delay(CODE_REFRESH_MILLIS)
            }
        }
    }

    /** Close the Pair a phone screen, and its code with it. */
    fun closePairing() {
        refresh?.cancel()
        refresh = null
        pairing = false
        ownCode = null
        remote?.closePairing()
        if (!findable) stop()
    }

    /** Cancel a phone's request to pair: its code is abandoned. */
    fun cancelRequest() {
        request = null
        remote?.closePairing()
    }

    /** Forget a paired phone, closing its connection. */
    fun remove(phone: FfiPairedPhone) {
        val listening = remote ?: return
        runCatching { listening.revoke(phone.fingerprint) }
        phones = listening.paired()
    }

    /** The link the QR code carries: this TV's address and key, and the code on screen. */
    fun pairLink(code: String?): String? {
        val address = address() ?: return null
        return remote?.pairLink(address, code)
    }

    /** This TV's address and port, for typing into a phone by hand. */
    fun typedAddress(): String? {
        val address = address() ?: return null
        val port = remote?.port() ?: return null
        return "$address:$port"
    }

    /** What the TV has on screen, for the phones' remotes. */
    fun setScreen(screen: FfiScreen) {
        remote?.setScreen(screen)
    }

    private fun listen() {
        if (remote != null || starting) return
        starting = true
        scope.launch {
            store.phase.first { it == AppStore.Phase.Running }
            val started = withContext(Dispatchers.IO) { store.startRemote(name, DEFAULT_PORT) }
            starting = false
            if (started == null) return@launch
            // Switched off, or gone to the background for good, while it started.
            if (!(findable || pairing) || (!foreground && stopLater == null)) {
                started.stop()
                return@launch
            }
            remote = started
            advertise(started)
            phones = started.paired()
            reportVolume()
        }
    }

    private suspend fun remoteWhenListening(): FfiTvRemote? {
        repeat(STARTING_TRIES) {
            remote?.let { return it }
            delay(STARTING_PAUSE_MILLIS)
        }
        return remote
    }

    private fun stop() {
        val listening = remote ?: return
        remote = null
        request = null
        withdraw()
        scope.launch { listening.stop() }
    }

    private fun advertise(listening: FfiTvRemote) {
        val nsd = context.getSystemService(NsdManager::class.java) ?: return
        val advert = listening.advert()
        val info = NsdServiceInfo().apply {
            serviceName = advert.instance
            serviceType = SERVICE_TYPE
            port = advert.port.toInt()
            advert.txt.forEach { setAttribute(it.key, it.value) }
        }
        val listener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                Log.i(TAG, "advertised as ${info.serviceName}")
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {
                Log.i(TAG, "advert withdrawn")
            }
            override fun onRegistrationFailed(info: NsdServiceInfo, error: Int) {
                Log.w(TAG, "advert failed: $error")
            }
            override fun onUnregistrationFailed(info: NsdServiceInfo, error: Int) {
                Log.w(TAG, "withdrawing failed: $error")
            }
        }
        runCatching { nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, listener) }
            .onSuccess { registration = listener }
            .onFailure { Log.w(TAG, "advert not registered", it) }
        // Android 12 and older drop multicast on Wi-Fi without a lock, and the advert with it.
        if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.S_V2) {
            multicast = context.applicationContext.getSystemService(WifiManager::class.java)
                ?.createMulticastLock("anistream-remote")
                ?.apply {
                    setReferenceCounted(false)
                    acquire()
                }
        }
    }

    private fun withdraw() {
        registration?.let { listener ->
            runCatching { context.getSystemService(NsdManager::class.java)?.unregisterService(listener) }
        }
        registration = null
        multicast?.release()
        multicast = null
    }

    private fun handle(intent: FrontIntent) {
        when (intent) {
            is FrontIntent.Volume -> stepVolume(intent.step)
            is FrontIntent.ShowPairingCode -> request = PairRequest(intent.code, intent.phone)
            is FrontIntent.HidePairingCode -> {
                request = null
                // The screen's own code was used up or replaced: open another while it shows.
                if (pairing) ownCode = remote?.openPairing()
            }
            is FrontIntent.Paired -> {
                say(Strings.tvPhonePaired(context, intent.phone))
                phones = remote?.paired().orEmpty()
                if (pairing) closePairing()
            }
            is FrontIntent.Text -> {
                typing = !intent.submit
                _intents.tryEmit(intent)
            }
            else -> _intents.tryEmit(intent)
        }
    }

    /** Show [line] in the top right for a few seconds. */
    fun say(line: String) {
        notice = line
        notices += 1
        noticeJob?.cancel()
        noticeJob = scope.launch {
            delay(noticeMillis(line))
            notice = null
        }
    }

    /**
     * Step the system's own volume by [step] points of 100, at least one of its steps. A box on
     * HDMI keeps its own volume at full and leaves loudness to the TV: there the step goes to the
     * TV over HDMI-CEC as presses of its volume key, one for every 5 points, when CEC allows it.
     */
    private fun stepVolume(step: Double) {
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        if (step == 0.0) return
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        val now = audio.getStreamVolume(AudioManager.STREAM_MUSIC)
        var target = ((now * 100.0 / max + step) * max / 100.0).roundToInt().coerceIn(0, max)
        if (target == now) target = (now + step.sign.toInt()).coerceIn(0, max)
        if (!audio.isVolumeFixed) audio.setStreamVolume(AudioManager.STREAM_MUSIC, target, AudioManager.FLAG_SHOW_UI)
        if (audio.getStreamVolume(AudioManager.STREAM_MUSIC) == now) {
            val direction = if (step > 0) AudioManager.ADJUST_RAISE else AudioManager.ADJUST_LOWER
            repeat((kotlin.math.abs(step) / CEC_STEP).roundToInt().coerceIn(1, CEC_PRESSES_MAX)) {
                audio.adjustStreamVolume(AudioManager.STREAM_MUSIC, direction, AudioManager.FLAG_SHOW_UI)
            }
        }
        reportVolume()
    }

    private fun reportVolume() {
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        remote?.setVolume(audio.getStreamVolume(AudioManager.STREAM_MUSIC) * 100.0 / max)
    }

    private fun address(): String? = localAddress(context)

    companion object {
        const val TAG = "anistream-remote"
        const val SERVICE_TYPE = "_anistream._tcp"
        const val DEFAULT_PORT = 47_811
        /** How long the TV stays findable after the app leaves the screen: 10 minutes. */
        const val BACKGROUND_MILLIS = 10 * 60_000L
        /** A little under the engine's five minutes, so the code on screen never runs out. */
        const val CODE_REFRESH_MILLIS = 290_000L
        private const val FINDABLE = "findable"
        /** Points of a phone's volume step per press of the TV's volume key over HDMI-CEC. */
        private const val CEC_STEP = 5.0
        private const val CEC_PRESSES_MAX = 5
        private const val STARTING_TRIES = 50
        private const val STARTING_PAUSE_MILLIS = 100L
    }
}
