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

    private fun mockRepo(
        daita: Boolean = false,
        natPmp: Boolean = false,
        multiHop: Boolean = false,
        obfuscation: Boolean = false,
    ): WarrenLocalSettingsRepository {
        val repo: WarrenLocalSettingsRepository = mockk()
        every { repo.daitaEnabled } returns MutableStateFlow(daita)
        every { repo.natPmpEnabled } returns MutableStateFlow(natPmp)
        every { repo.multiHopEnabled } returns MutableStateFlow(multiHop)
        every { repo.obfuscationM40 } returns MutableStateFlow(obfuscation)
        return repo
    }

    @Test
    fun `default config has no entry hop and no daita`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo())
        val config = builder.build(pubkey)

        assertNull(config.entryHop)
        assertNull(config.daita)
        assertFalse(config.natPmpEnabled)
        assertFalse(config.obfuscationM40)
        assertEquals(pubkey.value, config.walletPubkeyHex)
    }

    @Test
    fun `daita on injects a Tamaraw spec`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(daita = true))
        val config = builder.build(pubkey)

        assertNotNull(config.daita)
        assertEquals("tamaraw", config.daita?.paddingMachine)
        assertTrue(config.daita?.normalizePackets == true)
    }

    @Test
    fun `multi-hop on injects an entry hop`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(multiHop = true))
        val config = builder.build(pubkey)

        assertNotNull(config.entryHop)
        assertTrue(config.entryHop?.relayEndpoint?.isNotEmpty() == true)
    }

    @Test
    fun `nat pmp toggle flows through`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(natPmp = true))
        val config = builder.build(pubkey)
        assertTrue(config.natPmpEnabled)
    }

    @Test
    fun `obfuscation toggle flows through`() {
        val builder = WarrenTunnelConfigBuilder(mockRepo(obfuscation = true))
        val config = builder.build(pubkey)
        assertTrue(config.obfuscationM40)
    }

    @Test
    fun `all flags on produces a fully-populated config`() {
        val builder = WarrenTunnelConfigBuilder(
            mockRepo(daita = true, natPmp = true, multiHop = true, obfuscation = true)
        )
        val config = builder.build(pubkey)

        assertNotNull(config.entryHop)
        assertNotNull(config.daita)
        assertTrue(config.natPmpEnabled)
        assertTrue(config.obfuscationM40)
        assertEquals(pubkey.value, config.walletPubkeyHex)
    }
}
