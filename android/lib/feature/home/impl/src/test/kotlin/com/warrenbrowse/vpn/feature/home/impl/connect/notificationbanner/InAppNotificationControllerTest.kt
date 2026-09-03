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
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.usecase.inappnotification.EnvStandDownUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.LaunchAnnouncementNotificationUseCase
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
    private val envStandDownNotifications = MutableStateFlow<InAppNotification?>(null)
    private val announcementNotifications = MutableStateFlow<InAppNotification?>(null)

    private lateinit var job: Job

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)

        val newVersionChangelogUseCase: NewChangelogNotificationUseCase = mockk()
        val versionNotificationUseCase: VersionNotificationUseCase = mockk()
        val tunnelStateNotificationUseCase: TunnelStateNotificationUseCase = mockk()
        val envStandDownUseCase: EnvStandDownUseCase = mockk()
        val announcementUseCase: LaunchAnnouncementNotificationUseCase = mockk()
        every { newVersionChangelogUseCase.invoke() } returns newVersionChangelogNotifications
        every { versionNotificationUseCase.invoke() } returns versionNotifications
        every { versionNotificationUseCase.invoke() } returns versionNotifications
        every { tunnelStateNotificationUseCase.invoke() } returns tunnelStateNotifications
        every { envStandDownUseCase.invoke() } returns envStandDownNotifications
        every { announcementUseCase.invoke() } returns announcementNotifications
        job = Job()

        inAppNotificationController =
            InAppNotificationController(
                listOf(
                    newVersionChangelogUseCase,
                    versionNotificationUseCase,
                    tunnelStateNotificationUseCase,
                    envStandDownUseCase,
                    announcementUseCase,
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

        // The stand-down says this build will not connect at all, so it takes
        // the head of the ladder: any banner about the connection would be
        // describing a state this build is no longer trying to reach.
        val envStandDown = InAppNotification.EnvStandDown
        envStandDownNotifications.value = envStandDown

        // The launch announcement is first of all (desktop NotificationArea):
        // it steps aside the moment the reader puts it away, while a notice or
        // a stand-down holds the slot for as long as the condition stands, and
        // the code it carries stops being worth anything once the campaign
        // closes.
        val announcement =
            InAppNotification.LaunchAnnouncement(
                WarrenAnnouncement(
                    id = "a1",
                    headline = "Production is open",
                    body = "Your beta account gets a free month.",
                    level = WarrenNoticeLevel.WARNING,
                )
            )
        announcementNotifications.value = announcement

        inAppNotificationController.notifications.test {
            val notifications = awaitItem()

            assertEquals(
                listOf(
                    announcement,
                    envStandDown,
                    tunnelStateBlocked,
                    unsupportedVersion,
                    newVersionChangelog,
                ),
                notifications,
            )
        }
    }
}
