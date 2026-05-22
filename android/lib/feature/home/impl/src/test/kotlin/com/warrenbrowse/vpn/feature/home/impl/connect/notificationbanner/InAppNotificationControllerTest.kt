package com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner

import app.cash.turbine.test
import io.mockk.MockKAnnotations
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.usecase.inappnotification.NewChangelogNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.TunnelStateNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.VersionNotificationUseCase
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExperimentalCoroutinesApi
@ExtendWith(TestCoroutineRule::class)
class InAppNotificationControllerTest {

    private lateinit var inAppNotificationController: InAppNotificationController
    private val newVersionChangelogNotifications =
        MutableStateFlow<InAppNotification.NewVersionChangelog?>(null)
    private val versionNotifications = MutableStateFlow<InAppNotification.UnsupportedVersion?>(null)
    private val tunnelStateNotifications = MutableStateFlow<InAppNotification?>(null)

    private lateinit var job: Job

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)

        val newVersionChangelogUseCase: NewChangelogNotificationUseCase = mockk()
        val versionNotificationUseCase: VersionNotificationUseCase = mockk()
        val tunnelStateNotificationUseCase: TunnelStateNotificationUseCase = mockk()
        every { newVersionChangelogUseCase.invoke() } returns newVersionChangelogNotifications
        every { versionNotificationUseCase.invoke() } returns versionNotifications
        every { versionNotificationUseCase.invoke() } returns versionNotifications
        every { tunnelStateNotificationUseCase.invoke() } returns tunnelStateNotifications
        job = Job()

        inAppNotificationController =
            InAppNotificationController(
                listOf(
                    newVersionChangelogUseCase,
                    versionNotificationUseCase,
                    tunnelStateNotificationUseCase,
                ),
                CoroutineScope(job + UnconfinedTestDispatcher()),
            )
    }

    @AfterEach
    fun teardown() {
        job.cancel()
        unmockkAll()
    }

    @Test
    fun `ensure all notifications have the right priority`() = runTest {
        val newVersionChangelog = InAppNotification.NewVersionChangelog
        newVersionChangelogNotifications.value = newVersionChangelog

        val errorState: ErrorState = mockk()
        every { errorState.cause } returns mockk()
        val tunnelStateBlocked = InAppNotification.TunnelStateBlocked
        tunnelStateNotifications.value = tunnelStateBlocked

        val unsupportedVersion = InAppNotification.UnsupportedVersion(mockk())
        versionNotifications.value = unsupportedVersion

        // D.4 step 41: NewDevice priority slot dropped (multi-device dead).
        // D.4 step 38: AccountExpiry priority slot dropped (subscription dead).

        inAppNotificationController.notifications.test {
            val notifications = awaitItem()

            assertEquals(
                listOf(
                    tunnelStateBlocked,
                    unsupportedVersion,
                    newVersionChangelog,
                ),
                notifications,
            )
        }
    }
}
