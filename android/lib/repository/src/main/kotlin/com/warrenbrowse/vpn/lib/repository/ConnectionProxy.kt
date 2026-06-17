package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import java.net.InetSocketAddress
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.model.TunnelState

// ConnectionProxy adapts the Warren-native tunnel state into the legacy
// Mullvad-shaped Flow<TunnelState> consumed by the connection card, the
// in-app notification plumbing, and the rest of the upstream UI.
//
// It reads the lossless WarrenConnectedInfo projection (not the flattened
// String label) so the card can show the true state:
//   - Failed            -> Error(isBlocking = false)  ("ERROR STATE")
//   - Blocking (kill sw) -> Error(isBlocking = true)  ("BLOCKED CONNECTION")
//   - Connected         -> real exit/entry endpoint + DAITA/multihop/QUIC chips
//
// connect/disconnect/reconnect remain stubs (returning Right(true)) - the
// actual Warren tunnel commands flow through WarrenQuinnConnectInvoker /
// WarrenQuinnDisconnectInvoker / WarrenQuinnReconnectInvoker invoked
// directly by ConnectViewModel + DeviceRevokedViewModel.
class ConnectionProxy(private val tunnelStateProvider: WarrenTunnelStateProvider) {

    // Fallback when an endpoint host cannot be parsed as an IP literal, so
    // TunnelState.Connected always has a non-null endpoint.
    private val sentinelEndpoint: Endpoint = Endpoint(
        address = InetSocketAddress("0.0.0.0", 0),
        protocol = TransportProtocol.Udp,
    )

    val tunnelState: Flow<TunnelState> =
        tunnelStateProvider.connectedInfo.map { info ->
            when (info) {
                is WarrenConnectedInfo.Disconnected -> TunnelState.Disconnected(location = null)
                is WarrenConnectedInfo.Connecting -> TunnelState.Connecting(null, null, emptyList())
                is WarrenConnectedInfo.Reconnecting ->
                    TunnelState.Connecting(null, null, emptyList())
                is WarrenConnectedInfo.Connected ->
                    TunnelState.Connected(
                        endpoint = buildTunnelEndpoint(info),
                        location = null,
                        featureIndicators = buildFeatureIndicators(info),
                    )
                is WarrenConnectedInfo.Failed ->
                    TunnelState.Error(
                        ErrorState(cause = ErrorStateCause.StartTunnelError, isBlocking = false),
                    )
                is WarrenConnectedInfo.Blocking ->
                    TunnelState.Error(
                        ErrorState(
                            // The kill switch is up either way (isBlocking) and
                            // the block SUCCEEDED - this is the protective state
                            // working, not a failure. A flap surfaces the
                            // unstable-network cause; otherwise the dedicated
                            // kill-switch-active cause (NOT FirewallPolicyError,
                            // which means the firewall could not be applied and
                            // wrongly tells the user to send a problem report).
                            cause = if (info.flapping) {
                                ErrorStateCause.WarrenTunnelFlapping
                            } else {
                                ErrorStateCause.WarrenKillSwitchActive
                            },
                            isBlocking = true,
                        ),
                    )
            }
        }

    private fun buildTunnelEndpoint(info: WarrenConnectedInfo.Connected): TunnelEndpoint =
        TunnelEndpoint(
            entryEndpoint = info.entryEndpointHost?.let(::parseEndpoint),
            endpoint = parseEndpoint(info.exitEndpointHost) ?: sentinelEndpoint,
            quantumResistant = false,
            obfuscation = null,
            daita = info.daita,
        )

    private fun buildFeatureIndicators(info: WarrenConnectedInfo.Connected): List<FeatureIndicator> =
        buildList {
            if (info.daita && info.multiHop) {
                add(FeatureIndicator.DAITA_MULTIHOP)
            } else {
                if (info.daita) add(FeatureIndicator.DAITA)
                if (info.multiHop) add(FeatureIndicator.MULTIHOP)
            }
            // Warren tunnels are always QUIC with always-on HTTP/3 mimicry.
            add(FeatureIndicator.QUIC)
        }

    // Parse a "host:port" literal into a resolved Endpoint. Only IP literals
    // are accepted (no hostname DNS, which would block the collecting
    // thread); anything else returns null and falls back to the sentinel.
    private fun parseEndpoint(hostPort: String): Endpoint? {
        val (host, port) = splitHostPort(hostPort) ?: return null
        if (!looksLikeIpLiteral(host)) return null
        return try {
            Endpoint(
                address = InetSocketAddress(host, port),
                protocol = TransportProtocol.Udp,
            )
        } catch (e: IllegalArgumentException) {
            null
        }
    }

    private fun splitHostPort(hostPort: String): Pair<String, Int>? {
        val trimmed = hostPort.trim()
        if (trimmed.isEmpty()) return null
        // Bracketed IPv6, e.g. "[2001:db8::1]:443".
        if (trimmed.startsWith("[")) {
            val close = trimmed.indexOf(']')
            if (close <= 1) return null
            val host = trimmed.substring(1, close)
            val port = trimmed.substringAfter("]:", "").toIntOrNull() ?: return null
            return host to port
        }
        val host = trimmed.substringBeforeLast(":", "")
        val port = trimmed.substringAfterLast(":", "").toIntOrNull() ?: return null
        if (host.isEmpty()) return null
        return host to port
    }

    private fun looksLikeIpLiteral(host: String): Boolean =
        host.contains(":") || // bare IPv6
            host.matches(Regex("""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"""))

    suspend fun connect(): Either<ConnectError, Boolean> = true.right()

    suspend fun connectWithoutPermissionCheck(): Either<ConnectError, Boolean> = true.right()

    @Suppress("UNUSED_PARAMETER")
    suspend fun disconnect(disconnectReason: DisconnectReason): Either<ConnectError, Boolean> =
        true.right()

    suspend fun reconnect(): Either<ConnectError, Boolean> = true.right()
}
