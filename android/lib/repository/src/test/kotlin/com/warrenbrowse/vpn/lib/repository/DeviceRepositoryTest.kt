package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.DeviceState
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

class DeviceRepositoryTest {
    private class FakeDevices(var revoked: Boolean) : WarrenDeviceBridge {
        override fun listDevices(mnemonic: String) = error("not called")

        override fun renameDevice(mnemonic: String, deviceId: String, name: String) =
            error("not called")

        override fun revokeDevice(mnemonic: String, deviceId: String) = error("not called")

        override fun deviceRevoked() = revoked

        override fun forgetDeviceRevocation() {
            revoked = false
        }
    }

    @Test
    fun `a device the registry revoked emits its revocation`() = runTest {
        val repository = DeviceRepository(FakeDevices(revoked = true))

        repository.updateDevice()

        assertEquals(DeviceState.Revoked, repository.deviceState.value)
    }

    @Test
    fun `a device still registered emits nothing`() = runTest {
        val repository = DeviceRepository(FakeDevices(revoked = false))

        repository.updateDevice()

        assertNull(repository.deviceState.value)
    }
}
