package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.DeviceState
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
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
    fun `a revocation shown is consumed natively so it is acted on once`() {
        val bridge = FakeDevices(revoked = true)
        val repository = DeviceRepository(bridge)

        repository.showRevoked()

        assertEquals(DeviceState.Revoked, repository.deviceState.value)
        assertFalse(repository.revokedByRegistry())
    }

    @Test
    fun `a device still registered reads as not revoked`() {
        val repository = DeviceRepository(FakeDevices(revoked = false))

        assertFalse(repository.revokedByRegistry())
        assertNull(repository.deviceState.value)
    }

    @Test
    fun `a dropped revocation shows no screen`() {
        val bridge = FakeDevices(revoked = true)
        val repository = DeviceRepository(bridge)
        assertTrue(repository.revokedByRegistry())

        repository.dropRevocation()

        assertFalse(repository.revokedByRegistry())
        assertNull(repository.deviceState.value)
    }

    @Test
    fun `leaving the revoked screen clears the revocation`() {
        val repository = DeviceRepository(FakeDevices(revoked = true))
        repository.showRevoked()

        repository.acknowledgeRevocation()

        assertNull(repository.deviceState.value)
    }
}
