package com.warrenbrowse.vpn.lib.repository

/**
 * Whether a forum request (the sign-in, the in-app report) may leave now.
 *
 * The forum POST rides the VpnService-protected transport, so its socket
 * bypasses the TUN, but the connect host name is still resolved by the system
 * resolver, which the protector cannot cover. While the tunnel is being brought
 * up or torn down the resolver points at the tunnel DNS and the lookup times
 * out; while the kill switch holds a blocking interface there is no DNS server
 * at all and the lookup fails at once. A tunnel that failed without the kill
 * switch is a state the reconnect loop may leave at any moment, so it is asked
 * to settle too. Connected and Disconnected are the two states in which the
 * lookup can succeed, and the only two that proceed.
 */
sealed interface ForumPreflight {
    data object Proceed : ForumPreflight

    /**
     * Not now. [tunnelClass] names the state class for the journal and the
     * report (`connecting`, `reconnecting`, `disconnecting`, `failed`,
     * `blocking`), never an endpoint or an engine reason.
     */
    data class Defer(val tunnelClass: String) : ForumPreflight

    companion object {
        fun of(info: WarrenConnectedInfo): ForumPreflight =
            when (info) {
                WarrenConnectedInfo.Disconnected,
                is WarrenConnectedInfo.Connected -> Proceed
                is WarrenConnectedInfo.Connecting -> Defer("connecting")
                is WarrenConnectedInfo.Reconnecting -> Defer("reconnecting")
                is WarrenConnectedInfo.Disconnecting ->
                    Defer(if (info.reconnecting) "reconnecting" else "disconnecting")
                is WarrenConnectedInfo.Failed -> Defer("failed")
                is WarrenConnectedInfo.Blocking -> Defer("blocking")
            }
    }
}
