package com.warrenbrowse.vpn.lib.usecase

import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceBridge
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class DeviceRevocationUseCaseTest {

    private val events = mutableListOf<String>()

    private val bridge =
        object : WarrenDeviceBridge {
            var revoked = false

            override fun listDevices(mnemonic: String) = error("not called")

            override fun renameDevice(mnemonic: String, deviceId: String, name: String) =
                error("not called")

            override fun revokeDevice(mnemonic: String, deviceId: String) = error("not called")

            override fun deviceRevoked() = revoked

            override fun forgetDeviceRevocation() {
                revoked = false
            }
        }

    private val devices = DeviceRepository(bridge)

    private val wallet =
        object : WalletRepository {
            val flow =
                MutableStateFlow<WalletState>(
                    WalletState.Locked(
                        WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
                    )
                )
            override val state: StateFlow<WalletState> = flow

            override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic =
                error("unused")

            override suspend fun importWallet(
                mnemonic: Mnemonic,
                authorizer: SensitiveOpAuthorizer?,
            ) = error("unused")

            override suspend fun unlock(
                authorizer: SensitiveOpAuthorizer,
                reason: String,
            ): Mnemonic = error("unused")

            override suspend fun readMnemonic(): Mnemonic = error("unused")

            override suspend fun erase() = error("unused")
        }

    private var logoutOutcome: WarrenLogoutOutcome = WarrenLogoutOutcome.LoggedOut

    private val useCase =
        DeviceRevocationUseCase(devices, wallet) {
            events += "logout while ${devices.deviceState.value}"
            logoutOutcome
        }

    @Test
    fun `a revocation logs the wallet out before the revoked screen shows`() = runTest {
        bridge.revoked = true

        useCase.takeInRevocation()

        assertEquals(listOf("logout while null"), events)
        assertEquals(DeviceState.Revoked, devices.deviceState.value)
    }

    @Test
    fun `a logout that did not complete is tried again on the next look`() = runTest {
        bridge.revoked = true
        logoutOutcome = WarrenLogoutOutcome.TunnelStillUp

        useCase.takeInRevocation()

        assertNull(devices.deviceState.value)
        assertTrue(bridge.revoked, "the revocation must stay pending")

        logoutOutcome = WarrenLogoutOutcome.LoggedOut
        useCase.takeInRevocation()

        assertEquals(2, events.size)
        assertEquals(DeviceState.Revoked, devices.deviceState.value)
    }

    @Test
    fun `a revocation learned once the wallet left is dropped without a screen`() = runTest {
        bridge.revoked = true
        wallet.flow.value = WalletState.Absent

        useCase.takeInRevocation()

        assertEquals(emptyList<String>(), events)
        assertNull(devices.deviceState.value)
        assertTrue(!bridge.revoked)
    }

    @Test
    fun `a device still registered is left alone`() = runTest {
        useCase.takeInRevocation()

        assertEquals(emptyList<String>(), events)
        assertNull(devices.deviceState.value)
    }

    @Test
    fun `a revocation is taken in on the tunnel's wake without waiting for the period`() =
        runTest {
            val wakes = MutableSharedFlow<Any?>()
            val watching = launch { useCase.watch(wakes) }
            runCurrent()

            // The native side ended the session on the revocation: the tunnel
            // state moves, which is the wake.
            bridge.revoked = true
            advanceTimeBy(1.seconds)
            wakes.emit(Unit)
            runCurrent()

            assertEquals(DeviceState.Revoked, devices.deviceState.value)
            watching.cancel()
        }

    @Test
    fun `a revocation learned with no tunnel is taken in within the period`() = runTest {
        val watching = launch { useCase.watch(MutableSharedFlow()) }
        runCurrent()

        bridge.revoked = true
        advanceTimeBy(DeviceRevocationUseCase.PERIOD - 1.seconds)
        assertNull(devices.deviceState.value)
        advanceTimeBy(2.seconds)

        assertEquals(DeviceState.Revoked, devices.deviceState.value)
        watching.cancel()
    }
}
