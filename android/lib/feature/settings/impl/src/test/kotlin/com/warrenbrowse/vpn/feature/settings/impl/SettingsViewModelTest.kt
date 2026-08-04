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
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class SettingsViewModelTest {

    private val mockWalletRepository: WalletRepository = mockk()
    private val mockWarrenLocalSettings: WarrenLocalSettingsRepository = mockk()
    private val mockAppVersionInfoRepository: AppVersionInfoRepository = mockk()

    private val walletStateFlow = MutableStateFlow<WalletState>(WalletState.Absent)
    private val daitaFlow = MutableStateFlow(false)
    private val multiHopFlow = MutableStateFlow(true)
    private val natPmpFlow = MutableStateFlow(false)
    private val versionInfo =
        MutableStateFlow(VersionInfo(currentVersion = "", isSupported = false))

    private lateinit var viewModel: SettingsViewModel

    @BeforeEach
    fun setup() {
        every { mockWalletRepository.state } returns walletStateFlow
        every { mockWarrenLocalSettings.daitaEnabled } returns daitaFlow
        every { mockWarrenLocalSettings.multiHopEnabled } returns multiHopFlow
        every { mockWarrenLocalSettings.natPmpEnabled } returns natPmpFlow
        every { mockAppVersionInfoRepository.versionInfo } returns versionInfo

        viewModel =
            SettingsViewModel(
                walletRepository = mockWalletRepository,
                warrenLocalSettings = mockWarrenLocalSettings,
                appVersionInfoRepository = mockAppVersionInfoRepository,
                isPlayBuild = false,
            )
    }

    @AfterEach
    fun tearDown() {
        viewModel.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `wallet Absent maps to isLoggedIn false`() = runTest {
        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(false, item.value.isLoggedIn)
        }
    }

    @Test
    fun `wallet Ready maps to isLoggedIn true`() = runTest {
        walletStateFlow.value = WalletState.Ready(
            WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"),
        )

        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(true, item.value.isLoggedIn)
        }
    }

    @Test
    fun `version supported flag flows through`() = runTest {
        versionInfo.value = VersionInfo(currentVersion = "1.0", isSupported = true)

        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(true, item.value.isSupportedVersion)
        }
    }

    @Test
    fun `daita toggle flows from WarrenLocalSettings`() = runTest {
        daitaFlow.value = true

        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(true, item.value.isDaitaEnabled)
        }
    }

    @Test
    fun `multi-hop toggle flows from WarrenLocalSettings`() = runTest {
        multiHopFlow.value = false

        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(false, item.value.isMultiHopEnabled)
        }
    }

    @Test
    fun `port forwarding toggle flows from WarrenLocalSettings`() = runTest {
        natPmpFlow.value = true

        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<Lc.Content<SettingsUiState>>(item)
            assertEquals(true, item.value.isPortForwardingEnabled)
        }
    }
}
