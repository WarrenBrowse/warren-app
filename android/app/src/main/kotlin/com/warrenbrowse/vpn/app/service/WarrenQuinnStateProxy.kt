package com.warrenbrowse.vpn.app.service

import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

/**
 * Process-singleton mirror of [WarrenQuinnAdapter.state] so any
 * Composable (or any Koin consumer) can read the current tunnel state
 * without having to bind to the running [WarrenVpnService].
 *
 * The adapter lives inside the service (it needs the `VpnService`
 * reference); the proxy lives in the Koin graph (it has no
 * dependencies). The service forwards every transition into the proxy
 * via a collector spawned in `onCreate`. When the service tears down,
 * the proxy is left at its last value, which is the correct semantic
 * (the home screen shows the last known state until the next connect
 * cycle resets it).
 *
 * Implements [WarrenTunnelStateProvider] so lib-side Composables can
 * subscribe to the state without having to import the app-private
 * [WarrenTunnelState] type (they observe a `String` projection).
 */
class WarrenQuinnStateProxy : WarrenTunnelStateProvider, WarrenNatPmpStatusProvider {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private val _state = MutableStateFlow<WarrenTunnelState>(WarrenTunnelState.Disconnected)
    val tunnelState: StateFlow<WarrenTunnelState> = _state.asStateFlow()

    override val state: StateFlow<String> =
        tunnelState.map { it.describe() }.stateIn(
            scope = scope,
            started = SharingStarted.Eagerly,
            initialValue = WarrenTunnelState.Disconnected.describe(),
        )

    // Lossless typed projection consumed by ConnectionProxy → the main
    // connection card. Carries the real endpoint hosts + feature flags +
    // the distinct Failed/Blocking states (the String projection above
    // flattens these and is kept only for legacy banner display).
    override val connectedInfo: StateFlow<WarrenConnectedInfo> =
        tunnelState.map { it.toConnectedInfo() }.stateIn(
            scope = scope,
            started = SharingStarted.Eagerly,
            initialValue = WarrenConnectedInfo.Disconnected,
        )

    private val _natPmpStatus = MutableStateFlow(NATPMP_IDLE)
    override val natPmpStatus: StateFlow<String> = _natPmpStatus.asStateFlow()

    /** Called by [WarrenVpnService] on every adapter-side transition. */
    fun update(next: WarrenTunnelState) {
        _state.value = next
    }

    /** Called by [WarrenVpnService] on every NAT-PMP status change. */
    fun updateNatPmpStatus(json: String) {
        _natPmpStatus.value = json
    }

    private companion object {
        const val NATPMP_IDLE = "{\"state\":\"idle\"}"
    }

    private fun WarrenTunnelState.toConnectedInfo(): WarrenConnectedInfo = when (this) {
        is WarrenTunnelState.Disconnected -> WarrenConnectedInfo.Disconnected
        is WarrenTunnelState.Connecting -> WarrenConnectedInfo.Connecting
        is WarrenTunnelState.Reconnecting -> WarrenConnectedInfo.Reconnecting
        is WarrenTunnelState.Connected -> WarrenConnectedInfo.Connected(
            exitEndpointHost = exitEndpointHost,
            entryEndpointHost = entryEndpointHost,
            multiHop = multiHop,
            daita = daita,
            assignedNatPmpPort = assignedNatPmpPort,
        )
        is WarrenTunnelState.Failed -> WarrenConnectedInfo.Failed(reason, expired)
        is WarrenTunnelState.Blocking -> WarrenConnectedInfo.Blocking(reason, flapping, expired)
    }

    private fun WarrenTunnelState.describe(): String = when (this) {
        is WarrenTunnelState.Disconnected -> "Disconnected"
        is WarrenTunnelState.Connecting -> "Connecting..."
        is WarrenTunnelState.Connected ->
            "Connected" + listOfNotNull(
                if (multiHop) "multi-hop" else null,
                if (daita) "DAITA" else null,
                // HTTP/3 mimicry is always-on for Warren tunnels (not togglable);
                // surface it unconditionally rather than gating on a setting.
                "mimicry",
                if (assignedNatPmpPort != null) "port $assignedNatPmpPort" else null,
            ).let { features -> if (features.isEmpty()) "" else " (${features.joinToString()})" }
        is WarrenTunnelState.Reconnecting -> "Reconnecting..."
        is WarrenTunnelState.Failed -> "Failed: $reason"
        is WarrenTunnelState.Blocking -> "Blocking internet (kill switch)"
    }
}
