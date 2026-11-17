package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceBridge
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class DeviceRevocationUnconfirmedNotificationUseCaseTest {

    private val bridge =
        object : WarrenDeviceBridge {
            override fun listDevices(mnemonic: String) = error("not called")

            override fun renameDevice(mnemonic: String, deviceId: String, name: String) =
                error("not called")

            override fun revokeDevice(mnemonic: String, deviceId: String) = error("not called")

            override fun deviceRevoked() = false

            override fun forgetDeviceRevocation() = Unit
        }

    private val walletState =
        MutableStateFlow<WalletState>(
            WalletState.Locked(WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"))
        )

    private val wallet =
        object : WalletRepository {
            override val state: StateFlow<WalletState> = walletState

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

    private val devices = DeviceRepository(bridge, wallet)
    private val useCase = DeviceRevocationUnconfirmedNotificationUseCase(devices, wallet)

    @Test
    fun `no banner without an unconfirmed revocation`() = runTest {
        useCase().test { assertNull(awaitItem()) }
    }

    @Test
    fun `an unconfirmed revocation raises the banner until the wallet leaves`() = runTest {
        useCase().test {
            assertNull(awaitItem())
            devices.showRevocationUnconfirmed()
            assertEquals(InAppNotification.DeviceRevocationUnconfirmed, awaitItem())
            walletState.value = WalletState.Absent
            assertNull(awaitItem())
        }
    }
}
