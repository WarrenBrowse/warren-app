package com.warrenbrowse.vpn.lib.pushnotification.tunnelstate

import android.content.Context
import app.cash.turbine.test
import arrow.core.left
import arrow.core.right
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import java.util.concurrent.Executors
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationChannelId
import com.warrenbrowse.vpn.lib.model.NotificationId
import com.warrenbrowse.vpn.lib.model.NotificationAction
import com.warrenbrowse.vpn.lib.model.NotificationTunnelState
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.Prepared
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.repository.UserPreferences
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class TunnelStateNotificationProviderTest {

    private lateinit var mockContext: Context
    private lateinit var connectionProxy: ConnectionProxy
    private lateinit var deviceRepository: DeviceRepository
    private lateinit var userPreferencesRepository: UserPreferencesRepository

    private lateinit var tunnelStateFlow: MutableStateFlow<TunnelState>
    private lateinit var deviceStateFlow: MutableStateFlow<DeviceState>
    private lateinit var preferencesFlow: MutableStateFlow<UserPreferences>

    private val testNotificationId = NotificationId(2)
    private val testChannelId = NotificationChannelId("test_channel")

    private lateinit var provider: TunnelStateNotificationProvider

    private val testAccountNumber = AccountNumber("1234567890123456")

    @OptIn(ExperimentalCoroutinesApi::class)
    @BeforeEach
    fun setup() {
        mockkStatic(VPN_SERVICE_UTILS)

        // Initialize mocks and default behaviors
        mockContext = mockk()
        connectionProxy = mockk()
        deviceRepository = mockk()
        userPreferencesRepository = mockk()
        val userPreferences = mockk<UserPreferences>()
        every { userPreferences.showLocationInSystemNotification } returns false

        // Setup controllable StateFlows
        tunnelStateFlow = MutableStateFlow(TunnelState.Disconnected(null))
        deviceStateFlow = MutableStateFlow(DeviceState.LoggedIn(testAccountNumber))
        preferencesFlow = MutableStateFlow(userPreferences)

        // Link flows to repository mocks
        every { connectionProxy.tunnelState } returns tunnelStateFlow
        every { deviceRepository.deviceState } returns deviceStateFlow
        every { userPreferencesRepository.preferencesFlow() } returns preferencesFlow

        // Default behavior for prepareSafe()
        every { mockContext.prepareVpnSafe() } returns Prepared.right()

        // Initialize the class under test
        provider =
            TunnelStateNotificationProvider(
                context = mockContext,
                connectionProxy = connectionProxy,
                deviceRepository = deviceRepository,
                preferences = userPreferencesRepository,
                channelId = testChannelId,
                scope = CoroutineScope(UnconfinedTestDispatcher()),
                computeDispatcher = UnconfinedTestDispatcher(),
            )
    }

    @Test
    @OptIn(ExperimentalCoroutinesApi::class)
    fun `the notification is computed off the thread that owns the scope`() {
        // The scope is the main one in the app, and each emission makes Binder calls into the
        // package manager (the VPN consent, the locked apps).
        val threads = mutableListOf<String>()
        every { mockContext.prepareVpnSafe() } answers
            {
                threads += Thread.currentThread().name.substringBefore(" @")
                Prepared.right()
            }
        val compute = Executors.newSingleThreadExecutor { Thread(it, "notification-compute") }
        try {
            val offMain =
                TunnelStateNotificationProvider(
                    context = mockContext,
                    connectionProxy = connectionProxy,
                    deviceRepository = deviceRepository,
                    preferences = userPreferencesRepository,
                    channelId = testChannelId,
                    scope = CoroutineScope(UnconfinedTestDispatcher()),
                    computeDispatcher = compute.asCoroutineDispatcher(),
                )
            runBlocking {
                withTimeout(5_000) {
                    offMain.notifications.first { it is NotificationUpdate.Notify }
                }
            }
        } finally {
            compute.shutdown()
        }

        assertTrue(threads.isNotEmpty())
        assertTrue(threads.all { it == "notification-compute" }, "$threads")
    }

    @Test
    fun `should emit cancel notification when device state is logged out and tunnel state is disconnected`() =
        runTest {
            provider.notifications.test {
                val initialItem = awaitItem()
                assertTrue(initialItem is NotificationUpdate.Notify)
                assertTrue(initialItem.value.state is NotificationTunnelState.Disconnected)

                // When device state changes to LoggedOut
                deviceStateFlow.value = DeviceState.LoggedOut

                // Then a Cancel notification update is emitted
                val cancelItem = awaitItem()
                assertTrue(cancelItem is NotificationUpdate.Cancel)
            }
        }

    @Test
    fun `should emit connected notification when tunnel state is connected`() = runTest {
        provider.notifications.test {
            awaitItem() // Skip initial emission

            // When tunnel state becomes Connected
            tunnelStateFlow.value = TunnelState.Connected(mockk(), mockk(), emptyList())

            // Then a Connected notification update is emitted
            val update = awaitItem()
            assertTrue(update is NotificationUpdate.Notify)
            assertTrue(update.value.state is NotificationTunnelState.Connected)
        }
    }

    @Test
    fun `a release after flapping is notified as traffic outside the VPN`() = runTest {
        provider.notifications.test {
            awaitItem() // Skip initial emission

            tunnelStateFlow.value =
                TunnelState.Error(
                    ErrorState(ErrorStateCause.WarrenTrafficReleased, isBlocking = false)
                )

            val update = awaitItem()
            assertTrue(update is NotificationUpdate.Notify)
            assertEquals(NotificationTunnelState.Error.TrafficReleased, update.value.state)
        }
    }

    @Test
    fun `a block on a network that reaches no entry is notified under its own cause`() =
        runTest {
            provider.notifications.test {
                awaitItem() // Skip initial emission

                tunnelStateFlow.value =
                    TunnelState.Error(
                        ErrorState(ErrorStateCause.WarrenNoDialableNetwork, isBlocking = true)
                    )

                val update = awaitItem()
                assertTrue(update is NotificationUpdate.Notify)
                assertEquals(NotificationTunnelState.Error.NoDialableNetwork, update.value.state)
                assertEquals(
                    listOf(NotificationAction.Tunnel.Reconnect, NotificationAction.Tunnel.Disconnect),
                    update.value.actions,
                )
            }
        }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun `should emit disconnected with prepare error when VPN is not prepared`() = runTest {
        // Given VPN is not prepared
        val prepareError = PrepareError.OtherAlwaysOnApp("OtherVPN")
        every { mockContext.prepareVpnSafe() } returns prepareError.left()

        provider =
            TunnelStateNotificationProvider(
                context = mockContext,
                connectionProxy = connectionProxy,
                deviceRepository = deviceRepository,
                preferences = userPreferencesRepository,
                channelId = testChannelId,
                scope = CoroutineScope(UnconfinedTestDispatcher()),
            )

        provider.notifications.test {
            val item = awaitItem() as NotificationUpdate.Notify<Notification.Tunnel>

            // Then a Disconnected notification update is emitted with the pepare error
            assertEquals(testNotificationId, item.notificationId)
            val expectedState = NotificationTunnelState.Disconnected(prepareError)
            assertEquals(expectedState, item.value.state)
        }
    }

    @Test
    fun `should re-emit current notification if the show location in notification preference is changed`() =
        runTest {
            val location =
                GeoIpLocation(
                    country = "USA",
                    latitude = 40.7128,
                    longitude = -74.0060,
                    ipv4 = null,
                    ipv6 = null,
                    city = null,
                    hostname = null,
                    entryHostname = null,
                )

            provider.notifications.test {
                awaitItem() // Skip initial

                // When tunnel state becomes Connected
                tunnelStateFlow.value =
                    TunnelState.Connected(mockk(), location = location, emptyList())

                // Then a Connected notification update is emitted
                val update = awaitItem()
                assertTrue(update is NotificationUpdate.Notify)
                assertTrue(update.value.state is NotificationTunnelState.Connected)
                // but the location is null because showLocationInSystemNotification is false
                assertEquals(
                    (update.value.state as NotificationTunnelState.Connected).location,
                    null,
                )

                // When show location in notification preference is changed to true
                preferencesFlow.value =
                    mockk<UserPreferences>().also {
                        every { it.showLocationInSystemNotification } returns true
                    }

                // Then the notification should be re-emitted
                val update2 = awaitItem()
                assertTrue(update2 is NotificationUpdate.Notify)
                assertTrue(update2.value.state is NotificationTunnelState.Connected)
                // And the location is now present in the notification
                assertEquals(
                    (update2.value.state as NotificationTunnelState.Connected).location,
                    location,
                )
            }
        }

    companion object {
        const val VPN_SERVICE_UTILS = "com.warrenbrowse.vpn.lib.common.util.VpnServiceUtilsKt"
    }
}
