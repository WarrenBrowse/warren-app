package com.warrenbrowse.vpn.feature.managedevices.impl

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceBridge
import com.warrenbrowse.vpn.lib.usecase.DeviceRevocationUseCase
import com.warrenbrowse.vpn.lib.usecase.WarrenLogoutOutcome
import java.time.LocalDate
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.EnumSource

@OptIn(ExperimentalCoroutinesApi::class)
@ExtendWith(TestCoroutineRule::class)
class ManageDevicesViewModelTest {

    private val calls = mutableListOf<String>()

    /** The native registry: the system boundary these tests stand in for. */
    private val bridge =
        object : WarrenDeviceBridge {
            var list = listEnvelope(emptyList())
            var actionAnswer = """{"ok":true,"this_device":false}"""
            var revoked = false

            override fun listDevices(mnemonic: String): String {
                calls += "list"
                return list
            }

            override fun renameDevice(mnemonic: String, deviceId: String, name: String): String {
                calls += "rename $deviceId [$name]"
                return actionAnswer
            }

            override fun revokeDevice(mnemonic: String, deviceId: String): String {
                calls += "revoke $deviceId"
                if (actionAnswer.contains("\"this_device\":true")) revoked = true
                return actionAnswer
            }

            override fun deviceRevoked() = revoked

            override fun forgetDeviceRevocation() {
                revoked = false
            }
        }

    private val wallet =
        object : WalletRepository {
            override val state: StateFlow<WalletState> =
                MutableStateFlow(
                    WalletState.Locked(
                        WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
                    )
                )

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

            override suspend fun readMnemonic(): Mnemonic = Mnemonic(PHRASE)

            override suspend fun erase() = error("unused")
        }

    private val devices = DeviceRepository(bridge, wallet, Dispatchers.Unconfined)

    private var logoutOutcome: WarrenLogoutOutcome = WarrenLogoutOutcome.LoggedOut

    private val revocation =
        DeviceRevocationUseCase(devices, wallet) {
            calls += "logout"
            logoutOutcome
        }

    private fun viewModel() = ManageDevicesViewModel(devices, revocation, today = { TODAY })

    private suspend fun ManageDevicesViewModel.content(): ManageDevicesUiState {
        var content: ManageDevicesUiState? = null
        uiState.test {
            content = (awaitItem() as? Lce.Content)?.value ?: (awaitItem() as Lce.Content).value
            cancelAndIgnoreRemainingEvents()
        }
        return content!!
    }

    @Test
    fun `this device comes first, then the others by last activity, then by name`() = runTest {
        bridge.list =
            listEnvelope(
                listOf(
                    row("old", "zebra", "2026-10-01"),
                    row("recent-b", "beta", "2026-10-28"),
                    row("me", "my phone", "2026-09-01", thisDevice = true),
                    row("recent-a", "alpha", "2026-10-28"),
                )
            )

        val ids = viewModel().content().devices.map { it.device.id }

        assertEquals(listOf("me", "recent-a", "recent-b", "old"), ids)
    }

    @Test
    fun `a name is shown without its bidirectional controls`() = runTest {
        bridge.list = listEnvelope(listOf(row("a", "\\u202Eevil\\u2066 name\\u2069", "2026-10-28")))

        val shown = viewModel().content().devices.single().device.displayedName

        assertEquals("evil name", shown)
    }

    @Test
    fun `the older-client row shows only when the registry names its day`() = runTest {
        bridge.list = listEnvelope(emptyList(), olderClientSeenOn = "\"2026-10-27\"")
        assertEquals(LocalDate.of(2026, 10, 27), viewModel().content().olderClientSeenOn)

        bridge.list = listEnvelope(emptyList(), olderClientSeenOn = "null")
        assertNull(viewModel().content().olderClientSeenOn)
    }

    @Test
    fun `activity is read against the UTC day the view model is given`() = runTest {
        assertEquals(TODAY, viewModel().content().today)
    }

    @ParameterizedTest
    @EnumSource(DeviceRegistryError::class, names = ["Revoked"], mode = EnumSource.Mode.EXCLUDE)
    fun `a list the registry refused shows its error with a retry`(error: DeviceRegistryError) =
        runTest {
            bridge.list = """{"ok":false,"error":"${code(error)}"}"""

            viewModel().uiState.test {
                assertEquals(Lce.Error(error), expectMostRecentItem())
            }
        }

    @Test
    fun `a list that says this device was revoked takes the revocation in at once`() = runTest {
        bridge.list = """{"ok":false,"error":"revoked"}"""
        bridge.revoked = true

        viewModel().uiState.test {
            assertEquals(Lce.Error(DeviceRegistryError.Revoked), expectMostRecentItem())
        }

        assertEquals(listOf("list", "logout"), calls)
        assertEquals(DeviceState.Revoked, devices.deviceState.value)
    }

    @Test
    fun `retrying a failed list reads it again`() = runTest {
        bridge.list = """{"ok":false,"error":"unavailable"}"""
        val viewModel = viewModel()
        viewModel.uiState.test {
            assertEquals(Lce.Error(DeviceRegistryError.Unavailable), expectMostRecentItem())
            bridge.list = listEnvelope(listOf(row("a", "alpha", "2026-10-28")))

            viewModel.fetchDevices()

            assertEquals("a", (expectMostRecentItem() as Lce.Content).value.devices.single().device.id)
        }
    }

    @Test
    fun `a disconnect the registry refused says so and reads the list again`() = runTest {
        val viewModel = viewModel()
        viewModel.uiSideEffect.test {
            bridge.actionAnswer = """{"ok":false,"error":"unavailable"}"""
            calls.clear()

            viewModel.removeDevice("other")

            assertEquals(ManageDevicesSideEffect.FailedToDisconnect, awaitItem())
            assertEquals(listOf("revoke other", "list"), calls)
        }
    }

    @Test
    fun `a device already gone is no failure`() = runTest {
        val viewModel = viewModel()
        viewModel.uiSideEffect.test {
            bridge.actionAnswer = """{"ok":false,"error":"not_found"}"""

            viewModel.removeDevice("gone")

            expectNoEvents()
        }
    }

    @Test
    fun `a rename sends the trimmed name and never one the registry would refuse`() = runTest {
        val viewModel = viewModel()

        viewModel.renameDevice("a", "   ")
        viewModel.renameDevice("a", "x".repeat(41))
        viewModel.renameDevice("a", "bell\u0007")
        viewModel.renameDevice("a", "  kitchen tablet ")

        assertEquals(listOf("rename a [kitchen tablet]"), calls.filter { it.startsWith("rename") })
    }

    @Test
    fun `a rename the registry refused says so`() = runTest {
        val viewModel = viewModel()
        viewModel.uiSideEffect.test {
            bridge.actionAnswer = """{"ok":false,"error":"invalid_name"}"""

            viewModel.renameDevice("a", "kitchen")

            assertEquals(ManageDevicesSideEffect.FailedToRename, awaitItem())
        }
    }

    @Test
    fun `logging this device out revokes it, then logs the wallet out, then leaves`() = runTest {
        val viewModel = viewModel()
        viewModel.uiSideEffect.test {
            bridge.actionAnswer = """{"ok":true,"this_device":true}"""
            calls.clear()

            viewModel.logOutThisDevice("me")

            assertEquals(ManageDevicesSideEffect.LoggedOut, awaitItem())
            assertEquals(listOf("revoke me", "logout"), calls)
            assertNull(devices.deviceState.value, "a logout is not a revocation to show")
        }
    }

    @Test
    fun `a logout the registry refused keeps the wallet and says so`() = runTest {
        val viewModel = viewModel()
        viewModel.uiSideEffect.test {
            bridge.actionAnswer = """{"ok":false,"error":"unavailable"}"""
            calls.clear()

            viewModel.logOutThisDevice("me")

            assertEquals(ManageDevicesSideEffect.FailedToLogOut, awaitItem())
            assertEquals(listOf("revoke me", "list"), calls)
        }
    }

    @Test
    fun `a logout whose tunnel did not come down in time stays on the list and says so`() =
        runTest {
            val viewModel = viewModel()
            viewModel.uiSideEffect.test {
                bridge.actionAnswer = """{"ok":true,"this_device":true}"""
                logoutOutcome = WarrenLogoutOutcome.TunnelStillUp

                viewModel.logOutThisDevice("me")

                assertEquals(ManageDevicesSideEffect.FailedToLogOut, awaitItem())
            }
        }

    private fun code(error: DeviceRegistryError) =
        when (error) {
            DeviceRegistryError.NoWallet -> "no_wallet"
            DeviceRegistryError.InvalidDevice -> "invalid_device"
            DeviceRegistryError.InvalidName -> "invalid_name"
            DeviceRegistryError.NotFound -> "not_found"
            DeviceRegistryError.Revoked -> "revoked"
            DeviceRegistryError.Unavailable -> "unavailable"
        }

    private companion object {
        val TODAY: LocalDate = LocalDate.of(2026, 10, 28)

        const val PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"

        fun row(id: String, name: String, lastSeenOn: String, thisDevice: Boolean = false) =
            """{"device_id":"$id","name":"$name","platform":"android","app_version":"1.1.40",""" +
                """"registered_on":"2026-09-01","last_seen_on":"$lastSeenOn",""" +
                """"this_device":$thisDevice}"""

        fun listEnvelope(rows: List<String>, olderClientSeenOn: String = "null") =
            """{"ok":true,"devices":[${rows.joinToString(",")}],""" +
                """"older_client_seen_on":$olderClientSeenOn}"""
    }
}
