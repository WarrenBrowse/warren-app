package com.warrenbrowse.vpn.lib.repository

import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.PackageName
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
    private val appExits = MutableStateFlow<Map<String, AppExit>>(emptyMap())
    private val appExitsEnabled = MutableStateFlow(true)
    private val settings =
        mockk<WarrenLocalSettingsRepository> {
            every { splitMode } returns this@SplitTunnelingRepositoryTest.splitMode
            every { includedApps } returns included
            every { excludedApps } returns MutableStateFlow(setOf("org.chat"))
            every { appExits } returns this@SplitTunnelingRepositoryTest.appExits
            every { appExitsEnabled } returns this@SplitTunnelingRepositoryTest.appExitsEnabled
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

    @Test
    fun `an app with a country counts among the apps vpn only for carries`() {
        val repository = repository()
        repository.awaitCount { it == 2 }

        appExits.value = mapOf("org.chat" to AppExit("de"))

        assertEquals(3, repository.awaitCount { it != 2 })
    }

    @Test
    fun `a country switched off counts nothing`() {
        appExits.value = mapOf("org.chat" to AppExit("de"))
        appExitsEnabled.value = false

        assertEquals(2, repository().awaitCount { it != null })
    }

    @Test
    fun `a first country asks before it narrows the full tunnel, and only then`() {
        val repository = repository()

        included.value = setOf("org.gone")
        val fromFullTunnel = repository.countryChoiceNarrowsFullTunnel(PackageName("org.mail"), AppExit("de"))
        included.value = setOf("org.bank")
        val fromList = repository.countryChoiceNarrowsFullTunnel(PackageName("org.mail"), AppExit("de"))

        assertTrue(fromFullTunnel)
        assertFalse(fromList)
    }

    @Test
    fun `turning the country switch on asks before it narrows the full tunnel, and only then`() {
        val repository = repository()
        appExits.value = mapOf("org.mail" to AppExit("de"))
        appExitsEnabled.value = false

        included.value = setOf("org.gone")
        val fromFullTunnel = repository.appExitsSwitchNarrowsFullTunnel()
        included.value = setOf("org.bank")
        val fromList = repository.appExitsSwitchNarrowsFullTunnel()

        assertTrue(fromFullTunnel)
        assertFalse(fromList)
    }

    private companion object {
        const val TIMEOUT_MS = 5_000L
    }
}
