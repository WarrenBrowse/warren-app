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
    ) = WarrenTunnelConfig(
        exitPubkeyHex = "ab".repeat(32),
        exitEndpoint = "warren-exit-1.warren.brown:443",
        walletPubkeyHex = "cd".repeat(32),
        enableIpv6 = enableIpv6,
        lockdownMode = lockdown,
        dns = dns,
    )

    private fun WarrenTunInterfacePlan.hasRoute(address: String) =
        routes.any { it.address == address && it.prefixLength == 0 }

    private fun WarrenTunInterfacePlan.hasAddress(address: String) =
        addresses.any { it.address == address }

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
