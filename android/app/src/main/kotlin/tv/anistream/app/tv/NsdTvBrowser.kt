// The TVs on the network, as Android's DNS-SD finds them: `_anistream._tcp` services, each
// followed for its addresses, port and TXT record, handed to the engine's browse.

package tv.anistream.app.tv

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.util.Log
import java.util.concurrent.Executor
import tv.anistream.engine.ffi.FfiDiscoverySink
import tv.anistream.engine.ffi.FfiTvBrowser
import tv.anistream.engine.ffi.FfiTxtPair

/**
 * A browse for the engine's sender. The engine starts it while a picker is up and stops it after;
 * nothing runs between, so it costs nothing while Play on TV is not being chosen.
 */
class NsdTvBrowser(context: Context) : FfiTvBrowser {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private val main: Executor = context.mainExecutor

    private var discovery: NsdManager.DiscoveryListener? = null
    private val following = mutableMapOf<String, NsdManager.ServiceInfoCallback>()

    override fun start(sink: FfiDiscoverySink) = main.execute {
        if (discovery != null || nsd == null) return@execute
        val listener = object : NsdManager.DiscoveryListener {
            override fun onServiceFound(info: NsdServiceInfo) = follow(info, sink)
            override fun onServiceLost(info: NsdServiceInfo) {
                unfollow(info.serviceName)
                sink.lost(info.serviceName)
            }
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.w(TAG, "the TV browse did not start: $errorCode")
                main.execute { if (discovery === this) discovery = null }
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
        }
        discovery = listener
        nsd.discoverServices(SERVICE, NsdManager.PROTOCOL_DNS_SD, null, main, listener)
    }

    override fun stop() = main.execute {
        val listener = discovery ?: return@execute
        discovery = null
        runCatching { nsd?.stopServiceDiscovery(listener) }
        for (name in following.keys.toList()) unfollow(name)
    }

    /** Follow a service found for what it resolves to, and each change to it, until it goes. */
    private fun follow(info: NsdServiceInfo, sink: FfiDiscoverySink) = main.execute {
        val nsd = nsd ?: return@execute
        if (discovery == null || info.serviceName in following) return@execute
        val callback = object : NsdManager.ServiceInfoCallback {
            override fun onServiceUpdated(resolved: NsdServiceInfo) {
                val addresses = resolved.hostAddresses.mapNotNull { it.hostAddress?.substringBefore('%') }
                if (addresses.isEmpty() || resolved.port == 0) return
                val txt = resolved.attributes.map { (key, value) -> FfiTxtPair(key, value?.decodeToString().orEmpty()) }
                sink.found(resolved.serviceName, addresses, resolved.port.toUShort(), txt)
            }
            override fun onServiceLost() = sink.lost(info.serviceName)
            override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {
                Log.w(TAG, "a TV did not resolve: $errorCode")
            }
            override fun onServiceInfoCallbackUnregistered() {}
        }
        following[info.serviceName] = callback
        runCatching { nsd.registerServiceInfoCallback(info, main, callback) }
            .onFailure { following.remove(info.serviceName) }
    }

    private fun unfollow(name: String) {
        val callback = following.remove(name) ?: return
        runCatching { nsd?.unregisterServiceInfoCallback(callback) }
    }

    companion object {
        private const val TAG = "anistream"

        /** The remote's DNS-SD service, as the TV advertises it. */
        const val SERVICE = "_anistream._tcp"
    }
}
