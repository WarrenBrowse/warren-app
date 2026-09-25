package com.warrenbrowse.vpn.lib.repository

import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import org.junit.jupiter.api.Test

/**
 * The count the connect screen shows under the connection state. It follows
 * what the tunnel carries, not what the list holds: an uninstalled app is not
 * protected, and a list with nothing installed leaves a full tunnel, which is
 * not "VPN only for" anything.
 */
class SplitTunnelingRepositoryTest {

    private val splitMode = MutableStateFlow(SplitTunnelMode.IncludeOnly)
    private val included = MutableStateFlow(setOf("org.bank", "org.mail", "org.gone"))
    private val settings =
        mockk<WarrenLocalSettingsRepository> {
            every { splitMode } returns this@SplitTunnelingRepositoryTest.splitMode
            every { includedApps } returns included
            every { excludedApps } returns MutableStateFlow(setOf("org.chat"))
        }
    private val installed = setOf("org.bank", "org.mail", "org.chat")

    private fun repository() = SplitTunnelingRepository(settings) { it in installed }

    private fun SplitTunnelingRepository.awaitCount(predicate: (Int?) -> Boolean): Int? =
        runBlocking { withTimeout(TIMEOUT_MS) { vpnOnlyForCount.first(predicate) } }

    @Test
    fun `vpn only for counts the included apps on the device`() {
        assertEquals(2, repository().awaitCount { it != null })
    }

    @Test
    fun `the count follows the list`() {
        val repository = repository()
        repository.awaitCount { it == 2 }

        included.value = setOf("org.bank")

        assertEquals(1, repository.awaitCount { it != 2 })
    }

    @Test
    fun `vpn only for with nothing installed shows no count`() {
        val repository = repository()
        repository.awaitCount { it == 2 }

        included.value = setOf("org.gone", "org.removed")

        assertEquals(null, repository.awaitCount { it != 2 })
    }

    @Test
    fun `no count outside vpn only for`() {
        val repository = repository()
        repository.awaitCount { it == 2 }

        splitMode.value = SplitTunnelMode.Exclude

        assertEquals(null, repository.awaitCount { it == null })
    }

    private companion object {
        const val TIMEOUT_MS = 5_000L
    }
}
