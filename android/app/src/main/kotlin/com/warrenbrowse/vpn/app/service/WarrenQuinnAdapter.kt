package com.warrenbrowse.vpn.app.service

import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.ParcelFileDescriptor
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

// Owns the Warren Quinn tunnel lifecycle for a given `WarrenVpnService`
// instance. Sits between the Android `VpnService` API and the Rust JNI:
//
//   WarrenVpnService -- onStartCommand --> WarrenQuinnAdapter.connect(config)
//                                                |
//                                                v
//                                       VpnService.Builder.establish()
//                                                |
//                                                v
//                                  WarrenJni.connectTunnel(fd, configJson)
//                                                |
//                                                v
//                                  warren_tunnel Quinn pump
//
// State machine + reconnect-on-network-change live here, not in Rust.
//
// D.4 scaffold. The actual builder configuration (DNS, bypass CIDRs,
// multi-hop entry plumbing) is intentionally TODO - the architecture is
// committed; the wiring is the next focused session's work. See
// `.planning/session-d-d4-d7-design.md`.
class WarrenQuinnAdapter(
    private val vpnService: VpnService,
    private val connectivityManager: ConnectivityManager,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()

    private val _state = MutableStateFlow<WarrenTunnelState>(WarrenTunnelState.Disconnected)
    val state: StateFlow<WarrenTunnelState> = _state.asStateFlow()

    // Live NAT-PMP port-forwarding status (raw JSON from the Rust side),
    // polled alongside the tunnel status. `idle` when no mapping is active.
    private val _natPmpStatus = MutableStateFlow(NATPMP_IDLE)
    val natPmpStatus: StateFlow<String> = _natPmpStatus.asStateFlow()

    private var activeConfig: WarrenTunnelConfig? = null
    private var activeMnemonic: String? = null
    private var activeFd: ParcelFileDescriptor? = null
    private var statusPollJob: Job? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var lastNetwork: Network? = null
    private var pendingHandover: Job? = null

    // Kill-switch (lockdown) state. When the tunnel drops while lockdown is
    // enabled we keep `blockingFd` established as a blackhole interface so
    // traffic stays captured instead of leaking to the physical network.
    private var blockingFd: ParcelFileDescriptor? = null
    // Distinguishes a user-requested teardown (release traffic) from an
    // unexpected drop (engage the kill switch).
    private var userInitiatedDisconnect = false

    suspend fun connect(config: WarrenTunnelConfig, mnemonic: String) = lock.withLock {
        if (_state.value !is WarrenTunnelState.Disconnected) {
            Logger.w("WarrenQuinnAdapter: connect() called while not disconnected")
            return@withLock
        }
        _state.value = WarrenTunnelState.Connecting
        userInitiatedDisconnect = false
        activeConfig = config
        // Hold the mnemonic in memory only for the duration of the active
        // session, so the reconnect-on-handover path can re-derive the
        // SigningKey without going back to the wallet repository (which
        // would re-trigger BiometricPrompt mid-handover - bad UX).
        activeMnemonic = mnemonic

        val fd = buildTunInterface(config) ?: run {
            onSessionDown(config, "VpnService.Builder.establish() returned null")
            return@withLock
        }
        activeFd = fd

        val rc = WarrenJni.connectTunnel(fd.detachFd(), mnemonic, Json.encodeToString(config))
        if (rc != 0) {
            onSessionDown(config, "connectTunnel returned $rc")
            return@withLock
        }

        // The real tunnel is being (re)established: drop any blackhole
        // interface left over from a previous lockdown drop.
        exitBlockingMode()

        // Poll the Rust-side session status atomic and translate transitions
        // back to `WarrenTunnelState`. A JNI callback channel would be more
        // elegant (no busy-poll) but requires JVM ref management gymnastics
        // we are deferring to D.4 step 5 follow-up. The poll cadence
        // (250 ms) is fast enough to keep the UI responsive without
        // burning battery: the Rust side updates the atomic only once per
        // transition.
        val sessionConfig = config
        statusPollJob = scope.launch {
            var lastSeen = STATUS_CONNECTING
            while (isActive) {
                val code = WarrenJni.getTunnelStatus()
                if (code != lastSeen) {
                    lastSeen = code
                    if (code == STATUS_DISCONNECTED || code < 0) {
                        // Rust task ended (clean exit or error). Engage the
                        // kill switch on an unexpected drop under lockdown;
                        // otherwise release and stop polling.
                        val reason =
                            if (code < 0) "native status code $code" else "tunnel disconnected"
                        lock.withLock {
                            if (userInitiatedDisconnect) {
                                _state.value = WarrenTunnelState.Disconnected
                            } else {
                                onSessionDown(sessionConfig, reason)
                            }
                        }
                        break
                    }
                    _state.value = statusFromCode(code, sessionConfig)
                }
                // Mirror the live NAT-PMP status (cheap static read).
                val np = WarrenJni.getNatPmpStatus()
                if (np != _natPmpStatus.value) _natPmpStatus.value = np
                delay(STATUS_POLL_INTERVAL_MS)
            }
        }

        // Register a NetworkCallback for handover-triggered reconnect
        // (Wi-Fi <-> cellular <-> ethernet). The reconnect happens
        // off-lock via `triggerHandoverReconnect` so the callback returns
        // immediately to the ConnectivityManager.
        registerNetworkCallback()
    }

    /**
     * Tear down the active Quinn session and immediately reconnect using
     * the cached [activeConfig] + [activeMnemonic]. Used by the user-
     * facing Reconnect button (and by the OS handover flow); does NOT
     * re-prompt for biometric auth because we have the mnemonic still
     * in process memory from the prior connect.
     *
     * No-op when there is no active session; the user must use the
     * normal connect flow in that case.
     */
    suspend fun reconnect() {
        val config = activeConfig
        val mnemonic = activeMnemonic
        if (config == null || mnemonic == null) {
            Logger.w("WarrenQuinnAdapter: reconnect() called without an active session")
            return
        }
        disconnect()
        connect(config, mnemonic)
    }

    suspend fun disconnect() = lock.withLock {
        // User-requested teardown: release traffic instead of engaging the
        // kill switch.
        userInitiatedDisconnect = true
        unregisterNetworkCallback()
        pendingHandover?.cancel()
        pendingHandover = null
        WarrenJni.disconnectTunnel()
        statusPollJob?.cancel()
        statusPollJob = null
        activeFd?.close()
        activeFd = null
        exitBlockingMode()
        activeConfig = null
        activeMnemonic = null
        lastNetwork = null
        _natPmpStatus.value = NATPMP_IDLE
        _state.value = WarrenTunnelState.Disconnected
    }

    private fun buildTunInterface(config: WarrenTunnelConfig): ParcelFileDescriptor? {
        config.bypassCidrs.forEach { cidr ->
            // TODO: translate CIDR -> addDisallowedApplication or
            //   excludeRoute depending on payload shape.
            Logger.d("WarrenQuinnAdapter: bypass CIDR pending $cidr")
        }
        return applyPlan(planTunInterface(config))
    }

    /**
     * Apply a [WarrenTunInterfacePlan] to a fresh `VpnService.Builder` and
     * establish the interface. Invalid routes / DNS servers are skipped
     * rather than aborting the whole tunnel.
     */
    private fun applyPlan(plan: WarrenTunInterfacePlan): ParcelFileDescriptor? {
        val builder = vpnService.Builder()
            .setSession(plan.session)
            .setMtu(plan.mtu)
        plan.addresses.forEach { builder.addAddress(it.address, it.prefixLength) }
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
        return builder.establish()
    }

    /**
     * Handle the active tunnel going down. Must be called holding [lock].
     *
     * When [WarrenTunnelConfig.lockdownMode] is on and the drop was not
     * user-initiated, establish a blackhole interface (kill switch) so
     * traffic stays blocked, then schedule a reconnect. Otherwise surface a
     * [WarrenTunnelState.Failed] and release traffic.
     */
    private fun onSessionDown(config: WarrenTunnelConfig, reason: String) {
        activeFd?.close()
        activeFd = null
        _natPmpStatus.value = NATPMP_IDLE
        if (config.lockdownMode && !userInitiatedDisconnect) {
            enterBlockingMode(config, reason)
            scheduleLockdownReconnect(config)
        } else {
            unregisterNetworkCallback()
            _state.value = WarrenTunnelState.Failed(reason)
        }
    }

    /**
     * Establish (or keep) a kill-switch blackhole interface that captures
     * all traffic but pumps nothing, so it is dropped instead of leaking to
     * the physical network. Must be called holding [lock].
     */
    private fun enterBlockingMode(config: WarrenTunnelConfig, reason: String) {
        if (blockingFd == null) {
            val fd = applyPlan(planTunInterface(config, blocking = true))
            if (fd == null) {
                Logger.e("WarrenQuinnAdapter: failed to establish blocking interface; traffic may leak")
                _state.value = WarrenTunnelState.Failed(reason)
                return
            }
            blockingFd = fd
            Logger.w("WarrenQuinnAdapter: lockdown engaged, traffic blocked ($reason)")
        }
        _state.value = WarrenTunnelState.Blocking(reason)
    }

    /** Tear down the kill-switch blackhole interface, if any. */
    private fun exitBlockingMode() {
        blockingFd?.close()
        blockingFd = null
    }

    /**
     * After a lockdown drop, retry the real tunnel once the grace period
     * elapses. The blackhole interface stays up until [connect] confirms a
     * new tunnel (it calls [exitBlockingMode] on success), so there is no
     * leak window between attempts. Repeated failures re-enter blocking via
     * [onSessionDown], forming a bounded retry loop.
     */
    private fun scheduleLockdownReconnect(config: WarrenTunnelConfig) {
        val mnemonic = activeMnemonic ?: return
        pendingHandover?.cancel()
        pendingHandover = scope.launch {
            delay(HANDOVER_GRACE_MS)
            lock.withLock {
                if (userInitiatedDisconnect) return@withLock
                statusPollJob?.cancel()
                statusPollJob = null
                // Reset so connect()'s guard passes; the blackhole stays up.
                _state.value = WarrenTunnelState.Disconnected
            }
            if (!userInitiatedDisconnect) connect(config, mnemonic)
        }
    }

    private fun registerNetworkCallback() {
        if (networkCallback != null) return
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if (lastNetwork == null) {
                    // First seen network during the session is the
                    // baseline; not a handover.
                    lastNetwork = network
                    return
                }
                if (network != lastNetwork) {
                    Logger.i(
                        "WarrenQuinnAdapter: underlying network changed " +
                            "($lastNetwork -> $network), scheduling reconnect"
                    )
                    lastNetwork = network
                    scheduleHandoverReconnect()
                }
            }

            override fun onLost(network: Network) {
                if (network == lastNetwork) {
                    Logger.i("WarrenQuinnAdapter: underlying network $network lost")
                    lastNetwork = null
                }
            }
        }
        try {
            connectivityManager.registerNetworkCallback(request, callback)
            networkCallback = callback
        } catch (e: SecurityException) {
            Logger.w(throwable = e) {
                "registerNetworkCallback denied (missing permission?); handover reconnect disabled"
            }
        }
    }

    private fun unregisterNetworkCallback() {
        val cb = networkCallback ?: return
        try {
            connectivityManager.unregisterNetworkCallback(cb)
        } catch (e: IllegalArgumentException) {
            Logger.w(throwable = e) {
                "unregisterNetworkCallback failed (callback was not registered)"
            }
        }
        networkCallback = null
    }

    /**
     * Tear down the current Quinn session and re-issue `connectTunnel`
     * with the cached config + mnemonic after a brief grace period. The
     * 15 s wait mirrors warren-core `Backoff::HANDSHAKE` so the new
     * handshake aligns with the exit's expected re-handshake window.
     */
    private fun scheduleHandoverReconnect() {
        val config = activeConfig
        val mnemonic = activeMnemonic
        if (config == null || mnemonic == null) {
            Logger.w("scheduleHandoverReconnect: no active session to reconnect")
            return
        }
        pendingHandover?.cancel()
        pendingHandover = scope.launch {
            _state.value = WarrenTunnelState.Reconnecting
            // Stop polling BEFORE the intentional teardown so the status
            // loop does not observe the DISCONNECTED transition and trip
            // the kill switch (this is an expected handover, not a drop).
            statusPollJob?.cancel()
            statusPollJob = null
            WarrenJni.disconnectTunnel()
            // Backoff::HANDSHAKE = 15 s (cf. warren-core M4.H.G).
            delay(HANDOVER_GRACE_MS)
            // Re-acquire the lock through `connect` itself; first drop
            // local state so it re-initialises cleanly.
            activeFd?.close()
            activeFd = null
            _state.value = WarrenTunnelState.Disconnected
            connect(config, mnemonic)
        }
    }

    private fun statusFromCode(code: Int, config: WarrenTunnelConfig): WarrenTunnelState =
        when (code) {
            STATUS_DISCONNECTED -> WarrenTunnelState.Disconnected
            STATUS_CONNECTING -> WarrenTunnelState.Connecting
            STATUS_CONNECTED ->
                WarrenTunnelState.Connected(
                    exitId = config.exitPubkeyHex,
                    assignedNatPmpPort = null,
                    multiHop = config.entryHop != null,
                    daita = config.daita != null,
                    obfuscationM40 = config.obfuscationM40,
                )
            STATUS_RECONNECTING -> WarrenTunnelState.Reconnecting
            else -> WarrenTunnelState.Failed("native status code $code")
        }

    private companion object {
        const val STATUS_DISCONNECTED = 0
        const val STATUS_CONNECTING = 1
        const val STATUS_CONNECTED = 2
        const val STATUS_RECONNECTING = 3
        const val STATUS_POLL_INTERVAL_MS = 250L
        const val NATPMP_IDLE = "{\"state\":\"idle\"}"

        /** Backoff::HANDSHAKE = 15 s (cf. warren-core M4.H.G). */
        const val HANDOVER_GRACE_MS = 15_000L
    }
}
