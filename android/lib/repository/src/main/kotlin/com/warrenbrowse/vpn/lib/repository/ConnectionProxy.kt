package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.TunnelState

// D.4 step 59: ConnectionProxy pipes real tunnel state through from
// [WarrenTunnelStateProvider]. The proxy still exposes the legacy
// `Flow<TunnelState>` (Mullvad model) for back-compat with the existing
// in-app-notification + UI plumbing ; the state string from WarrenQuinnState-
// Proxy is mapped to the closest matching `TunnelState` enum value.
//
// connect/disconnect/reconnect remain stubs (returning Right(true)) — the
// actual Warren tunnel commands flow through `WarrenQuinnConnectInvoker` /
// `WarrenQuinnDisconnectInvoker` / `WarrenQuinnReconnectInvoker` invoked
// directly by ConnectViewModel + DeviceRevokedViewModel (step 60).
class ConnectionProxy(private val tunnelStateProvider: WarrenTunnelStateProvider) {
    val tunnelState: Flow<TunnelState> =
        tunnelStateProvider.state.map { stateLabel ->
            when {
                stateLabel.startsWith("Connecting") -> TunnelState.Connecting(null, null, emptyList())
                stateLabel.startsWith("Reconnecting") ->
                    TunnelState.Connecting(null, null, emptyList())
                stateLabel.startsWith("Connected") -> TunnelState.Connected(null, null, emptyList())
                stateLabel.startsWith("Failed") -> TunnelState.Disconnected(location = null)
                else -> TunnelState.Disconnected(location = null)
            }
        }

    suspend fun connect(): Either<ConnectError, Boolean> = true.right()

    suspend fun connectWithoutPermissionCheck(): Either<ConnectError, Boolean> = true.right()

    @Suppress("UNUSED_PARAMETER")
    suspend fun disconnect(disconnectReason: DisconnectReason): Either<ConnectError, Boolean> =
        true.right()

    suspend fun reconnect(): Either<ConnectError, Boolean> = true.right()
}
