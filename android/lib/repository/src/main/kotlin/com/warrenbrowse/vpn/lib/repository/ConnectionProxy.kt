package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import java.net.InetSocketAddress
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.model.TunnelState

// D.4 step 59 + audit follow-up: ConnectionProxy pipes real tunnel state
// through from WarrenTunnelStateProvider. The proxy still exposes the
// legacy Flow<TunnelState> (Mullvad model) for back-compat with the
// existing in-app-notification + UI plumbing ; the state string from
// WarrenQuinnStateProxy is mapped to the closest matching TunnelState
// enum value.
//
// TunnelState.Connected requires a non-nullable TunnelEndpoint. Warren
// does not surface the live endpoint through this back-compat path
// (richer endpoint info flows via WarrenLocalSettingsRepository +
// WarrenRelayProvider). We construct a sentinel TunnelEndpoint here so
// the type system is satisfied; consumers that actually need endpoint
// details should read them from the Warren-native repositories
// directly.
//
// connect/disconnect/reconnect remain stubs (returning Right(true)) -
// the actual Warren tunnel commands flow through
// WarrenQuinnConnectInvoker / WarrenQuinnDisconnectInvoker /
// WarrenQuinnReconnectInvoker invoked directly by ConnectViewModel +
// DeviceRevokedViewModel (step 60).
class ConnectionProxy(private val tunnelStateProvider: WarrenTunnelStateProvider) {

    private val sentinelEndpoint: TunnelEndpoint = TunnelEndpoint(
        entryEndpoint = null,
        endpoint = Endpoint(
            address = InetSocketAddress("0.0.0.0", 0),
            protocol = TransportProtocol.Udp,
        ),
        quantumResistant = false,
        obfuscation = null,
        daita = false,
    )

    val tunnelState: Flow<TunnelState> =
        tunnelStateProvider.state.map { stateLabel ->
            when {
                stateLabel.startsWith("Connecting") -> TunnelState.Connecting(null, null, emptyList())
                stateLabel.startsWith("Reconnecting") ->
                    TunnelState.Connecting(null, null, emptyList())
                stateLabel.startsWith("Connected") ->
                    TunnelState.Connected(sentinelEndpoint, null, emptyList())
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
