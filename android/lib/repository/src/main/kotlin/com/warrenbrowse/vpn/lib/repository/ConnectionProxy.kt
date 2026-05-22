package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.TunnelState

// D.4 step 58: ConnectionProxy stripped of its Mullvad daemon ManagementService
// dependency. The Mullvad daemon is dead on Warren — the real connect /
// disconnect / reconnect path goes through `WarrenQuinnConnectInvoker`,
// `WarrenQuinnDisconnectInvoker`, `WarrenQuinnReconnectInvoker` (wired in
// MainActivity), and the live tunnel state surfaces through
// `WarrenTunnelStateProvider.state` (a `StateFlow<String>`) consumed directly
// by feature modules.
//
// ConnectionProxy is kept as a thin compile-shim because ConnectViewModel +
// DeviceRevokedViewModel still wire their disconnect/reconnect/connect button
// handlers to it — at runtime those calls are no-ops while the dead-daemon
// path is being phased out (D.4 step 67+). The `tunnelState` flow emits a
// constant `Disconnected(null)` to keep the UI in a defined state ; consumers
// that need the live Warren tunnel status read `WarrenTunnelStateProvider`
// directly.
@Suppress("UNUSED_PARAMETER", "unused")
class ConnectionProxy {
    val tunnelState: Flow<TunnelState> = flowOf(TunnelState.Disconnected(location = null))

    suspend fun connect(): Either<ConnectError, Boolean> = true.right()

    suspend fun connectWithoutPermissionCheck(): Either<ConnectError, Boolean> = true.right()

    suspend fun disconnect(disconnectReason: DisconnectReason): Either<ConnectError, Boolean> =
        true.right()

    suspend fun reconnect(): Either<ConnectError, Boolean> = true.right()
}
