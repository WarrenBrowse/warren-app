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

    private var activeConfig: WarrenTunnelConfig? = null
    private var activeMnemonic: String? = null
    private var activeFd: ParcelFileDescriptor? = null
    private var statusPollJob: Job? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var lastNetwork: Network? = null
    private var pendingHandover: Job? = null

    suspend fun connect(config: WarrenTunnelConfig, mnemonic: String) = lock.withLock {
        if (_state.value !is WarrenTunnelState.Disconnected) {
            Logger.w("WarrenQuinnAdapter: connect() called while not disconnected")
            return@withLock
        }
        _state.value = WarrenTunnelState.Connecting
        activeConfig = config
        // Hold the mnemonic in memory only for the duration of the active
        // session, so the reconnect-on-handover path can re-derive the
        // SigningKey without going back to the wallet repository (which
        // would re-trigger BiometricPrompt mid-handover - bad UX).
        activeMnemonic = mnemonic

        val fd = buildTunInterface(config) ?: run {
            _state.value = WarrenTunnelState.Failed("VpnService.Builder.establish() returned null")
            return@withLock
        }
        activeFd = fd

        val rc = WarrenJni.connectTunnel(fd.detachFd(), mnemonic, Json.encodeToString(config))
        if (rc != 0) {
            _state.value = WarrenTunnelState.Failed("connectTunnel returned $rc")
            return@withLock
        }

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
                    _state.value = statusFromCode(code, sessionConfig)
                    if (code == STATUS_DISCONNECTED || code < 0) {
                        // Rust task ended (clean exit or error). Stop
                        // polling; the next `connect()` will start a fresh
                        // session.
                        break
                    }
                }
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
        unregisterNetworkCallback()
        pendingHandover?.cancel()
        pendingHandover = null
        WarrenJni.disconnectTunnel()
        statusPollJob?.cancel()
        statusPollJob = null
        activeFd?.close()
        activeFd = null
        activeConfig = null
        activeMnemonic = null
        lastNetwork = null
        _state.value = WarrenTunnelState.Disconnected
    }

    private fun buildTunInterface(config: WarrenTunnelConfig): ParcelFileDescriptor? {
        val builder = vpnService.Builder()
            .setSession("Warren VPN")
            .addAddress("10.64.0.1", 32)
            .addAddress("fd00:0:0:0:0:0:0:1", 128)
            .addRoute("0.0.0.0", 0)
            .addRoute("::", 0)
            .setMtu(1280)

        config.bypassCidrs.forEach { cidr ->
            // TODO (D.4): translate CIDR -> addDisallowedApplication or
            //   excludeRoute depending on payload shape.
            Logger.d("WarrenQuinnAdapter: bypass CIDR pending $cidr")
        }

        return builder.establish()
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
            WarrenJni.disconnectTunnel()
            // Backoff::HANDSHAKE = 15 s (cf. warren-core M4.H.G).
            delay(HANDOVER_GRACE_MS)
            // Re-acquire the lock through `connect` itself; first drop
            // local state so it re-initialises cleanly.
            activeFd?.close()
            activeFd = null
            statusPollJob?.cancel()
            statusPollJob = null
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

        /** Backoff::HANDSHAKE = 15 s (cf. warren-core M4.H.G). */
        const val HANDOVER_GRACE_MS = 15_000L
    }
}
