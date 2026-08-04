package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.WarrenPathHealthProvider
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ExitEgressNotificationUseCaseTest {

    private val pathWedged = MutableStateFlow(false)
    private val tunnelState = MutableStateFlow<TunnelState>(TunnelState.Disconnected())
    private val connectionProxy: ConnectionProxy = mockk()

    private val pathHealthProvider =
        object : WarrenPathHealthProvider {
            override val pathWedged: StateFlow<Boolean>
                get() = this@ExitEgressNotificationUseCaseTest.pathWedged
        }

    private fun useCase(): ExitEgressNotificationUseCase {
        every { connectionProxy.tunnelState } returns tunnelState
        return ExitEgressNotificationUseCase(pathHealthProvider, connectionProxy)
    }

    private fun connected() =
        TunnelState.Connected(
            endpoint = mockk(),
            location = null,
            featureIndicators = emptyList(),
        )

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    @Test
    fun `a healthy tunnel raises nothing`() = runTest {
        tunnelState.value = connected()

        useCase()().test { assertNull(awaitItem()) }
    }

    @Test
    fun `a wedged datapath on a connected tunnel names the cause`() = runTest {
        tunnelState.value = connected()

        useCase()().test {
            assertNull(awaitItem())
            pathWedged.value = true
            assertEquals(InAppNotification.ExitEgressDead, awaitItem())
        }
    }

    @Test
    fun `a wedge verdict left over from a torn down tunnel raises nothing`() = runTest {
        pathWedged.value = true
        tunnelState.value = TunnelState.Disconnected()

        useCase()().test { assertNull(awaitItem()) }
    }

    @Test
    fun `the wedge clearing drops the banner`() = runTest {
        tunnelState.value = connected()
        pathWedged.value = true

        useCase()().test {
            assertEquals(InAppNotification.ExitEgressDead, awaitItem())
            pathWedged.value = false
            assertNull(awaitItem())
        }
    }

    @Test
    fun `the cause banner ranks under the offline banner and over the tunnel state banners`() {
        assertEquals(
            true,
            InAppNotification.ExitEgressDead.priority < InAppNotification.HostOffline.priority,
        )
        assertEquals(
            true,
            InAppNotification.ExitEgressDead.priority >
                InAppNotification.TunnelStateBlocked.priority,
        )
    }
}
