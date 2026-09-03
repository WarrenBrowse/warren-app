package com.warrenbrowse.vpn.screen.outoftime

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

/**
 * The lapsed verdict the app shell collects instead of the expiry, the
 * tunnel state and a clock of its own.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@ExtendWith(TestCoroutineRule::class)
class OutOfTimeGateViewModelTest {

    private val expiry = MutableStateFlow(0L)
    private val connectedInfo = MutableStateFlow<WarrenConnectedInfo>(WarrenConnectedInfo.Disconnected)
    private var now = 1_000_000L

    private fun viewModel(): OutOfTimeGateViewModel {
        val settings = mockk<WarrenLocalSettingsRepository>()
        every { settings.cachedSubscriptionExpiry } returns expiry
        val tunnel = mockk<WarrenTunnelStateProvider>()
        every { tunnel.connectedInfo } returns connectedInfo
        return OutOfTimeGateViewModel(settings, tunnel, nowSecs = { now })
    }

    @Test
    fun `a subscription still running is not lapsed`() = runTest {
        expiry.value = now + 3_600
        viewModel().lapsed.test {
            assertFalse(awaitItem())
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `the exit refusing the account lapses the verdict without an expiry`() = runTest {
        val vm = viewModel()
        vm.lapsed.test {
            assertFalse(awaitItem(), "an unknown expiry alone is not a lockout")

            connectedInfo.value = WarrenConnectedInfo.Failed("subscription expired", expired = true)

            assertTrue(awaitItem())
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `a tunnel transition that changes nothing about the account emits nothing`() = runTest {
        // The whole point of the view model: the shell must not recompose on
        // every Connecting / Connected / Disconnecting edge.
        expiry.value = now + 3_600
        viewModel().lapsed.test {
            assertFalse(awaitItem())

            connectedInfo.value = WarrenConnectedInfo.Connecting()
            connectedInfo.value =
                WarrenConnectedInfo.Connected("exit.example:443", null, true, false, null)
            connectedInfo.value = WarrenConnectedInfo.Disconnected

            expectNoEvents()
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `the verdict flips on the expiry boundary while the app is open`() = runTest {
        expiry.value = now + 30
        val vm = viewModel()
        vm.lapsed.test {
            assertFalse(awaitItem())

            // The clock passes the boundary; the re-evaluation is armed on it.
            now += 30
            advanceTimeBy(30_001)

            assertTrue(awaitItem())
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `credit landing clears the verdict at once`() = runTest {
        expiry.value = now - 60
        viewModel().lapsed.test {
            assertTrue(awaitItem())

            expiry.value = now + 3_600

            assertFalse(awaitItem())
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `the re-evaluation lands on the boundary when it is near and on the tick otherwise`() {
        assertEquals(30L, reevaluationDelaySecs(expiryUnixSecs = now + 30, nowSecs = now))
        assertEquals(OUT_OF_TIME_TICK_SECS, reevaluationDelaySecs(now + 3_600, now))
        assertEquals(OUT_OF_TIME_TICK_SECS, reevaluationDelaySecs(now - 5, now))
        assertEquals(OUT_OF_TIME_TICK_SECS, reevaluationDelaySecs(0L, now))
    }
}
