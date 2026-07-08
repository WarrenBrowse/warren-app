package com.warrenbrowse.vpn.app.connectivity

import android.net.ConnectivityManager
import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.talpid.util.UnderlyingConnectivityStatusResolver
import com.warrenbrowse.talpid.util.hasInternetConnectivity
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import java.net.DatagramSocket
import java.util.concurrent.atomic.AtomicReference
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

/**
 * Process-wide truthful connectivity source, shared by the connect/retry
 * policy (via [connectivity]) and the UI honesty surfaces (via
 * [hostOffline]). Wraps `hasInternetConnectivity`, which checks the
 * default network's link addresses directly, or, when the default network
 * is a VPN (ours), probes each IP family through a protected socket on the
 * underlying network.
 *
 * The socket protector is only meaningful while our `VpnService` runs: the
 * service registers `VpnService.protect` on create and clears it on
 * destroy. With no VPN up the resolver path is not taken (the default
 * network is physical), so a missing protector never produces a lying
 * verdict for Warren's own tunnel.
 */
class WarrenConnectivityMonitor(
    connectivityManager: ConnectivityManager,
    scope: CoroutineScope,
) : WarrenHostOfflineProvider {

    private val protector = AtomicReference<((DatagramSocket) -> Boolean)?>(null)

    /** Install the live `VpnService.protect` bridge (service onCreate). */
    fun registerSocketProtector(protect: (DatagramSocket) -> Boolean) {
        protector.set(protect)
    }

    /** Drop the bridge when the service dies (service onDestroy). */
    fun clearSocketProtector() {
        protector.set(null)
    }

    /**
     * Live connectivity with IP-family detail, undebounced beyond the
     * source's own settling: the retry policy wants the online edge as
     * promptly as possible.
     */
    val connectivity: StateFlow<Connectivity> =
        connectivityManager
            .hasInternetConnectivity(
                UnderlyingConnectivityStatusResolver { socket ->
                    protector.get()?.invoke(socket) ?: false
                }
            )
            .stateIn(scope, SharingStarted.Eagerly, Connectivity.PresumeOnline)

    /**
     * Host-offline verdict for the UI, held [HOST_OFFLINE_SHOW_DELAY] on
     * the rising edge (network handovers synthesize sub-second offline
     * blips); the falling edge (back online) applies immediately.
     */
    override val hostOffline: StateFlow<Boolean> =
        connectivity
            .map { it is Connectivity.Offline }
            .holdRisingEdge(HOST_OFFLINE_SHOW_DELAY)
            .stateIn(scope, SharingStarted.Eagerly, false)

    private companion object {
        // Same horizon as the desktop useHostOffline debounce.
        val HOST_OFFLINE_SHOW_DELAY = 1200.milliseconds
    }
}
