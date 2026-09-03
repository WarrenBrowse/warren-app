package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenNotice
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementRepository
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class LaunchAnnouncementNotificationUseCaseTest {

    private val state = WarrenAnnouncementRepository()
    private val dismissed = MutableStateFlow<List<String>>(emptyList())
    private val userPreferencesRepository: UserPreferencesRepository =
        mockk { every { dismissedAnnouncements() } returns dismissed }
    private val useCase = LaunchAnnouncementNotificationUseCase(state, userPreferencesRepository)

    private fun announcement(
        id: String,
        level: WarrenNoticeLevel = WarrenNoticeLevel.INFO,
        campaign: String? = null,
        code: String? = null,
    ) =
        WarrenAnnouncement(
            id = id,
            headline = "headline of $id",
            body = "body of $id",
            level = level,
            voucherCampaignId = campaign,
            voucherCode = code,
        )

    @Test
    fun `no announcement raises no card`() = runTest { useCase().test { assertNull(awaitItem()) } }

    @Test
    fun `the card carries the operator's own words and this account's code`() = runTest {
        val launch = announcement("a1", campaign = "prod-launch", code = "ABCD1234EFGH5678")
        state.setAnnouncements(listOf(launch))

        useCase().test {
            assertEquals(InAppNotification.LaunchAnnouncement(launch), awaitItem())
        }
    }

    @Test
    fun `an announcement with no campaign renders no code`() = runTest {
        state.setAnnouncements(listOf(announcement("a1")))

        useCase().test {
            val card = awaitItem() as InAppNotification.LaunchAnnouncement
            assertNull(
                card.announcement.voucherCode,
                "a card with no offer must not show a voucher block at all",
            )
            assertNull(card.announcement.voucherCampaignId)
        }
    }

    @Test
    fun `the single slot shows the first announcement published, not the loudest`() = runTest {
        state.setAnnouncements(
            listOf(
                announcement("first", WarrenNoticeLevel.INFO),
                announcement("second", WarrenNoticeLevel.ERROR),
            )
        )

        useCase().test {
            assertEquals(
                "first",
                (awaitItem() as InAppNotification.LaunchAnnouncement).announcement.id,
            )
        }
    }

    @Test
    fun `a withdrawn announcement clears the card`() = runTest {
        state.setAnnouncements(listOf(announcement("a1")))

        useCase().test {
            assertEquals(InAppNotification.LaunchAnnouncement(announcement("a1")), awaitItem())
            state.setAnnouncements(emptyList())
            assertNull(awaitItem())
        }
    }

    @Test
    fun `a card the reader put away never comes back`() = runTest {
        // The code it carried is already in the reader's hands, so raising it
        // again on every launch would be nagging about something dealt with.
        state.setAnnouncements(listOf(announcement("a1")))

        useCase().test {
            assertEquals(InAppNotification.LaunchAnnouncement(announcement("a1")), awaitItem())
            dismissed.value = listOf("a1")
            assertNull(awaitItem())
        }
    }

    @Test
    fun `putting the first card away reveals the next one`() = runTest {
        state.setAnnouncements(listOf(announcement("first"), announcement("second")))

        useCase().test {
            assertEquals(
                "first",
                (awaitItem() as InAppNotification.LaunchAnnouncement).announcement.id,
            )
            dismissed.value = listOf("first")
            assertEquals(
                "second",
                (awaitItem() as InAppNotification.LaunchAnnouncement).announcement.id,
            )
        }
    }

    @Test
    fun `severity drives the card colour`() {
        assertEquals(
            StatusLevel.Error,
            InAppNotification.LaunchAnnouncement(announcement("a", WarrenNoticeLevel.ERROR))
                .statusLevel,
        )
        assertEquals(
            StatusLevel.Warning,
            InAppNotification.LaunchAnnouncement(announcement("a", WarrenNoticeLevel.WARNING))
                .statusLevel,
        )
        assertEquals(
            StatusLevel.Info,
            InAppNotification.LaunchAnnouncement(announcement("a", WarrenNoticeLevel.INFO))
                .statusLevel,
        )
    }

    @Test
    fun `the card outranks the operator notice it can be buried under`() {
        // Desktop NotificationArea: the announcement provider is first of all.
        // A warning or an error notice holds the slot for as long as it stands,
        // and the code the card carries stops being worth anything once the
        // campaign closes; the card steps aside the moment it is put away.
        val card = InAppNotification.LaunchAnnouncement(announcement("a1"))
        val notice =
            InAppNotification.OperatorNotice(
                WarrenNotice("n1", "exit outage in NL", WarrenNoticeLevel.ERROR)
            )
        assertEquals(
            card,
            listOf(
                    notice,
                    InAppNotification.EnvStandDown,
                    InAppNotification.HostOffline,
                    InAppNotification.TunnelStateBlocked,
                    card,
                )
                .maxByOrNull { it.priority },
        )
    }
}
