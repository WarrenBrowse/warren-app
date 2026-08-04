package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import java.net.InetSocketAddress
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.transformLatest
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.AuthFailedError
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
//   - Connecting        -> the dialled endpoint + its chips, so the card does
//                          not empty out for the whole connecting phase
//   - Disconnecting     -> the teardown window, where Connect is disabled
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

    // A user request the engine has not acknowledged yet. The connect path runs
    // a signed round trip before the service is even started, so without an
    // optimistic frame the card sits unchanged for over a second after the tap.
    private val pendingRequest = MutableStateFlow<PendingRequest?>(null)

    private enum class RequestKind {
        Connect,
        Disconnect,
        Reconnect,
    }

    private data class PendingRequest(
        val kind: RequestKind,
        // The engine state the request was dispatched against. The frame is
        // retired the moment the engine leaves it, which is what keeps the
        // optimism bounded to the round trip it is covering.
        val baseline: WarrenConnectedInfo,
        val multiHop: Boolean = false,
        val daita: Boolean = false,
        // Reconnect only: the re-dial has been observed, so the next settled
        // state answers the request. A re-dial onto the same exit lands on an
        // engine state identical to the baseline, so the baseline alone cannot
        // tell the old tunnel from the new one.
        val redialSeen: Boolean = false,
    )

    /**
     * What the pending request wants done with the current engine state.
     */
    private sealed interface Frame {
        /** Present [state] instead of the engine state. */
        data class Show(val state: TunnelState) : Frame

        /** The engine is already telling the truth; keep the request pending. */
        data object Passthrough : Frame

        /** The engine answered the request: drop it. */
        data object Retire : Frame
    }

    /** A connect was dispatched: present it now, before the engine catches up. */
    fun onConnectRequested(multiHop: Boolean = false, daita: Boolean = false) {
        pendingRequest.value =
            PendingRequest(
                kind = RequestKind.Connect,
                baseline = tunnelStateProvider.connectedInfo.value,
                multiHop = multiHop,
                daita = daita,
            )
    }

    /** A disconnect (or a cancel) was dispatched. */
    fun onDisconnectRequested() {
        pendingRequest.value =
            PendingRequest(
                kind = RequestKind.Disconnect,
                baseline = tunnelStateProvider.connectedInfo.value,
            )
    }

    /**
     * A re-dial was dispatched (a location pick, a shuffle, a settings change
     * applied to the live tunnel). The engine tears the old session down and
     * passes through [WarrenConnectedInfo.Disconnected] before dialling the
     * next one; that frame would flip the scenery to the exposed art and
     * collapse the expanded card for the length of the gap, so it is bridged
     * as the connection in progress it actually is.
     */
    fun onReconnectRequested() {
        pendingRequest.value =
            PendingRequest(
                kind = RequestKind.Reconnect,
                baseline = tunnelStateProvider.connectedInfo.value,
            )
    }

    /** The request never reached the engine, so nothing will ever answer it. */
    fun onRequestAborted() {
        pendingRequest.value = null
    }

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val tunnelState: Flow<TunnelState> =
        combine(tunnelStateProvider.connectedInfo, pendingRequest) { info, pending ->
            info to pending
        }
            .transformLatest { (info, pending) ->
                when (val frame = pending?.let { frame(it, info) }) {
                    is Frame.Show -> {
                        emit(frame.state)
                        // A presented frame is a promise the engine has not kept
                        // yet, so it is held only as long as the round trip it
                        // covers: a dispatch the engine never answers must not
                        // strand the card mid-transition forever.
                        delay(OPTIMISTIC_FRAME_TIMEOUT_MILLIS)
                        pendingRequest.compareAndSet(pending, null)
                    }
                    Frame.Passthrough -> emit(engineState(info))
                    null,
                    Frame.Retire -> {
                        // Answered, or never applicable: retire the frame so a
                        // later identical engine state cannot revive it.
                        if (pending != null) pendingRequest.compareAndSet(pending, null)
                        emit(engineState(info))
                    }
                }
            }
            .distinctUntilChanged()

    private fun frame(pending: PendingRequest, info: WarrenConnectedInfo): Frame =
        when (pending.kind) {
            RequestKind.Connect -> connectFrame(pending, info)
            RequestKind.Disconnect -> disconnectFrame(pending, info)
            RequestKind.Reconnect -> reconnectFrame(pending, info)
        }

    private fun connectFrame(pending: PendingRequest, info: WarrenConnectedInfo): Frame =
        if (
            info != pending.baseline ||
                info is WarrenConnectedInfo.Connected ||
                info is WarrenConnectedInfo.Dialling
        ) {
            Frame.Retire
        } else {
            Frame.Show(
                TunnelState.Connecting(
                    endpoint = null,
                    location = null,
                    featureIndicators =
                        buildFeatureIndicators(pending.multiHop, pending.daita, null),
                ),
            )
        }

    private fun disconnectFrame(pending: PendingRequest, info: WarrenConnectedInfo): Frame =
        if (
            info != pending.baseline ||
                info is WarrenConnectedInfo.Disconnected ||
                info is WarrenConnectedInfo.Disconnecting
        ) {
            Frame.Retire
        } else {
            Frame.Show(TunnelState.Disconnecting(ActionAfterDisconnect.Nothing))
        }

    // The re-dial sequence is teardown -> Disconnected -> dial -> up. Only the
    // Disconnected gap is substituted; every other step already reads as a
    // connection in progress, so the engine keeps speaking for itself.
    private fun reconnectFrame(pending: PendingRequest, info: WarrenConnectedInfo): Frame =
        when (info) {
            is WarrenConnectedInfo.Disconnected ->
                Frame.Show(TunnelState.Disconnecting(ActionAfterDisconnect.Reconnect))
            is WarrenConnectedInfo.Dialling -> {
                // Records the phase so the tunnel that comes up next is known to
                // be the new one; the write re-runs this mapping once and
                // settles, since the recorded request maps to the same frame.
                pendingRequest.compareAndSet(pending, pending.copy(redialSeen = true))
                Frame.Passthrough
            }
            is WarrenConnectedInfo.Disconnecting -> Frame.Passthrough
            is WarrenConnectedInfo.Connected ->
                if (pending.redialSeen) Frame.Retire else Frame.Passthrough
            is WarrenConnectedInfo.Failed,
            is WarrenConnectedInfo.Blocking -> Frame.Retire
        }

    private fun engineState(info: WarrenConnectedInfo): TunnelState =
        when (info) {
            is WarrenConnectedInfo.Disconnected -> TunnelState.Disconnected(location = null)
            is WarrenConnectedInfo.Dialling ->
                TunnelState.Connecting(
                    endpoint = buildDiallingEndpoint(info),
                    location = null,
                    featureIndicators =
                        buildFeatureIndicators(info.multiHop, info.daita, null),
                )
            is WarrenConnectedInfo.Disconnecting ->
                TunnelState.Disconnecting(
                    if (info.reconnecting) {
                        ActionAfterDisconnect.Reconnect
                    } else {
                        ActionAfterDisconnect.Nothing
                    },
                )
            is WarrenConnectedInfo.Connected ->
                TunnelState.Connected(
                    endpoint = buildTunnelEndpoint(info),
                    location = null,
                    featureIndicators =
                        buildFeatureIndicators(
                            info.multiHop,
                            info.daita,
                            info.assignedNatPmpPort,
                        ),
                )
            is WarrenConnectedInfo.Failed ->
                TunnelState.Error(
                    ErrorState(
                        // An expired/revoked subscription is an auth
                        // failure (actionable: renew), not a generic tunnel
                        // start error.
                        cause = if (info.expired) {
                            ErrorStateCause.AuthFailed(AuthFailedError.ExpiredAccount)
                        } else {
                            ErrorStateCause.StartTunnelError
                        },
                        isBlocking = false,
                    ),
                )
            is WarrenConnectedInfo.Blocking ->
                TunnelState.Error(
                    ErrorState(
                        // The kill switch is up either way (isBlocking) and
                        // the block SUCCEEDED - this is the protective state
                        // working, not a failure. An expired account
                        // surfaces the actionable "subscription expired"
                        // cause; a flap surfaces the unstable-network cause;
                        // otherwise the dedicated kill-switch-active cause
                        // (NOT FirewallPolicyError, which means the firewall
                        // could not be applied and wrongly tells the user to
                        // send a problem report).
                        cause = when {
                            info.expired ->
                                ErrorStateCause.AuthFailed(AuthFailedError.ExpiredAccount)
                            info.flapping -> ErrorStateCause.WarrenTunnelFlapping
                            else -> ErrorStateCause.WarrenKillSwitchActive
                        },
                        isBlocking = true,
                    ),
                )
        }

    private fun buildTunnelEndpoint(info: WarrenConnectedInfo.Connected): TunnelEndpoint =
        TunnelEndpoint(
            entryEndpoint = info.entryEndpointHost?.let(::parseEndpoint),
            endpoint = parseEndpoint(info.exitEndpointHost) ?: sentinelEndpoint,
            quantumResistant = false,
            obfuscation = null,
            daita = info.daita,
        )

    // A dial whose exit host is unknown (or unparseable) reports no endpoint at
    // all rather than the sentinel: the connection details leave the In row
    // blank instead of printing 0.0.0.0:0 at the user.
    private fun buildDiallingEndpoint(info: WarrenConnectedInfo.Dialling): TunnelEndpoint? {
        val exit = info.exitEndpointHost?.let(::parseEndpoint) ?: return null
        return TunnelEndpoint(
            entryEndpoint = info.entryEndpointHost?.let(::parseEndpoint),
            endpoint = exit,
            quantumResistant = false,
            obfuscation = null,
            daita = info.daita,
        )
    }

    private fun buildFeatureIndicators(
        multiHop: Boolean,
        daita: Boolean,
        assignedNatPmpPort: Int?,
    ): List<FeatureIndicator> =
        buildList {
            if (daita && multiHop) {
                add(FeatureIndicator.DAITA_MULTIHOP)
            } else {
                if (daita) add(FeatureIndicator.DAITA)
                if (multiHop) add(FeatureIndicator.MULTIHOP)
            }
            // Show the port-forwarding chip only when the gateway actually
            // granted a mapping (desktop parity: the badge advertises a live
            // forward, not merely that NAT-PMP is enabled in settings).
            if (assignedNatPmpPort != null) add(FeatureIndicator.PORT_FORWARDING)
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

    companion object {
        /**
         * How long a presented frame may outlive the engine state it was
         * dispatched against. Long enough for a signed connect round trip on a
         * slow network, short enough that a dispatch nothing ever answers is
         * corrected rather than left on screen.
         */
        internal const val OPTIMISTIC_FRAME_TIMEOUT_MILLIS = 10_000L
    }
}
