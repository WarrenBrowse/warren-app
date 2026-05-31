package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenTunnelConfigBuilderTest {

    private val pubkey = WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")

    private val sampleRelay = RelayInfo(
        exitId = "2921abad869e94064b56cf48c8da3631",
        exitPubkeyHex = "2921abad869e94064b56cf48c8da3631",
        endpoint = "warren-exit-1.warren.brown:443",
        country = "DE",
        city = "Falkenstein",
        active = true,
        weight = 100,
    )

    private fun mockRepo(
        daita: Boolean = false,
        natPmp: Boolean = false,
        multiHop: Boolean = false,
        obfuscation: Boolean = false,
        selectedExitId: String? = null,
        entryCountry: String? = null,
        exitCountry: String? = null,
        natPmpProtocol: String = "udp",
        natPmpExternalPort: Int = 0,
        natPmpLifetimeSecs: Int = 3600,
        ipv6: Boolean = false,
        lockdown: Boolean = false,
        allowLan: Boolean = false,
        dnsState: String = WarrenLocalSettingsRepository.DNS_STATE_DEFAULT,
        customDns: List<String> = emptyList(),
        blockAds: Boolean = false,
        blockTrackers: Boolean = false,
        blockMalware: Boolean = false,
        blockAdult: Boolean = false,
        blockGambling: Boolean = false,
        blockSocial: Boolean = false,
    ): WarrenLocalSettingsRepository {
        val repo: WarrenLocalSettingsRepository = mockk()
        every { repo.daitaEnabled } returns MutableStateFlow(daita)
        every { repo.natPmpEnabled } returns MutableStateFlow(natPmp)
        every { repo.natPmpProtocol } returns MutableStateFlow(natPmpProtocol)
        every { repo.natPmpExternalPort } returns MutableStateFlow(natPmpExternalPort)
        every { repo.natPmpLifetimeSecs } returns MutableStateFlow(natPmpLifetimeSecs)
        every { repo.multiHopEnabled } returns MutableStateFlow(multiHop)
        every { repo.obfuscationM40 } returns MutableStateFlow(obfuscation)
        every { repo.selectedExitId } returns MutableStateFlow(selectedExitId)
        every { repo.entryCountry } returns MutableStateFlow(entryCountry)
        every { repo.exitCountry } returns MutableStateFlow(exitCountry)
        every { repo.ipv6Enabled } returns MutableStateFlow(ipv6)
        every { repo.lockdownMode } returns MutableStateFlow(lockdown)
        every { repo.allowLan } returns MutableStateFlow(allowLan)
        every { repo.dnsState } returns MutableStateFlow(dnsState)
        every { repo.customDnsServers } returns MutableStateFlow(customDns)
        every { repo.blockAds } returns MutableStateFlow(blockAds)
        every { repo.blockTrackers } returns MutableStateFlow(blockTrackers)
        every { repo.blockMalware } returns MutableStateFlow(blockMalware)
        every { repo.blockAdultContent } returns MutableStateFlow(blockAdult)
        every { repo.blockGambling } returns MutableStateFlow(blockGambling)
        every { repo.blockSocialMedia } returns MutableStateFlow(blockSocial)
        return repo
    }

    private fun mockCatalog(relays: List<RelayInfo> = listOf(sampleRelay)): RelayCatalog {
        val catalog: RelayCatalog = mockk()
        every { catalog.listRelays() } returns relays
        return catalog
    }

    @Test
    fun `default config has no entry hop and no daita`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(), mockCatalog())
        val config = builder.build(pubkey)!!

        assertNull(config.entryHop)
        assertNull(config.daita)
        assertFalse(config.natPmpEnabled)
        assertFalse(config.obfuscationM40)
        assertEquals(pubkey.value, config.walletPubkeyHex)
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `daita on injects a Tamaraw spec`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(daita = true), mockCatalog())
        val config = builder.build(pubkey)!!

        assertNotNull(config.daita)
        assertEquals("tamaraw", config.daita?.paddingMachine)
        assertTrue(config.daita?.normalizePackets == true)
    }

    @Test
    fun `multi-hop on injects an entry hop`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(multiHop = true), mockCatalog())
        val config = builder.build(pubkey)!!

        assertNotNull(config.entryHop)
        assertTrue(config.entryHop?.relayEndpoint?.isNotEmpty() == true)
    }

    @Test
    fun `nat pmp toggle flows through`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(natPmp = true), mockCatalog())
        val config = builder.build(pubkey)!!
        assertTrue(config.natPmpEnabled)
    }

    @Test
    fun `obfuscation toggle flows through`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(obfuscation = true), mockCatalog())
        val config = builder.build(pubkey)!!
        assertTrue(config.obfuscationM40)
    }

    @Test
    fun `empty catalogue yields null config`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(), mockCatalog(emptyList()))
        assertNull(builder.build(pubkey))
    }

    @Test
    fun `selectedExitId picks the matching relay when present`() {
        val otherRelay = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            endpoint = "warren-exit-2.warren.brown:443",
        )
        val builder = WarrenTunnelConfigBuilder(
            mockRepo(selectedExitId = otherRelay.exitId),
            mockCatalog(listOf(sampleRelay, otherRelay)),
        )
        val config = builder.build(pubkey)!!
        assertEquals(otherRelay.exitPubkeyHex, config.exitPubkeyHex)
        assertEquals(otherRelay.endpoint, config.exitEndpoint)
    }

    @Test
    fun `selectedExitId falls back when target is inactive`() {
        val inactive = sampleRelay.copy(exitId = "deadbeefdeadbeefdeadbeefdeadbeef", active = false)
        val builder = WarrenTunnelConfigBuilder(
            mockRepo(selectedExitId = inactive.exitId),
            mockCatalog(listOf(sampleRelay, inactive)),
        )
        // Inactive selection falls back to the first active relay (sample).
        val config = builder.build(pubkey)!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `exit country selects a matching relay`() {
        val fr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-fr.warren.brown:443",
            country = "FR",
        )
        val config = WarrenTunnelConfigBuilder(
            mockRepo(exitCountry = "FR"),
            mockCatalog(listOf(sampleRelay, fr)),
        ).build(pubkey)!!
        assertEquals(fr.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `exit country falls back to first active when no relay matches`() {
        val config = WarrenTunnelConfigBuilder(
            mockRepo(exitCountry = "JP"),
            mockCatalog(listOf(sampleRelay)),
        ).build(pubkey)!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `multi-hop entry country picks a distinct entry relay in that country`() {
        val fr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-fr.warren.brown:443",
            country = "FR",
        )
        val config = WarrenTunnelConfigBuilder(
            mockRepo(multiHop = true, entryCountry = "FR"),
            mockCatalog(listOf(sampleRelay, fr)),
        ).build(pubkey)!!
        // Exit defaults to first active (DE sample); entry is the FR relay.
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
        assertEquals(fr.exitPubkeyHex, config.entryHop?.relayPubkeyHex)
    }

    @Test
    fun `nat-pmp parameters flow through to the config`() {
        val config = WarrenTunnelConfigBuilder(
            mockRepo(natPmp = true, natPmpProtocol = "tcp", natPmpExternalPort = 51820, natPmpLifetimeSecs = 21600),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.natPmpEnabled)
        assertEquals("tcp", config.natPmpProtocol)
        assertEquals(51820, config.natPmpExternalPort)
        assertEquals(21600, config.natPmpLifetimeSecs)
    }

    @Test
    fun `nat-pmp defaults are udp auto-port one-hour`() {
        val config = WarrenTunnelConfigBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertEquals("udp", config.natPmpProtocol)
        assertEquals(0, config.natPmpExternalPort)
        assertEquals(3600, config.natPmpLifetimeSecs)
    }

    @Test
    fun `ipv6 and lockdown default to the leak-safe values`() {
        val config = WarrenTunnelConfigBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertFalse(config.enableIpv6)
        assertFalse(config.lockdownMode)
    }

    @Test
    fun `ipv6 and lockdown toggles flow through`() {
        val config = WarrenTunnelConfigBuilder(
            mockRepo(ipv6 = true, lockdown = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.enableIpv6)
        assertTrue(config.lockdownMode)
    }

    @Test
    fun `allow lan defaults off and flows through when enabled`() {
        assertFalse(WarrenTunnelConfigBuilder(mockRepo(), mockCatalog()).build(pubkey)!!.allowLan)
        val config = WarrenTunnelConfigBuilder(
            mockRepo(allowLan = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.allowLan)
    }

    @Test
    fun `dns is null in default mode with no content blocking`() {
        val config = WarrenTunnelConfigBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertNull(config.dns)
    }

    @Test
    fun `custom dns servers flow into the config`() {
        val config = WarrenTunnelConfigBuilder(
            mockRepo(
                dnsState = WarrenLocalSettingsRepository.DNS_STATE_CUSTOM,
                customDns = listOf("9.9.9.9", "149.112.112.112"),
            ),
            mockCatalog(),
        ).build(pubkey)!!
        assertNotNull(config.dns)
        assertEquals("custom", config.dns?.state)
        assertEquals(listOf("9.9.9.9", "149.112.112.112"), config.dns?.customServers)
    }

    @Test
    fun `content blocking flags produce a default-mode dns config`() {
        val config = WarrenTunnelConfigBuilder(
            mockRepo(blockAds = true, blockMalware = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertNotNull(config.dns)
        assertEquals("default", config.dns?.state)
        assertTrue(config.dns?.blockAds == true)
        assertTrue(config.dns?.blockMalware == true)
        assertFalse(config.dns?.blockTrackers == true)
        assertTrue(config.dns?.customServers?.isEmpty() == true)
    }

    @Test
    fun `all flags on produces a fully-populated config`() {
        val builder = WarrenTunnelConfigBuilder(
            mockRepo(daita = true, natPmp = true, multiHop = true, obfuscation = true),
            mockCatalog(),
        )
        val config = builder.build(pubkey)!!

        assertNotNull(config.entryHop)
        assertNotNull(config.daita)
        assertTrue(config.natPmpEnabled)
        assertTrue(config.obfuscationM40)
        assertEquals(pubkey.value, config.walletPubkeyHex)
    }
}
