package com.warrenbrowse.vpn.app.service

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenTunInterfacePlanTest {

    private fun config(
        enableIpv6: Boolean = false,
        lockdown: Boolean = false,
        dns: WarrenTunnelConfig.DnsConfig? = null,
        allowLan: Boolean = false,
        mtu: Int = 1280,
    ) = WarrenTunnelConfig(
        exitPubkeyHex = "ab".repeat(32),
        exitEndpoint = "warren-exit-1.warren.brown:443",
        walletPubkeyHex = "cd".repeat(32),
        enableIpv6 = enableIpv6,
        lockdownMode = lockdown,
        dns = dns,
        allowLan = allowLan,
        mtu = mtu,
    )

    private fun WarrenTunInterfacePlan.hasRoute(address: String) =
        routes.any { it.address == address && it.prefixLength == 0 }

    private fun WarrenTunInterfacePlan.hasAddress(address: String) =
        addresses.any { it.address == address }

    // Self-contained "is this IPv4 covered by some route?" check, mirroring
    // the longest-prefix match the OS would do, so the tests can assert
    // exactly which traffic the TUN captures vs lets out to the LAN.
    private fun ipv4ToLong(a: String): Long =
        a.split('.').fold(0L) { acc, o -> (acc shl 8) or (o.toLong() and 0xFF) }

    private fun WarrenTunInterfacePlan.coversV4(ip: String): Boolean {
        val addr = ipv4ToLong(ip)
        return routes.any { r ->
            if (r.address.contains(':')) return@any false
            val base = ipv4ToLong(r.address)
            val mask = if (r.prefixLength == 0) {
                0L
            } else {
                (0xFFFFFFFFL shl (32 - r.prefixLength)) and 0xFFFFFFFFL
            }
            (addr and mask) == base
        }
    }

    @Test
    fun `ipv4 is always routed`() {
        val plan = planTunInterface(config())
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV4_DEFAULT_ROUTE))
        assertTrue(plan.hasAddress(WarrenTunDefaults.IPV4_ADDRESS))
    }

    @Test
    fun `ipv6 disabled still captures the v6 default route (no leak) but assigns no v6 address`() {
        val plan = planTunInterface(config(enableIpv6 = false))
        // Route present => IPv6 is blackholed into the tunnel, never the LAN.
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV6_DEFAULT_ROUTE))
        // No v6 address => apps get no global IPv6 connectivity.
        assertFalse(plan.hasAddress(WarrenTunDefaults.IPV6_ADDRESS))
    }

    @Test
    fun `ipv6 enabled assigns the v6 address and routes it`() {
        val plan = planTunInterface(config(enableIpv6 = true))
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV6_DEFAULT_ROUTE))
        assertTrue(plan.hasAddress(WarrenTunDefaults.IPV6_ADDRESS))
    }

    @Test
    fun `default dns points at the in-tunnel exit resolver (no dns leak)`() {
        val plan = planTunInterface(config())
        assertEquals(listOf(WarrenTunDefaults.EXIT_DNS_RESOLVER), plan.dnsServers)
    }

    @Test
    fun `custom dns servers are used when valid`() {
        val plan = planTunInterface(
            config(
                dns = WarrenTunnelConfig.DnsConfig(
                    state = WarrenTunnelConfig.DnsConfig.STATE_CUSTOM,
                    customServers = listOf("9.9.9.9", "2620:fe::fe"),
                ),
            ),
        )
        assertEquals(listOf("9.9.9.9", "2620:fe::fe"), plan.dnsServers)
    }

    @Test
    fun `invalid custom dns servers are filtered out`() {
        val plan = planTunInterface(
            config(
                dns = WarrenTunnelConfig.DnsConfig(
                    state = WarrenTunnelConfig.DnsConfig.STATE_CUSTOM,
                    customServers = listOf("9.9.9.9", "not-an-ip", "8.8.8.8:53", ""),
                ),
            ),
        )
        assertEquals(listOf("9.9.9.9"), plan.dnsServers)
    }

    @Test
    fun `custom dns with no usable address falls back to the exit resolver`() {
        val plan = planTunInterface(
            config(
                dns = WarrenTunnelConfig.DnsConfig(
                    state = WarrenTunnelConfig.DnsConfig.STATE_CUSTOM,
                    customServers = listOf("garbage", "still:bad:host"),
                ),
            ),
        )
        assertEquals(listOf(WarrenTunDefaults.EXIT_DNS_RESOLVER), plan.dnsServers)
    }

    @Test
    fun `blocking plan captures all traffic, resolves nothing, and is flagged blocking`() {
        val plan = planTunInterface(config(lockdown = true), blocking = true)
        assertTrue(plan.blocking)
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV4_DEFAULT_ROUTE))
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV6_DEFAULT_ROUTE))
        assertTrue(plan.dnsServers.isEmpty())
    }

    @Test
    fun `active plan is not flagged blocking`() {
        assertFalse(planTunInterface(config()).blocking)
    }

    @Test
    fun `allow lan off tunnels the entire ipv4 internet including private ranges`() {
        val plan = planTunInterface(config(allowLan = false))
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV4_DEFAULT_ROUTE))
        // Everything is captured: public and LAN alike.
        assertTrue(plan.coversV4("8.8.8.8"))
        assertTrue(plan.coversV4("192.168.1.1"))
        assertTrue(plan.coversV4("10.0.0.5"))
    }

    @Test
    fun `allow lan excludes private and link-local ranges from the tunnel`() {
        val plan = planTunInterface(config(allowLan = true))
        // LAN / link-local hosts are reachable directly (NOT tunnelled).
        assertFalse(plan.coversV4("192.168.1.1"), "192.168/16 must be off-tunnel")
        assertFalse(plan.coversV4("10.0.0.5"), "10/8 must be off-tunnel")
        assertFalse(plan.coversV4("172.16.5.4"), "172.16/12 must be off-tunnel")
        assertFalse(plan.coversV4("172.31.255.1"), "172.16/12 upper bound off-tunnel")
        assertFalse(plan.coversV4("169.254.10.10"), "169.254/16 must be off-tunnel")
        // The full default route must be gone (no 0.0.0.0/0 catch-all).
        assertFalse(plan.hasRoute(WarrenTunDefaults.IPV4_DEFAULT_ROUTE))
    }

    @Test
    fun `allow lan still tunnels the public internet and the exit dns resolver`() {
        val plan = planTunInterface(config(allowLan = true))
        // Public addresses stay tunnelled (no leak).
        assertTrue(plan.coversV4("8.8.8.8"))
        assertTrue(plan.coversV4("1.1.1.1"))
        assertTrue(plan.coversV4("93.184.216.34"))
        // Addresses just outside the excluded ranges stay tunnelled.
        assertTrue(plan.coversV4("11.0.0.1"), "just above 10/8")
        assertTrue(plan.coversV4("172.15.255.1"), "just below 172.16/12")
        assertTrue(plan.coversV4("172.32.0.1"), "just above 172.16/12")
        assertTrue(plan.coversV4("192.167.255.1"), "just below 192.168/16")
        // The exit DNS forwarder (10.66.0.1, inside 10/8) is re-added as /32
        // so DNS never leaks to the LAN resolver.
        assertTrue(plan.coversV4("10.66.0.1"), "exit DNS resolver must stay tunnelled")
        // IPv6 is still fully captured.
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV6_DEFAULT_ROUTE))
    }

    @Test
    fun `the kill-switch blocking plan ignores allow lan and captures everything`() {
        val plan = planTunInterface(config(allowLan = true, lockdown = true), blocking = true)
        assertTrue(plan.blocking)
        // Allow-LAN must never weaken the kill switch: LAN is captured too.
        assertTrue(plan.coversV4("192.168.1.1"))
        assertTrue(plan.coversV4("10.0.0.5"))
        assertTrue(plan.hasRoute(WarrenTunDefaults.IPV4_DEFAULT_ROUTE))
    }

    @Test
    fun `mtu comes from the config and is clamped to the safe range (never above the QUIC floor)`() {
        assertEquals(WarrenTunDefaults.MTU, planTunInterface(config()).mtu)
        assertEquals(1000, planTunInterface(config(mtu = 1000)).mtu)
        // Above the floor is clamped down (raising MTU could black-hole traffic).
        assertEquals(WarrenTunDefaults.MTU, planTunInterface(config(mtu = 1500)).mtu)
        // Below the minimum is clamped up.
        assertEquals(576, planTunInterface(config(mtu = 100)).mtu)
    }

    @Test
    fun `ipv4RoutesExcluding produces a minimal complement that omits the excluded block`() {
        val routes = ipv4RoutesExcluding(
            listOf(WarrenTunInterfacePlan.TunCidr("10.0.0.0", 8)),
        )
        // No catch-all default remains.
        assertFalse(routes.any { it.address == "0.0.0.0" && it.prefixLength == 0 })
        // The complement is well-formed: addresses in 10/8 are not covered,
        // neighbours are.
        val plan = WarrenTunInterfacePlan(
            session = "t", addresses = emptyList(), routes = routes,
            dnsServers = emptyList(), mtu = 1280, blocking = false,
        )
        assertFalse(plan.coversV4("10.0.0.1"))
        assertFalse(plan.coversV4("10.255.255.255"))
        assertTrue(plan.coversV4("9.255.255.255"))
        assertTrue(plan.coversV4("11.0.0.0"))
    }

    @Test
    fun `ip literal validation accepts v4 and v6, rejects junk and host-port`() {
        assertTrue(isValidIpLiteral("9.9.9.9"))
        assertTrue(isValidIpLiteral("2620:fe::fe"))
        assertTrue(isValidIpLiteral("::1"))
        assertFalse(isValidIpLiteral(""))
        assertFalse(isValidIpLiteral("not-an-ip"))
        assertFalse(isValidIpLiteral("8.8.8.8:53"))
        assertFalse(isValidIpLiteral("999.1.1.1"))
        assertFalse(isValidIpLiteral("1.2.3"))
    }
}
