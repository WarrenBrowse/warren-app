package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
@ExtendWith(TestCoroutineRule::class)
class ConnectingStuckNotificationUseCaseTest {

    private val tunnelState = MutableStateFlow<TunnelState>(TunnelState.Disconnected())
    private val connectionProxy: ConnectionProxy = mockk()

    private fun useCase(): ConnectingStuckNotificationUseCase {
        every { connectionProxy.tunnelState } returns tunnelState
        return ConnectingStuckNotificationUseCase(connectionProxy)
    }

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    private fun connecting() =
        TunnelState.Connecting(endpoint = null, location = null, featureIndicators = emptyList())

    @Test
    fun `a disconnected tunnel never raises the banner`() = runTest {
        useCase()().test {
            assertNull(awaitItem())
            advanceTimeBy(120.seconds)
            expectNoEvents()
        }
    }

    @Test
    fun `a connect that completes inside the window never raises the banner`() = runTest {
        useCase()().test {
            assertNull(awaitItem())
            tunnelState.value = connecting()
            advanceTimeBy(30.seconds)
            tunnelState.value =
                TunnelState.Connected(
                    endpoint = mockk(),
                    location = null,
                    featureIndicators = emptyList(),
                )
            advanceTimeBy(120.seconds)
            expectNoEvents()
        }
    }

    // awaitItem() lets runTest auto-advance virtual time, so the arrival alone
    // proves nothing about WHEN the banner is due. The elapsed virtual time is
    // what pins the window: a longer one is only reached by running the
    // scheduler further, and the assertion catches exactly that. It is a delta,
    // because the test dispatcher's clock is shared by the whole class.
    @Test
    fun `a connect still running past the stuck window raises the banner`() = runTest {
        val scheduler = testScheduler
        useCase()().test {
            assertNull(awaitItem())
            val start = scheduler.currentTime
            tunnelState.value = connecting()
            advanceTimeBy(44.seconds)
            expectNoEvents()
            advanceTimeBy(2.seconds)
            assertEquals(InAppNotification.ConnectingStuck, awaitItem())
            assertEquals(46.seconds.inWholeMilliseconds, scheduler.currentTime - start)
        }
    }

    @Test
    fun `leaving the connecting phase drops the banner`() = runTest {
        val scheduler = testScheduler
        useCase()().test {
            assertNull(awaitItem())
            val start = scheduler.currentTime
            tunnelState.value = connecting()
            advanceTimeBy(46.seconds)
            assertEquals(InAppNotification.ConnectingStuck, awaitItem())
            assertEquals(46.seconds.inWholeMilliseconds, scheduler.currentTime - start)
            tunnelState.value = TunnelState.Disconnected()
            assertNull(awaitItem())
        }
    }

    @Test
    fun `a reconnect counts as connecting so a stuck redial is surfaced too`() = runTest {
        val scheduler = testScheduler
        useCase()().test {
            assertNull(awaitItem())
            val start = scheduler.currentTime
            tunnelState.value = TunnelState.Disconnecting(ActionAfterDisconnect.Reconnect)
            advanceTimeBy(46.seconds)
            assertEquals(InAppNotification.ConnectingStuck, awaitItem())
            assertEquals(46.seconds.inWholeMilliseconds, scheduler.currentTime - start)
        }
    }

    @Test
    fun `the timer is not restarted by a redial hop inside one attempt`() = runTest {
        val scheduler = testScheduler
        useCase()().test {
            assertNull(awaitItem())
            val start = scheduler.currentTime
            tunnelState.value = TunnelState.Disconnecting(ActionAfterDisconnect.Reconnect)
            advanceTimeBy(30.seconds)
            tunnelState.value = connecting()
            advanceTimeBy(16.seconds)
            assertEquals(InAppNotification.ConnectingStuck, awaitItem())
            // 46 s total, not 30 + 45: a redial inside one attempt must not
            // push the help hint out of reach.
            assertEquals(46.seconds.inWholeMilliseconds, scheduler.currentTime - start)
        }
    }

    @Test
    fun `the banner outranks the plain blocked banner and ranks under the error banner`() {
        val error = ErrorState(cause = ErrorStateCause.StartTunnelError, isBlocking = true)
        assertEquals(
            true,
            InAppNotification.ConnectingStuck.priority >
                InAppNotification.TunnelStateBlocked.priority,
        )
        assertEquals(
            true,
            InAppNotification.ConnectingStuck.priority <
                InAppNotification.TunnelStateError(error).priority,
        )
    }
}
