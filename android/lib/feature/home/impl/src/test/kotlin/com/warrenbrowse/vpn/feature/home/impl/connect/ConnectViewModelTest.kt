package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.lifecycle.viewModelScope
import app.cash.turbine.test
import arrow.core.right
import io.mockk.Runs
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.just
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.InAppNotificationController
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenPathHealthProvider
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ConnectViewModelTest {
    private lateinit var viewModel: ConnectViewModel

    private val device = MutableStateFlow<DeviceState?>(null)
    private val notifications = MutableStateFlow<List<InAppNotification>>(emptyList())

    // Service connections
    private val mockConnectionProxy: ConnectionProxy = mockk()
    private val mockLocation: GeoIpLocation = mockk(relaxed = true)

    // Device Repository
    private val mockDeviceRepository: DeviceRepository = mockk()

    // Changelog Repository
    private val mockChangelogRepository: ChangelogRepository = mockk()

    // In App Notifications
    private val mockInAppNotificationController: InAppNotificationController = mockk()

    // Select location use case
    private val mockSelectedLocationTitleUseCase: SelectedLocationTitleUseCase = mockk()

    // Flows
    private val tunnelState = MutableStateFlow<TunnelState>(TunnelState.Disconnected())
    private val selectedRelayItemFlow = MutableStateFlow<String?>(null)
    private val lastKnownLocationFlow = MutableStateFlow<GeoIpLocation?>(null)
    private val exitPinFlow = MutableStateFlow<ExitPin>(ExitPin.Automatic)

    // Warren relay catalogue + local settings (map-marker location source)
    private val mockRelayProvider: WarrenRelayProvider = mockk(relaxed = true)
    private val pathWedgedFlow = MutableStateFlow(false)
    private val mockPathHealthProvider: WarrenPathHealthProvider = mockk()
    private val mockWarrenLocalSettings: WarrenLocalSettingsRepository = mockk()

    // Host-offline + auto-recovery surfaces
    private val hostOfflineFlow = MutableStateFlow(false)
    private val autoRecoveryCountFlow = MutableStateFlow(0)
    private val mockHostOfflineProvider: WarrenHostOfflineProvider = mockk()
    private val mockAutoRecoveryProvider: WarrenAutoRecoveryProvider = mockk()

    // Last known location
    private val mockLastKnownLocationUseCase: LastKnownLocationUseCase = mockk()

    // System VPN Settings
    private val mockSystemVpnSettingsUseCase: SystemVpnSettingsAvailableUseCase = mockk()

    @BeforeEach
    fun setup() {
        every { mockDeviceRepository.deviceState } returns device

        coEvery { mockDeviceRepository.updateDevice() } just Runs

        every { mockInAppNotificationController.notifications } returns notifications

        every { mockConnectionProxy.tunnelState } returns tunnelState

        every { mockLastKnownLocationUseCase.lastKnownDisconnectedLocation } returns
            lastKnownLocationFlow

        every { mockWarrenLocalSettings.exitPin } returns exitPinFlow
        every { mockHostOfflineProvider.hostOffline } returns hostOfflineFlow
        every { mockAutoRecoveryProvider.autoRecoveryCount } returns autoRecoveryCountFlow
        every { mockRelayProvider.list() } returns emptyList()
        every { mockRelayProvider.catalogue } returns MutableStateFlow(emptyList())
        every { mockPathHealthProvider.pathWedged } returns pathWedgedFlow

        every { mockLocation.country } returns "dummy country"

        // Flows
        every { mockSelectedLocationTitleUseCase() } returns selectedRelayItemFlow

        viewModel =
            ConnectViewModel(
                deviceRepository = mockDeviceRepository,
                changelogRepository = mockChangelogRepository,
                inAppNotificationController = mockInAppNotificationController,
                userPreferencesRepository = mockk(),
                selectedLocationTitleUseCase = mockSelectedLocationTitleUseCase,
                connectionProxy = mockConnectionProxy,
                lastKnownLocationUseCase = mockLastKnownLocationUseCase,
                systemVpnSettingsUseCase = mockSystemVpnSettingsUseCase,
                warrenDisconnect = mockk(relaxed = true),
                isPlayBuild = false,
                resolveAppListing = mockk(),
                relayProvider = mockRelayProvider,
                pathHealthProvider = mockPathHealthProvider,
                localSettings = mockWarrenLocalSettings,
                hostOfflineProvider = mockHostOfflineProvider,
                autoRecoveryProvider = mockAutoRecoveryProvider,
                exitSwitchedNotificationUseCase = mockk(relaxed = true),
                envStandDownUseCase = mockk(relaxed = true),
            )
    }

    @AfterEach
    fun teardown() {
        viewModel.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `uiState should emit initial state by default`() = runTest {
        viewModel.uiState.test { assertEquals(ConnectUiState.INITIAL, awaitItem()) }
    }

    @Test
    fun `given change in tunnel state uiState should emit new tunnel state`() = runTest {
        val tunnelStateTestItem = TunnelState.Connected(mockk(relaxed = true), null, emptyList())

        viewModel.uiState.test {
            assertEquals(ConnectUiState.INITIAL, awaitItem())
            tunnelState.emit(tunnelStateTestItem)
            val result = awaitItem()
            assertEquals(tunnelStateTestItem, result.tunnelState)
        }
    }

    @Test
    fun `given change in tunnelState uiState should emit new tunnelState`() = runTest {
        // Arrange
        val tunnelEndpoint: TunnelEndpoint = mockk()
        val location: GeoIpLocation = mockk()
        val tunnelStateTestItem = TunnelState.Connected(tunnelEndpoint, location, emptyList())
        every { location.ipv4?.hostAddress } returns "1.1.1.1"
        every { location.hostname } returns "hostname"

        // Act, Assert
        viewModel.uiState.test {
            assertEquals(ConnectUiState.INITIAL, awaitItem())
            tunnelState.emit(tunnelStateTestItem)
            val result = awaitItem()
            assertEquals(tunnelStateTestItem, result.tunnelState)
        }
    }

    @Test
    fun `given RelayListUseCase returns new selectedRelayItem uiState should emit new selectedRelayItem`() =
        runTest {
            val selectedRelayItemTitle = "Item"
            viewModel.uiState.test {
                assertEquals(ConnectUiState.INITIAL, awaitItem())

                selectedRelayItemFlow.value = selectedRelayItemTitle
                assertEquals(selectedRelayItemTitle, awaitItem().selectedRelayItemTitle)
            }
        }

    @Test
    fun `given new location in tunnel state uiState should emit new location`() = runTest {
        val locationTestItem =
            GeoIpLocation(
                ipv4 = mockk(relaxed = true),
                ipv6 = mockk(relaxed = true),
                country = "Sweden",
                city = "Gothenburg",
                hostname = "Host",
                entryHostname = "EntryHost",
                latitude = 57.7065,
                longitude = 11.967,
            )

        // Act, Assert
        viewModel.uiState.test {
            tunnelState.emit(TunnelState.Disconnected(null))

            // Start of with no location
            assertNull(awaitItem().location)

            // After updated we show latest
            tunnelState.emit(TunnelState.Disconnected(locationTestItem))
            assertEquals(locationTestItem, awaitItem().location)
        }
    }

    @Test
    fun `initial state should not include any location`() =
        // Arrange
        runTest {
            val locationTestItem = null

            // Act, Assert
            viewModel.uiState.test { assertEquals(locationTestItem, awaitItem().location) }
        }

    @Test
    fun `given InAppNotificationController returns TunnelStateError notification uiState should emit notification`() =
        runTest {
            // Arrange
            val mockErrorState: ErrorState = mockk()
            every { mockErrorState.cause } returns mockk()
            val expectedConnectNotificationState =
                InAppNotification.TunnelStateError(mockErrorState)

            // Act, Assert
            viewModel.uiState.test {
                assertEquals(ConnectUiState.INITIAL, awaitItem())
                notifications.value = listOf(expectedConnectNotificationState)
                assertEquals(expectedConnectNotificationState, awaitItem().inAppNotification)
            }
        }

    @Test
    fun `given tunnel state error should emit last known disconnected location as location`() =
        runTest {
            // Arrange
            val tunnel = TunnelState.Error(mockk(relaxed = true))
            val lastKnownLocation: GeoIpLocation = mockk(relaxed = true)

            // Act, Assert
            viewModel.uiState.test {
                assertEquals(ConnectUiState.INITIAL, awaitItem())
                lastKnownLocationFlow.emit(lastKnownLocation)
                tunnelState.emit(tunnel)
                awaitItem()
                val result = awaitItem()
                assertEquals(lastKnownLocation, result.location)
            }
        }

    @Test
    fun `given no vpn system setting available should return the correct permission denied`() =
        runTest {
            // Arrange
            val expectedSideEffect =
                ConnectViewModel.UiSideEffect.ConnectError.PermissionDenied(false)
            every { mockSystemVpnSettingsUseCase.invoke() } returns false

            // Act
            viewModel.createVpnProfileResult(hasVpnPermission = false)

            // Assert
            viewModel.uiSideEffect.test { assertEquals(expectedSideEffect, awaitItem()) }
        }

    @Test
    fun `given vpn system setting available should return the correct permission denied`() =
        runTest {
            // Arrange
            val expectedSideEffect =
                ConnectViewModel.UiSideEffect.ConnectError.PermissionDenied(true)
            every { mockSystemVpnSettingsUseCase.invoke() } returns true

            // Act
            viewModel.createVpnProfileResult(hasVpnPermission = false)

            // Assert
            viewModel.uiSideEffect.test { assertEquals(expectedSideEffect, awaitItem()) }
        }

    @Test
    fun `ensure a wedged datapath stops the card claiming protection`() = runTest {
        // The tunnel stays Connected while nothing crosses it. hostOffline is
        // what ConnectionStatusText keys on to swap "Connection established"
        // for "Connection interrupted", so a wedge must raise it.
        val connected = TunnelState.Connected(mockk(relaxed = true), null, emptyList())
        viewModel.uiState.test {
            assertEquals(ConnectUiState.INITIAL, awaitItem())
            tunnelState.emit(connected)
            assertEquals(false, awaitItem().hostOffline)

            pathWedgedFlow.emit(true)
            val wedged = awaitItem()
            assertIs<TunnelState.Connected>(wedged.tunnelState)
            assertEquals(true, wedged.hostOffline, "a wedged datapath must not read as protected")

            pathWedgedFlow.emit(false)
            assertEquals(false, awaitItem().hostOffline)
        }
    }
}
