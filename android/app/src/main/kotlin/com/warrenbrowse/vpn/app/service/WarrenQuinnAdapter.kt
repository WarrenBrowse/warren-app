package com.warrenbrowse.vpn.app.service

import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.ParcelFileDescriptor
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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
    private var activeFd: ParcelFileDescriptor? = null
    private var statusPollJob: Job? = null

    suspend fun connect(config: WarrenTunnelConfig, mnemonic: String) = lock.withLock {
        if (_state.value !is WarrenTunnelState.Disconnected) {
            Logger.w("WarrenQuinnAdapter: connect() called while not disconnected")
            return@withLock
        }
        _state.value = WarrenTunnelState.Connecting
        activeConfig = config

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

        // TODO (D.4 step 3): register ConnectivityManager.NetworkCallback for
        //   handover-triggered reconnect (Backoff::HANDSHAKE = 15 s).
        // TODO (D.4 step 3): observe Rust-side status changes via a callback
        //   channel instead of polling. For now we poll WarrenJni.getTunnelStatus()
        //   from this coroutine and translate to WarrenTunnelState.
        statusPollJob = scope.launch {
            // Placeholder: in a real impl, the Rust side pushes status
            // transitions through a JNI callback.
        }
    }

    suspend fun disconnect() = lock.withLock {
        WarrenJni.disconnectTunnel()
        statusPollJob?.cancel()
        statusPollJob = null
        activeFd?.close()
        activeFd = null
        activeConfig = null
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

    @Suppress("UnusedParameter")
    private fun onNetworkChange(network: Network?) {
        // TODO (D.4): trigger reconnect with Backoff::HANDSHAKE 15 s when
        //   the underlying network handle changes (Wi-Fi <-> cellular).
    }
}
