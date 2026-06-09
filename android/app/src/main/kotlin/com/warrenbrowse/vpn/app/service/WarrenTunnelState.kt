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
        val obfuscationM40: Boolean,
        // "host:port" literals from the active WarrenTunnelConfig, surfaced
        // to the main connection card via WarrenConnectedInfo. Default to
        // empty/null so callers that don't have the config (e.g. the
        // fromStatusCode bootstrap) still compile.
        val exitEndpointHost: String = "",
        val entryEndpointHost: String? = null,
    ) : WarrenTunnelState()
    data object Reconnecting : WarrenTunnelState()
    data class Failed(val reason: String) : WarrenTunnelState()

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
     */
    data class Blocking(
        val reason: String,
        val flapping: Boolean = false,
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
                obfuscationM40 = false,
            )
            3 -> Reconnecting
            else -> Failed("native status code $code")
        }
    }
}
