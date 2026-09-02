package com.warrenbrowse.vpn.lib.repository

/**
 * Whether a forum request (the sign-in, the in-app report) may leave now.
 *
 * The forum POST rides the VpnService-protected transport, so its socket
 * bypasses the TUN, but the connect host name is still resolved by the system
 * resolver, which the protector cannot cover. While the tunnel is being brought
 * up or torn down the resolver points at the tunnel DNS and the lookup times
 * out; while the kill switch holds a blocking interface there is no DNS server
 * at all and the lookup fails at once. Those are the states that defer.
 *
 * Failed proceeds: every path that publishes it has already released traffic
 * (the TUN closed, the blocking interface torn down, the network callback
 * gone) and schedules no reconnect, so the device sits on the bare network
 * with the physical resolver, and it stays there until the user acts. It is
 * also the state a user most wants to report from; deferring it refused the
 * report in exactly the failure it described.
 */
sealed interface ForumPreflight {
    data object Proceed : ForumPreflight

    /**
     * Not now. [tunnelClass] names the state class for the journal and the
     * report (`connecting`, `reconnecting`, `disconnecting`, `blocking`),
     * never an endpoint or an engine reason.
     */
    data class Defer(val tunnelClass: String) : ForumPreflight

    companion object {
        fun of(info: WarrenConnectedInfo): ForumPreflight =
            when (info) {
                WarrenConnectedInfo.Disconnected,
                is WarrenConnectedInfo.Connected,
                is WarrenConnectedInfo.Failed -> Proceed
                is WarrenConnectedInfo.Connecting -> Defer("connecting")
                is WarrenConnectedInfo.Reconnecting -> Defer("reconnecting")
                is WarrenConnectedInfo.Disconnecting ->
                    Defer(if (info.reconnecting) "reconnecting" else "disconnecting")
                is WarrenConnectedInfo.Blocking -> Defer("blocking")
            }
    }
}
