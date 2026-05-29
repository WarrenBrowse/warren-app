package com.warrenbrowse.vpn.app.service

/**
 * Pure description of the `VpnService.Builder` interface to establish for a
 * given [WarrenTunnelConfig]. Extracted from [WarrenQuinnAdapter] so the
 * leak-prevention logic (IPv6 blackholing, DNS-in-tunnel, kill-switch
 * blocking interface) can be unit-tested without a real `VpnService`.
 *
 * The adapter translates a plan onto the builder verbatim: every
 * [addresses] entry becomes `addAddress`, every [routes] entry becomes
 * `addRoute`, every [dnsServers] entry becomes `addDnsServer`.
 */
data class WarrenTunInterfacePlan(
    val session: String,
    val addresses: List<TunCidr>,
    val routes: List<TunCidr>,
    val dnsServers: List<String>,
    val mtu: Int,
    /**
     * When true the interface is a kill-switch blackhole: it captures all
     * traffic but no packet pump is attached, so everything is dropped.
     * Used to keep traffic from leaking to the physical network while the
     * real tunnel is down and `lockdownMode` is enabled.
     */
    val blocking: Boolean,
) {
    data class TunCidr(val address: String, val prefixLength: Int)
}

/** Tunnel-internal addressing, matching the Warren exit gateway layout. */
object WarrenTunDefaults {
    /** Client address assigned inside the tunnel (legacy fixed pair). */
    const val IPV4_ADDRESS = "10.64.0.1"
    const val IPV4_PREFIX = 32

    /** Unique-local IPv6 client address, only assigned when IPv6 is enabled. */
    const val IPV6_ADDRESS = "fd00::1"
    const val IPV6_PREFIX = 128

    const val IPV4_DEFAULT_ROUTE = "0.0.0.0"
    const val IPV6_DEFAULT_ROUTE = "::"

    /**
     * Warren exit DNS forwarder address (`10.66.0.1:53`). Used as the
     * default in-tunnel resolver so DNS queries egress through the tunnel
     * instead of the LAN resolver. See `warren-exit --dns-listen`.
     */
    const val EXIT_DNS_RESOLVER = "10.66.0.1"

    const val MTU = 1280
}

/**
 * Compute the TUN interface plan for [config].
 *
 * Privacy rules enforced here:
 *  - IPv4 is always routed into the tunnel (`0.0.0.0/0`).
 *  - IPv6 default route (`::/0`) is **always** installed so IPv6 can never
 *    leak to the physical network. When [WarrenTunnelConfig.enableIpv6] is
 *    false no IPv6 address is assigned, so captured IPv6 traffic is
 *    blackholed (apps get no global v6 address and any stray packet is
 *    dropped). When true, the ULA address is assigned and IPv6 is carried
 *    through the tunnel.
 *  - DNS always points at an in-tunnel resolver (custom servers when the
 *    user provides them, otherwise the exit forwarder) so DNS cannot leak.
 *
 * @param blocking when true, returns a kill-switch blackhole plan: full
 *   capture, no DNS, no pump. Used while the real tunnel is down under
 *   lockdown so traffic stays blocked rather than falling back to the
 *   physical network.
 */
fun planTunInterface(
    config: WarrenTunnelConfig,
    blocking: Boolean = false,
): WarrenTunInterfacePlan {
    val addresses = mutableListOf(
        WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV4_ADDRESS, WarrenTunDefaults.IPV4_PREFIX),
    )
    if (config.enableIpv6) {
        addresses += WarrenTunInterfacePlan.TunCidr(
            WarrenTunDefaults.IPV6_ADDRESS,
            WarrenTunDefaults.IPV6_PREFIX,
        )
    }

    // Capture everything. The IPv6 default route is present regardless of
    // the toggle: with no v6 address it acts as a blackhole (no leak); with
    // a v6 address it carries IPv6 through the tunnel.
    val routes = listOf(
        WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV4_DEFAULT_ROUTE, 0),
        WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV6_DEFAULT_ROUTE, 0),
    )

    if (blocking) {
        // Kill-switch interface: capture all, resolve nothing, pump nothing.
        return WarrenTunInterfacePlan(
            session = BLOCKING_SESSION,
            addresses = addresses,
            routes = routes,
            dnsServers = emptyList(),
            mtu = WarrenTunDefaults.MTU,
            blocking = true,
        )
    }

    val dnsServers = resolveDnsServers(config.dns)

    return WarrenTunInterfacePlan(
        session = ACTIVE_SESSION,
        addresses = addresses,
        routes = routes,
        dnsServers = dnsServers,
        mtu = WarrenTunDefaults.MTU,
        blocking = false,
    )
}

/**
 * Resolve the DNS servers to install. Custom valid resolvers win; otherwise
 * the exit forwarder is used so DNS always travels through the tunnel.
 */
private fun resolveDnsServers(dns: WarrenTunnelConfig.DnsConfig?): List<String> {
    if (dns != null && dns.state == WarrenTunnelConfig.DnsConfig.STATE_CUSTOM) {
        val valid = dns.customServers.filter { isValidIpLiteral(it) }
        if (valid.isNotEmpty()) return valid
        // Custom mode with no usable address: fall back to the in-tunnel
        // forwarder rather than leaving DNS unset (which would leak to the
        // LAN resolver).
    }
    return listOf(WarrenTunDefaults.EXIT_DNS_RESOLVER)
}

/**
 * Minimal IPv4/IPv6 literal validation. Rejects empty, host:port and
 * obviously malformed values before they reach `addDnsServer` (which would
 * throw and abort tunnel establishment).
 */
fun isValidIpLiteral(value: String): Boolean {
    val v = value.trim()
    if (v.isEmpty()) return false
    if (v.contains(':') && v.contains('.')) return false // no host:port v4
    return if (v.contains(':')) isValidIpv6(v) else isValidIpv4(v)
}

private fun isValidIpv4(v: String): Boolean {
    val parts = v.split('.')
    if (parts.size != 4) return false
    return parts.all { part ->
        part.isNotEmpty() && part.length <= 3 && part.all(Char::isDigit) &&
            part.toInt() in 0..255
    }
}

private fun isValidIpv6(v: String): Boolean {
    // Accept the standard hextet form with at most one "::" compression.
    if (v.count { it == ':' } < 2) return false
    val doubleColons = Regex("::").findAll(v).count()
    if (doubleColons > 1) return false
    val hextets = v.split(':').filter { it.isNotEmpty() }
    return hextets.all { it.length <= 4 && it.all { c -> c.isDigit() || c.lowercaseChar() in 'a'..'f' } }
}

private const val ACTIVE_SESSION = "Warren VPN"
private const val BLOCKING_SESSION = "Warren VPN (blocking)"
