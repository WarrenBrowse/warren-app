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

    // Largest usable TUN MTU for the Warren QUIC tunnel and the default. The
    // QUIC datagram path cannot carry more, so this doubles as the ceiling the
    // user-configurable MTU is clamped to (the user may only lower it).
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
    if (blocking) {
        // Kill-switch interface: capture EVERYTHING (LAN included), resolve
        // nothing, pump nothing. "Allow LAN" never weakens the kill switch.
        return WarrenTunInterfacePlan(
            session = BLOCKING_SESSION,
            addresses = addresses,
            routes = listOf(
                WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV4_DEFAULT_ROUTE, 0),
                WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV6_DEFAULT_ROUTE, 0),
            ),
            dnsServers = emptyList(),
            mtu = WarrenTunDefaults.MTU,
            blocking = true,
        )
    }

    val dnsServers = resolveDnsServers(config.dns)

    // IPv4 capture. By default the whole internet (0.0.0.0/0) is tunnelled.
    // With "allow LAN", the RFC1918 / link-local ranges are excluded so LAN
    // hosts are reachable directly; the Warren exit DNS forwarder
    // (10.66.0.1, inside 10/8) is re-added as a /32 so DNS still travels
    // through the tunnel and never leaks to the LAN resolver.
    val ipv4Routes =
        if (config.allowLan) {
            ipv4RoutesExcluding(LAN_EXCLUDED_IPV4) +
                WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.EXIT_DNS_RESOLVER, 32)
        } else {
            listOf(WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV4_DEFAULT_ROUTE, 0))
        }

    // The IPv6 default route is present regardless of the toggles: with no
    // v6 address it acts as a blackhole (no leak); with a v6 address it
    // carries IPv6 through the tunnel. IPv6 LAN sharing is not offered.
    val routes = ipv4Routes +
        WarrenTunInterfacePlan.TunCidr(WarrenTunDefaults.IPV6_DEFAULT_ROUTE, 0)

    return WarrenTunInterfacePlan(
        session = ACTIVE_SESSION,
        addresses = addresses,
        routes = routes,
        dnsServers = dnsServers,
        // Clamp defensively: never above the Warren QUIC ceiling (raising it
        // risks black-holing oversized encapsulated packets), never below a
        // usable minimum. This caps to the ceiling, so the MTU can only be
        // lowered, never raised.
        mtu = config.mtu.coerceIn(MIN_MTU, MAX_MTU),
        blocking = false,
    )
}

private const val MIN_MTU = 576
private const val MAX_MTU = WarrenTunDefaults.MTU

/**
 * RFC1918 private ranges + link-local, excluded from the TUN routes when
 * "allow LAN" is on so local hosts are reachable off-tunnel.
 */
private val LAN_EXCLUDED_IPV4 = listOf(
    WarrenTunInterfacePlan.TunCidr("10.0.0.0", 8),
    WarrenTunInterfacePlan.TunCidr("172.16.0.0", 12),
    WarrenTunInterfacePlan.TunCidr("192.168.0.0", 16),
    WarrenTunInterfacePlan.TunCidr("169.254.0.0", 16),
)

/**
 * Compute the IPv4 route set that covers `0.0.0.0/0` minus [excluded],
 * expressed as a minimal list of aligned CIDR blocks. Used to build the
 * "allow LAN" split-route table without an OS `excludeRoute` call (which
 * only exists on API 33+). Pure 32-bit arithmetic; unit-tested.
 */
fun ipv4RoutesExcluding(
    excluded: List<WarrenTunInterfacePlan.TunCidr>,
): List<WarrenTunInterfacePlan.TunCidr> {
    var kept = listOf(0L to 0) // (network base, prefix), starting at 0.0.0.0/0
    for (ex in excluded) {
        val exBase = ipv4ToLong(ex.address)
        kept = kept.flatMap { (base, prefix) -> subtractV4(base, prefix, exBase, ex.prefixLength) }
    }
    return kept
        .sortedBy { it.first }
        .map { (base, prefix) -> WarrenTunInterfacePlan.TunCidr(longToIpv4(base), prefix) }
}

/** Subtract the CIDR [exBase]/[exPrefix] from [base]/[prefix]. */
private fun subtractV4(base: Long, prefix: Int, exBase: Long, exPrefix: Int): List<Pair<Long, Int>> {
    // No overlap: keep the block whole.
    if (!v4Overlaps(base, prefix, exBase, exPrefix)) return listOf(base to prefix)
    // Block fully inside the excluded range: drop it.
    if (v4Contains(exBase, exPrefix, base, prefix)) return emptyList()
    // Partial overlap: split into two halves and recurse (terminates once
    // each half is either disjoint from or contained in the excluded range).
    val childPrefix = prefix + 1
    val half = 1L shl (32 - childPrefix)
    return subtractV4(base, childPrefix, exBase, exPrefix) +
        subtractV4(base + half, childPrefix, exBase, exPrefix)
}

/** True if a/aPrefix fully contains b/bPrefix. */
private fun v4Contains(aBase: Long, aPrefix: Int, bBase: Long, bPrefix: Int): Boolean =
    aPrefix <= bPrefix && (bBase and v4Mask(aPrefix)) == aBase

/** Two aligned CIDRs overlap iff one contains the other. */
private fun v4Overlaps(aBase: Long, aPrefix: Int, bBase: Long, bPrefix: Int): Boolean =
    v4Contains(aBase, aPrefix, bBase, bPrefix) || v4Contains(bBase, bPrefix, aBase, aPrefix)

private fun v4Mask(prefix: Int): Long =
    if (prefix == 0) 0L else (0xFFFFFFFFL shl (32 - prefix)) and 0xFFFFFFFFL

private fun ipv4ToLong(addr: String): Long =
    addr.split('.').fold(0L) { acc, octet -> (acc shl 8) or (octet.toLong() and 0xFF) }

private fun longToIpv4(value: Long): String =
    "${(value shr 24) and 0xFF}.${(value shr 16) and 0xFF}.${(value shr 8) and 0xFF}.${value and 0xFF}"

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
