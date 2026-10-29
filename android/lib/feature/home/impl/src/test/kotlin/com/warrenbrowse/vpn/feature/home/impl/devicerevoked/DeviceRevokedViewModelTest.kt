package com.warrenbrowse.vpn.feature.home.impl.devicerevoked

import app.cash.turbine.test
import arrow.core.right
import io.mockk.MockKAnnotations
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceBridge
import kotlin.test.assertFalse
import kotlin.test.assertNull
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExperimentalCoroutinesApi
@ExtendWith(TestCoroutineRule::class)
class DeviceRevokedViewModelTest {

    private val bridge =
        object : WarrenDeviceBridge {
            var revoked = true

            override fun listDevices(mnemonic: String) = error("not called")

            override fun renameDevice(mnemonic: String, deviceId: String, name: String) =
                error("not called")

            override fun revokeDevice(mnemonic: String, deviceId: String) = error("not called")

            override fun deviceRevoked() = revoked

            override fun forgetDeviceRevocation() {
                revoked = false
            }
        }

    private val deviceRepository = DeviceRepository(bridge)

    @MockK private lateinit var mockConnectionProxy: ConnectionProxy

    private lateinit var viewModel: DeviceRevokedViewModel

    private val tunnelStateFlow = MutableSharedFlow<TunnelState>()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
        every { mockConnectionProxy.tunnelState } returns tunnelStateFlow
        viewModel =
            DeviceRevokedViewModel(
                deviceRepository = deviceRepository,
                connectionProxy = mockConnectionProxy,
            )
    }

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    @Test
    fun `when tunnel state is secured uiState should be SECURED`() = runTest {
        // Arrange
        val tunnelState: TunnelState = mockk()
        every { tunnelState.isSecured() } returns true

        // Act, Assert
        viewModel.uiState.test {
            assertEquals(DeviceRevokedUiState.UNKNOWN, awaitItem())
            tunnelStateFlow.emit(tunnelState)
            assertEquals(DeviceRevokedUiState.SECURED, awaitItem())
        }
    }

    @Test
    fun `going to login clears the revocation the next login must not find`() {
        coEvery { mockConnectionProxy.disconnect(any()) } returns true.right()
        deviceRepository.showRevoked()
        bridge.revoked = true

        viewModel.onGoToLoginClicked()

        assertNull(deviceRepository.deviceState.value)
        assertFalse(bridge.revoked)
    }

    @Test
    fun `going to login lifts what still holds the traffic`() {
        coEvery { mockConnectionProxy.disconnect(any()) } returns true.right()

        viewModel.onGoToLoginClicked()

        coVerify { mockConnectionProxy.disconnect(DisconnectReason.USER_INITIATED_GO_TO_LOGIN) }
    }
}
