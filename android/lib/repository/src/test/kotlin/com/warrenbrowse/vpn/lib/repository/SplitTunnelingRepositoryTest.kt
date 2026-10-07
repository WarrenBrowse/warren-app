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
import com.warrenbrowse.vpn.lib.model.RoutingOp
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
    private val locked = MutableStateFlow<Set<String>>(emptySet())
    private val settings =
        mockk<WarrenLocalSettingsRepository> {
            every { splitMode } returns this@SplitTunnelingRepositoryTest.splitMode
            every { includedApps } returns included
            every { excludedApps } returns MutableStateFlow(setOf("org.chat"))
            every { appExits } returns this@SplitTunnelingRepositoryTest.appExits
            every { appExitsEnabled } returns this@SplitTunnelingRepositoryTest.appExitsEnabled
            every { lockedApps } returns locked
        }
    private val installed = setOf("org.bank", "org.mail", "org.chat")

    private fun repository() = SplitTunnelingRepository(settings, isAppInstalled = { it in installed })

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
    fun `a first rule asks before it narrows the full tunnel, and only then`() {
        val repository = repository()
        val firstCountry = listOf(RoutingOp.SetExit("org.mail", AppExit("de")))

        included.value = setOf("org.gone")
        val fromFullTunnel = repository.changeNarrowsTunnel(firstCountry)
        included.value = setOf("org.bank")
        val fromList = repository.changeNarrowsTunnel(firstCountry)

        assertTrue(fromFullTunnel)
        assertFalse(fromList)
    }

    @Test
    fun `a change reaches the settings one write at a time, in its order`() {
        val written = mutableListOf<String>()
        val recording =
            mockk<WarrenLocalSettingsRepository>(relaxed = true) {
                every { splitMode } returns this@SplitTunnelingRepositoryTest.splitMode
                every { includedApps } returns included
                every { excludedApps } returns MutableStateFlow(emptySet())
                every { appExits } returns this@SplitTunnelingRepositoryTest.appExits
                every { appExitsEnabled } returns this@SplitTunnelingRepositoryTest.appExitsEnabled
                every { lockedApps } returns MutableStateFlow(emptySet())
                every { lockApp(any()) } answers { written += "lock ${firstArg<Any>()}" }
                every { unlockApp(any()) } answers { written += "unlock ${firstArg<Any>()}" }
                every { setSplitMode(any()) } answers { written += "mode ${firstArg<Any>()}" }
                every { addExcludedApp(any()) } answers { written += "+excluded ${firstArg<Any>()}" }
                every { removeExcludedApp(any()) } answers { written += "-excluded ${firstArg<Any>()}" }
                every { addIncludedApp(any()) } answers { written += "+included ${firstArg<Any>()}" }
                every { removeIncludedApp(any()) } answers { written += "-included ${firstArg<Any>()}" }
                every { setAppExit(any(), any()) } answers { written += "exit ${firstArg<Any>()}" }
                every { clearAppExit(any()) } answers { written += "-exit ${firstArg<Any>()}" }
                every { setAppExitsEnabled(any()) } answers { written += "exits ${firstArg<Any>()}" }
            }
        val repository = SplitTunnelingRepository(recording, isAppInstalled = { true })

        repository.apply(
            listOf(
                RoutingOp.ClearExit("org.old"),
                RoutingOp.SetExit("org.mail", AppExit("de")),
                RoutingOp.SetExitsEnabled(true),
                RoutingOp.RemoveIncluded("org.mail"),
                RoutingOp.AddIncluded("org.bank"),
                RoutingOp.RemoveExcluded("org.chat"),
                RoutingOp.AddExcluded("org.game"),
                RoutingOp.SetSplitMode(SplitTunnelMode.Exclude),
                RoutingOp.Lock("org.bank"),
                RoutingOp.Unlock("org.mail"),
            )
        )

        assertEquals(
            listOf(
                "-exit org.old",
                "exit org.mail",
                "exits true",
                "-included org.mail",
                "+included org.bank",
                "-excluded org.chat",
                "+excluded org.game",
                "mode Exclude",
                "lock org.bank",
                "unlock org.mail",
            ),
            written,
        )
    }

    @Test
    fun `a lock brings the service up to hold the app, and nothing else does`() {
        val relaxed = mockk<WarrenLocalSettingsRepository>(relaxed = true) {
            every { splitMode } returns this@SplitTunnelingRepositoryTest.splitMode
            every { includedApps } returns included
            every { excludedApps } returns MutableStateFlow(emptySet())
            every { appExits } returns this@SplitTunnelingRepositoryTest.appExits
            every { appExitsEnabled } returns this@SplitTunnelingRepositoryTest.appExitsEnabled
            every { lockedApps } returns locked
        }
        var holds = 0
        val repository = SplitTunnelingRepository(relaxed, { true }) { holds++ }

        repository.apply(listOf(RoutingOp.AddIncluded("org.bank")))
        repository.apply(listOf(RoutingOp.Unlock("org.bank")))
        assertEquals(0, holds)

        repository.apply(listOf(RoutingOp.Lock("org.bank")))
        assertEquals(1, holds)
    }

    @Test
    fun `counts the locked apps on the device`() {
        locked.value = setOf("org.bank", "org.gone")

        val count = runBlocking {
            withTimeout(TIMEOUT_MS) { repository().lockedCount.first { it > 0 } }
        }

        assertEquals(1, count)
    }

    @Test
    fun `a locked app counts among the apps vpn only for carries`() {
        included.value = setOf("org.mail")
        locked.value = setOf("org.bank")

        assertEquals(2, repository().awaitCount { it == 2 })
    }

    private companion object {
        const val TIMEOUT_MS = 5_000L
    }
}
