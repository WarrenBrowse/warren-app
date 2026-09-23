package com.warrenbrowse.vpn.lib.model

import java.net.InetAddress

sealed class ErrorStateCause {
    class AuthFailed(val error: AuthFailedError) : ErrorStateCause() {
        fun isCausedByExpiredAccount(): Boolean {
            return error is AuthFailedError.ExpiredAccount
        }
    }

    data object Ipv6Unavailable : ErrorStateCause()

    sealed class FirewallPolicyError : ErrorStateCause() {
        data object Generic : FirewallPolicyError()
    }

    data object DnsError : ErrorStateCause()

    data class InvalidDnsServers(val addresses: List<InetAddress>) : ErrorStateCause()

    data object StartTunnelError : ErrorStateCause()

    /**
     * The tunnel reconnected too many times in a short window (the network is
     * flapping). The retry loop was stopped and the tunnel parked here so the
     * user can act once the connection settles. Under lockdown the kill switch
     * stays engaged, so traffic is blocked rather than leaked, while parked.
     */
    data object WarrenTunnelFlapping : ErrorStateCause()

    /**
     * The tunnel dropped too many times in a short window with lockdown mode
     * off, so the retry loop stopped and the traffic went back to the regular
     * network, outside the VPN. Never blocking. It has its own copy because the
     * generic non-blocking copy reports a firewall that could not block, while
     * this release is the policy with lockdown mode off, and the user needs
     * to know that lockdown mode keeps the traffic blocked instead.
     */
    data object WarrenTrafficReleased : ErrorStateCause()

    /**
     * The device is online and its network carries no address family a Warren
     * entry point can be dialed on (an IPv6-only mobile network: every entry
     * endpoint is an IPv4 literal). The retry loop is parked and the kill
     * switch is holding the traffic, so this is the one blocked state waiting
     * cannot end: the user changes network or disconnects. Distinct from
     * [IsOffline], which would be a lie here (the phone browses normally).
     */
    data object WarrenNoDialableNetwork : ErrorStateCause()

    /**
     * The kill switch (lockdown) is intentionally blocking traffic because the
     * tunnel is down. This is the protective state working as designed, NOT a
     * failure: do not map it to [FirewallPolicyError] (which means the OS
     * firewall could not be applied and tells the user to send a problem
     * report). The block succeeded; the message just informs the user.
     */
    data object WarrenKillSwitchActive : ErrorStateCause()

    data class TunnelParameterError(val error: ParameterGenerationError) : ErrorStateCause()

    data object IsOffline : ErrorStateCause()

    data object NotPrepared : ErrorStateCause()

    data class OtherAlwaysOnApp(val appName: String) : ErrorStateCause()

    data object OtherLegacyAlwaysOnApp : ErrorStateCause()

    data class NoRelaysMatchSelectedPort(val port: Port) : ErrorStateCause()

    data class InvalidIpv6Config(
        val addresses: List<String>,
        val routes: List<String>,
        val dnsServers: List<String>,
    ) : ErrorStateCause()
}

sealed interface AuthFailedError {
    data object ExpiredAccount : AuthFailedError

    data object InvalidAccount : AuthFailedError

    data object TooManyConnections : AuthFailedError

    // Warren: the account was explicitly revoked (banned) by the operator (its
    // pubkey is on the exit's signed CRL). Distinct from ExpiredAccount
    // (renewable) so the app shows a suspension rather than a renew prompt.
    data object Banned : AuthFailedError

    // Warren: banned specifically for port-forwarding abuse (the exit sealed the
    // port-forwarding reason code). Distinct from Banned only so the app can
    // show a forwarded-port-specific suspension; both are equally fatal.
    data object BannedPortForwarding : AuthFailedError

    data object Unknown : AuthFailedError
}
