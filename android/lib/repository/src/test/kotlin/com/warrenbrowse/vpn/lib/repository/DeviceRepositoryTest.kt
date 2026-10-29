package com.warrenbrowse.vpn.lib.repository

import arrow.core.left
import arrow.core.right
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import java.time.LocalDate
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

class DeviceRepositoryTest {
    private class FakeDevices(var revoked: Boolean = false) : WarrenDeviceBridge {
        var answer = """{"ok":true,"devices":[],"older_client_seen_on":null}"""
        var thrown: RuntimeException? = null
        val phrases = mutableListOf<String>()
        val calls = mutableListOf<String>()

        private fun answer(call: String, mnemonic: String): String {
            thrown?.let { throw it }
            phrases += mnemonic
            calls += call
            return answer
        }

        override fun listDevices(mnemonic: String) = answer("list", mnemonic)

        override fun renameDevice(mnemonic: String, deviceId: String, name: String) =
            answer("rename $deviceId $name", mnemonic)

        override fun revokeDevice(mnemonic: String, deviceId: String) =
            answer("revoke $deviceId", mnemonic)

        override fun deviceRevoked() = revoked

        override fun forgetDeviceRevocation() {
            revoked = false
        }
    }

    private class FakeWallet(var present: Boolean = true) : WalletRepository {
        override val state: StateFlow<WalletState> = MutableStateFlow(WalletState.Absent)

        override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic =
            error("unused")

        override suspend fun importWallet(mnemonic: Mnemonic, authorizer: SensitiveOpAuthorizer?) =
            error("unused")

        override suspend fun unlock(authorizer: SensitiveOpAuthorizer, reason: String): Mnemonic =
            error("unused")

        override suspend fun readMnemonic(): Mnemonic =
            if (present) Mnemonic(PHRASE) else throw IllegalStateException("no wallet")

        override suspend fun erase() = error("unused")
    }

    private val bridge = FakeDevices()
    private val wallet = FakeWallet()

    private fun repository(dispatcher: kotlinx.coroutines.CoroutineDispatcher) =
        DeviceRepository(bridge, wallet, dispatcher)

    @Test
    fun `a revocation shown is consumed natively so it is acted on once`() {
        bridge.revoked = true
        val repository = DeviceRepository(bridge)

        repository.showRevoked()

        assertEquals(DeviceState.Revoked, repository.deviceState.value)
        assertFalse(repository.revokedByRegistry())
    }

    @Test
    fun `a device still registered reads as not revoked`() {
        val repository = DeviceRepository(bridge)

        assertFalse(repository.revokedByRegistry())
        assertNull(repository.deviceState.value)
    }

    @Test
    fun `a dropped revocation shows no screen`() {
        bridge.revoked = true
        val repository = DeviceRepository(bridge)
        assertTrue(repository.revokedByRegistry())

        repository.dropRevocation()

        assertFalse(repository.revokedByRegistry())
        assertNull(repository.deviceState.value)
    }

    @Test
    fun `leaving the revoked screen clears the revocation`() {
        bridge.revoked = true
        val repository = DeviceRepository(bridge)
        repository.showRevoked()

        repository.acknowledgeRevocation()

        assertNull(repository.deviceState.value)
    }

    @Test
    fun `the list is read off the envelope, signed with the wallet on the device`() = runTest {
        bridge.answer =
            """{"ok":true,"devices":[{"device_id":"AAAAAAAAAAAAAAAAAAAAAA","name":"brave otter",""" +
                """"platform":"android","app_version":"1.2.0","registered_on":"2026-10-01",""" +
                """"last_seen_on":"2026-10-28","this_device":true}],""" +
                """"older_client_seen_on":"2026-10-27"}"""

        val list = repository(StandardTestDispatcher(testScheduler)).listDevices().getOrNull()!!

        assertEquals(listOf(PHRASE), bridge.phrases)
        val device = list.devices.single()
        assertEquals("AAAAAAAAAAAAAAAAAAAAAA", device.id)
        assertEquals("brave otter", device.name)
        assertEquals("android", device.platform)
        assertEquals("1.2.0", device.appVersion)
        assertEquals(LocalDate.of(2026, 10, 28), device.lastSeenOn)
        assertTrue(device.thisDevice)
        assertEquals(LocalDate.of(2026, 10, 27), list.olderClientSeenOn)
    }

    @Test
    fun `an account no older client held reads no older-client day`() = runTest {
        val list = repository(StandardTestDispatcher(testScheduler)).listDevices().getOrNull()!!

        assertNull(list.olderClientSeenOn)
    }

    @Test
    fun `every refusal maps to its error and an unknown one reads as unavailable`() = runTest {
        val repository = repository(StandardTestDispatcher(testScheduler))
        val cases =
            mapOf(
                "no_wallet" to DeviceRegistryError.NoWallet,
                "invalid_device" to DeviceRegistryError.InvalidDevice,
                "invalid_name" to DeviceRegistryError.InvalidName,
                "not_found" to DeviceRegistryError.NotFound,
                "revoked" to DeviceRegistryError.Revoked,
                "unavailable" to DeviceRegistryError.Unavailable,
                "something_new" to DeviceRegistryError.Unavailable,
            )
        cases.forEach { (code, error) ->
            bridge.answer = """{"ok":false,"error":"$code"}"""
            assertEquals(error.left(), repository.listDevices(), code)
        }
    }

    @Test
    fun `a malformed envelope or a throwing bridge reads as unavailable`() = runTest {
        val repository = repository(StandardTestDispatcher(testScheduler))
        bridge.answer = "not json"
        assertEquals(DeviceRegistryError.Unavailable.left(), repository.listDevices())

        bridge.thrown = IllegalStateException("native library not loaded")
        assertEquals(DeviceRegistryError.Unavailable.left(), repository.revokeDevice("x"))
    }

    @Test
    fun `no wallet on the device asks nothing`() = runTest {
        wallet.present = false

        val result = repository(StandardTestDispatcher(testScheduler)).listDevices()

        assertEquals(DeviceRegistryError.NoWallet.left(), result)
        assertEquals(emptyList<String>(), bridge.calls)
    }

    @Test
    fun `rename and revoke name the device and say whether it is this one`() = runTest {
        val repository = repository(StandardTestDispatcher(testScheduler))
        bridge.answer = """{"ok":true,"this_device":true}"""

        assertEquals(true.right(), repository.revokeDevice("dev1"))
        bridge.answer = """{"ok":true,"this_device":false}"""
        assertEquals(false.right(), repository.renameDevice("dev2", "kitchen"))

        assertEquals(listOf("revoke dev1", "rename dev2 kitchen"), bridge.calls)
    }

    private companion object {
        const val PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"
    }
}
