package com.warrenbrowse.vpn.app.service

// Tunnel lifecycle state owned by `WarrenQuinnAdapter`. Mirrors the int code
// returned by `WarrenJni.getTunnelStatus()`:
//   0 = Disconnected
//   1 = Connecting
//   2 = Connected
//   3 = Reconnecting
//   negative = Failed
sealed class WarrenTunnelState {
    data object Disconnected : WarrenTunnelState()
    data object Connecting : WarrenTunnelState()
    data class Connected(
        val exitId: String,
        val assignedNatPmpPort: Int?,
        val multiHop: Boolean,
        val daita: Boolean,
        // "host:port" literals from the active WarrenTunnelConfig, surfaced
        // to the main connection card via WarrenConnectedInfo. Default to
        // empty/null so callers that don't have the config (e.g. the
        // fromStatusCode bootstrap) still compile.
        val exitEndpointHost: String = "",
        val entryEndpointHost: String? = null,
    ) : WarrenTunnelState()
    data object Reconnecting : WarrenTunnelState()

    /**
     * Tunnel failed (no kill switch). [expired] is set when the exit
     * refused the setup because the account is not authorized (lapsed /
     * revoked subscription), so the UI shows a "subscription expired"
     * message instead of a generic error and the reconnect loop stops.
     */
    data class Failed(val reason: String, val expired: Boolean = false) : WarrenTunnelState()

    /**
     * The tunnel is down but the kill switch (lockdown mode) is keeping a
     * blocking interface in place, so traffic is blocked rather than
     * leaking to the physical network. Mirrors the desktop `lockedDown`
     * state / "BLOCKING INTERNET" notification.
     *
     * [flapping] is set when the lockdown reconnect loop gave up after too
     * many drops in a short window: the blackhole stays up but no further
     * reconnect is scheduled, so the error reads as an unstable network
     * rather than a generic block.
     *
     * [expired] is set when the block is the result of the exit refusing
     * the account (lapsed / revoked subscription): the kill switch stays
     * up (fail-closed) but the message reads as "subscription expired" and
     * no reconnect is scheduled (retrying cannot recover until renewal).
     */
    data class Blocking(
        val reason: String,
        val flapping: Boolean = false,
        val expired: Boolean = false,
    ) : WarrenTunnelState()

    companion object {
        fun fromStatusCode(code: Int): WarrenTunnelState = when (code) {
            0 -> Disconnected
            1 -> Connecting
            2 -> Connected(
                exitId = "", // hydrated by Adapter when status flips to 2
                assignedNatPmpPort = null,
                multiHop = false,
                daita = false,
            )
            3 -> Reconnecting
            4 -> Failed("subscription expired", expired = true)
            else -> Failed("native status code $code")
        }
    }
}
