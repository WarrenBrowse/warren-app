package com.warrenbrowse.vpn.app.service

import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.ParcelFileDescriptor
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni

/**
 * Every platform and native call [WarrenQuinnAdapter] makes, behind one seam.
 *
 * The adapter's fail-closed ordering (a TUN, live or blocking, always holds the
 * routes; the live fd is closed only once its successor is established) cannot
 * be observed through `VpnService` and `WarrenJni` directly: the first is an
 * Android inner-class builder and the second an `object` that loads
 * `libwarren_jni.so` at init, so neither exists off-device. Routing both
 * through this interface is what lets `WarrenQuinnAdapterTest` record the ORDER
 * of the handover sequence. Inverting that order is the one mistake in this
 * class that turns an outage into a leak, so it is pinned by a test.
 */
interface WarrenTunnelPlatform {
    /**
     * Apply [plan] to a fresh `VpnService.Builder` and establish it. Returns
     * null when the platform refused the interface; the caller must then keep
     * whatever interface is currently up rather than close it.
     */
    fun establish(plan: WarrenTunInterfacePlan): ParcelFileDescriptor?

    /**
     * Start the native session on [tunFd] (ownership transfers to the native
     * side). Returns 0 on success, negative on a synchronous failure; may throw
     * the native-thrown `RuntimeException` family.
     */
    fun connectTunnel(tunFd: Int, mnemonic: String, configJson: String): Int

    /** Stop the native session. No-op when none is running. */
    fun disconnectTunnel()

    /**
     * Report an underlying-network handover to the native migration watchdog,
     * which rebinds the live QUIC endpoint and revalidates the path in about
     * one RTT instead of re-handshaking. When it cannot, the watchdog ends the
     * session and the status goes `Disconnected`, which is the adapter's
     * signal to run its own handover fallback.
     */
    fun notifyNetworkChanged()

    fun tunnelStatus(): Int

    fun natPmpStatus(): String

    fun autoRecoveryCount(): Int

    /** Engine datapath verdict; see the `PATH_HEALTH_*` constants. */
    fun pathHealth(): Int

    /** Waits for the previous session to finish winding down. */
    fun awaitTunnelClosed(timeoutMs: Long): Boolean

    /**
     * Watch for underlying-network changes. Returns false when the platform
     * denied the registration, in which case the handover path is inert.
     */
    fun registerNetworkCallback(callback: ConnectivityManager.NetworkCallback): Boolean

    fun unregisterNetworkCallback(callback: ConnectivityManager.NetworkCallback)
}

/** The production [WarrenTunnelPlatform]: real `VpnService`, real JNI. */
class AndroidTunnelPlatform(
    private val vpnService: VpnService,
    private val connectivityManager: ConnectivityManager,
) : WarrenTunnelPlatform {

    override fun establish(plan: WarrenTunInterfacePlan): ParcelFileDescriptor? {
        val builder = vpnService.Builder()
            .setSession(plan.session)
            .setMtu(plan.mtu)
        plan.addresses.forEach {
            try {
                builder.addAddress(it.address, it.prefixLength)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid address ${it.address}/${it.prefixLength}" }
            }
        }
        plan.routes.forEach {
            try {
                builder.addRoute(it.address, it.prefixLength)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid route ${it.address}/${it.prefixLength}" }
            }
        }
        plan.dnsServers.forEach {
            try {
                builder.addDnsServer(it)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid DNS server $it" }
            }
        }
        // Split tunnelling: route excluded apps outside the tunnel. A package
        // that is no longer installed throws NameNotFoundException; skip it
        // rather than abort the whole interface. Never on a blocking plan (its
        // excludedApps is empty), so the kill switch always captures everything.
        plan.excludedApps.forEach { pkg ->
            try {
                builder.addDisallowedApplication(pkg)
            } catch (e: PackageManager.NameNotFoundException) {
                Logger.w(throwable = e) { "skipping excluded app not installed: $pkg" }
            }
        }
        // establish() validates the full config in the system process and
        // throws IllegalArgumentException ("Cannot set address") on a rejected
        // combination (e.g. an IPv6 address with MTU < the 1280 v6 minimum).
        // Fail closed by returning null - the caller routes that into
        // onSessionDown - instead of letting it crash the VpnService.
        return try {
            builder.establish()
        } catch (e: IllegalArgumentException) {
            Logger.e(throwable = e) { "VpnService.Builder.establish() rejected the plan" }
            null
        }
    }

    override fun connectTunnel(tunFd: Int, mnemonic: String, configJson: String): Int =
        WarrenJni.connectTunnel(vpnService, tunFd, mnemonic, configJson)

    override fun disconnectTunnel() = WarrenJni.disconnectTunnel()

    override fun notifyNetworkChanged() = WarrenJni.notifyNetworkChanged()

    override fun tunnelStatus(): Int = WarrenJni.getTunnelStatus()

    override fun natPmpStatus(): String = WarrenJni.getNatPmpStatus()

    override fun autoRecoveryCount(): Int = WarrenJni.getAutoRecoveryCount()

    override fun pathHealth(): Int = WarrenJni.getPathHealth()

    override fun awaitTunnelClosed(timeoutMs: Long): Boolean =
        WarrenJni.awaitTunnelClosed(timeoutMs.toInt()) == 1

    override fun registerNetworkCallback(
        callback: ConnectivityManager.NetworkCallback
    ): Boolean {
        // NET_CAPABILITY_NOT_VPN keeps our own TUN out of the stream, so the
        // interface we just established is never read as a handover.
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()
        return try {
            connectivityManager.registerNetworkCallback(request, callback)
            true
        } catch (e: SecurityException) {
            Logger.w(throwable = e) {
                "registerNetworkCallback denied (missing permission?); handover reconnect disabled"
            }
            false
        }
    }

    override fun unregisterNetworkCallback(callback: ConnectivityManager.NetworkCallback) {
        try {
            connectivityManager.unregisterNetworkCallback(callback)
        } catch (e: IllegalArgumentException) {
            Logger.w(throwable = e) {
                "unregisterNetworkCallback failed (callback was not registered)"
            }
        }
    }
}

/** Paired goodput probes deliver at both size classes. */
const val PATH_HEALTH_HEALTHY = 0

/**
 * Large probes die while small ones survive: a last-mile shrink. Deliberately
 * NOT a wedge, it has its own MSS-clamp / PMTU handling and a re-dial cannot
 * cure it.
 */
const val PATH_HEALTH_DEGRADED_LARGE = 1

/** Both probe sizes die while the session stays up: a wedged datapath. */
const val PATH_HEALTH_DEGRADED_BOTH = 2

/**
 * The in-tunnel egress probe's verdict: the exit answers the client and
 * forwards nothing to the internet. Kept distinct from [PATH_HEALTH_DEGRADED_BOTH]
 * because the goodput prober stays green on this class (the exit answers the
 * tunnel gateway it echoes off), so a log that conflated the two could not tell
 * a client-side wedge from a broken exit. Both read as wedged to the UI.
 */
const val PATH_HEALTH_EGRESS_DEAD = 3
