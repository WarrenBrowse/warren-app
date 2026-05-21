package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex
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

    private val pubkey = WalletPubkeyHex("a".repeat(64))

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
    ): WarrenLocalSettingsRepository {
        val repo: WarrenLocalSettingsRepository = mockk()
        every { repo.daitaEnabled } returns MutableStateFlow(daita)
        every { repo.natPmpEnabled } returns MutableStateFlow(natPmp)
        every { repo.multiHopEnabled } returns MutableStateFlow(multiHop)
        every { repo.obfuscationM40 } returns MutableStateFlow(obfuscation)
        every { repo.selectedExitId } returns MutableStateFlow(selectedExitId)
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
