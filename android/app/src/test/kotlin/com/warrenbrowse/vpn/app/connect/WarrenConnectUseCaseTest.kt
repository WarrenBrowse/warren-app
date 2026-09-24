package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.ExitKeyVerdict
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * The failover config the drop retry dials: the desktop
 * `assemble_failover_for_attempt` behaviour with the exit-key pin enforced
 * the way a fresh connect enforces it.
 */
class WarrenConnectUseCaseTest {

    private val pubkey = WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")

    private val previous =
        WarrenTunnelConfig(
            exitPubkeyHex = "ab".repeat(32),
            exitEndpoint = "exit1.example:443",
            walletPubkeyHex = pubkey.value,
            exitId = "11".repeat(16),
        )

    private val alternative =
        previous.copy(
            exitPubkeyHex = "cd".repeat(32),
            exitEndpoint = "exit2.example:443",
            exitId = "22".repeat(16),
        )

    private fun useCase(
        built: WarrenTunnelConfig?,
        verdict: ExitKeyVerdict = ExitKeyVerdict.Match,
        wallet: WalletState = WalletState.Ready(pubkey),
        localSettings: WarrenLocalSettingsRepository = mockk(relaxed = true),
    ): WarrenConnectUseCase {
        val walletRepository: WalletRepository = mockk()
        every { walletRepository.state } returns MutableStateFlow(wallet)
        val builder: WarrenTunnelConfigBuilder = mockk()
        every { builder.buildFailover(previous) } returns built
        every { localSettings.exitKeyVerdict(any(), any()) } returns verdict
        every { localSettings.allowLan } returns MutableStateFlow(true)
        every { localSettings.tunnelMtu } returns MutableStateFlow(1400)
        return WarrenConnectUseCase(walletRepository, builder, localSettings, mockk<ConnectionProxy>())
    }

    @Test
    fun `a failover config dials the alternative with the live local toggles applied`() {
        val config = useCase(alternative).buildFailoverConfig(previous)!!
        assertEquals(alternative.exitPubkeyHex, config.exitPubkeyHex)
        assertEquals(true, config.allowLan)
        assertEquals(1400, config.mtu)
    }

    @Test
    fun `no alternative yields no failover config`() {
        // The builder hands back nothing when the pin leaves no other exit:
        // that is a plain retry of the previous config.
        assertNull(useCase(null).buildFailoverConfig(previous))
    }

    @Test
    fun `an alternative whose key changed since it was pinned is refused`() {
        assertNull(
            useCase(alternative, verdict = ExitKeyVerdict.Mismatch("ee".repeat(32)))
                .buildFailoverConfig(previous)
        )
    }

    @Test
    fun `a drain has somewhere to go when the failover would dial another exit`() {
        assertEquals(true, useCase(alternative).hasFailoverExit(previous))
        assertEquals(false, useCase(null).hasFailoverExit(previous))
        assertEquals(
            false,
            useCase(alternative, verdict = ExitKeyVerdict.Mismatch("ee".repeat(32)))
                .hasFailoverExit(previous),
        )
    }

    @Test
    fun `asking whether a drain has somewhere to go pins nothing`() {
        // It is asked on every dial, well before any failover: a first
        // sighting is pinned when the failover actually dials it.
        val localSettings: WarrenLocalSettingsRepository = mockk(relaxed = true)
        useCase(alternative, verdict = ExitKeyVerdict.FirstSeen, localSettings = localSettings)
            .hasFailoverExit(previous)
        verify(exactly = 0) { localSettings.trustExitKey(any(), any()) }
    }

    @Test
    fun `a first-seen alternative is pinned like a fresh connect would`() {
        val localSettings: WarrenLocalSettingsRepository = mockk(relaxed = true)
        val config =
            useCase(alternative, verdict = ExitKeyVerdict.FirstSeen, localSettings = localSettings)
                .buildFailoverConfig(previous)
        assertEquals(alternative.exitPubkeyHex, config?.exitPubkeyHex)
        verify { localSettings.trustExitKey(alternative.exitId!!, alternative.exitPubkeyHex) }
    }
}
