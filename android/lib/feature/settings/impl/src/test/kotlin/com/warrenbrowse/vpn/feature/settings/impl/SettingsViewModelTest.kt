package com.warrenbrowse.vpn.feature.settings.impl

import androidx.lifecycle.viewModelScope
import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.model.WireguardConstraints
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class SettingsViewModelTest {

    private val mockDeviceRepository: DeviceRepository = mockk()
    private val mockAppVersionInfoRepository: AppVersionInfoRepository = mockk()
    private val mockWireguardConstraintsRepository: WireguardConstraintsRepository = mockk()
    private val mockSettingsRepository: SettingsRepository = mockk()

    private val versionInfo =
        MutableStateFlow(VersionInfo(currentVersion = "", isSupported = false))
    private val wireguardConstraints = MutableStateFlow<WireguardConstraints>(mockk(relaxed = true))
    private val settings = MutableStateFlow(mockk<Settings>(relaxed = true))

    private lateinit var viewModel: SettingsViewModel

    @BeforeEach
    fun setup() {
        val deviceState = MutableStateFlow<DeviceState>(DeviceState.LoggedOut)

        every { mockDeviceRepository.deviceState } returns deviceState
        every { mockAppVersionInfoRepository.versionInfo } returns versionInfo
        every { mockWireguardConstraintsRepository.wireguardConstraints } returns
            wireguardConstraints
        every { mockSettingsRepository.settingsUpdates } returns settings

        viewModel =
            SettingsViewModel(
                deviceRepository = mockDeviceRepository,
                appVersionInfoRepository = mockAppVersionInfoRepository,
                wireguardConstraintsRepository = mockWireguardConstraintsRepository,
                settingsRepository = mockSettingsRepository,
                isPlayBuild = false,
            )
    }

    @AfterEach
    fun tearDown() {
        viewModel.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `uiState should return isLoggedIn false by default`() = runTest {
        // Act, Assert
        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(false, item.value.isLoggedIn)
        }
    }

    @Test
    fun `when AppVersionInfoRepository returns isSupported true uiState should return isSupportedVersion true`() =
        runTest {
            // Arrange
            val versionInfoTestItem = VersionInfo(currentVersion = "", isSupported = true)
            versionInfo.value = versionInfoTestItem

            // Act, Assert
            viewModel.uiState.test {
                val result = awaitItem()
                assertIs<Lc.Content<SettingsUiState>>(result)
                assertEquals(true, result.value.isSupportedVersion)
            }
        }

    @Test
    fun `when AppVersionInfoRepository returns isSupported false uiState should return isSupportedVersion false`() =
        runTest {
            // Arrange
            val versionInfoTestItem = VersionInfo(currentVersion = "", isSupported = false)
            versionInfo.value = versionInfoTestItem

            // Act, Assert
            viewModel.uiState.test {
                val result = awaitItem()
                assertIs<Lc.Content<SettingsUiState>>(result)
                assertEquals(false, result.value.isSupportedVersion)
            }
        }

    @Test
    fun `when WireguardConstraintsRepository return multihop enabled uiState should return multihop enabled true`() =
        runTest {
            // Arrange
            wireguardConstraints.value =
                WireguardConstraints(
                    isMultihopEnabled = true,
                    entryLocation = Constraint.Any,
                    ipVersion = Constraint.Any,
                )

            // Act, Assert
            viewModel.uiState.test {
                val result = awaitItem()
                assertIs<Lc.Content<SettingsUiState>>(result)
                assertEquals(true, result.value.multihopEnabled)
            }
        }
}
