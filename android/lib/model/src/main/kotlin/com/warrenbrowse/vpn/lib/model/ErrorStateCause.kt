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

    data object Unknown : AuthFailedError
}
