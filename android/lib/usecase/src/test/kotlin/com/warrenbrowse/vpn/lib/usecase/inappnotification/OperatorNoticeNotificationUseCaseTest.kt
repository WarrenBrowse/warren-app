package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.WarrenNotice
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeRepository
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class OperatorNoticeNotificationUseCaseTest {

    private val state = WarrenNoticeRepository()
    private val dismissed = MutableStateFlow<List<String>>(emptyList())
    private val userPreferencesRepository: UserPreferencesRepository =
        mockk { every { dismissedNotices() } returns dismissed }
    private val useCase = OperatorNoticeNotificationUseCase(state, userPreferencesRepository)

    private fun notice(id: String, level: WarrenNoticeLevel = WarrenNoticeLevel.INFO) =
        WarrenNotice(id, "message of $id", level)

    @Test
    fun `no notice raises no banner`() = runTest { useCase().test { assertNull(awaitItem()) } }

    @Test
    fun `the banner carries the operator's own words`() = runTest {
        state.setNotices(listOf(notice("a1", WarrenNoticeLevel.ERROR)))

        useCase().test {
            assertEquals(
                InAppNotification.OperatorNotice(notice("a1", WarrenNoticeLevel.ERROR)),
                awaitItem(),
            )
        }
    }

    @Test
    fun `the single slot shows the first notice published, not the loudest`() = runTest {
        // Reordering here would make which message a user sees depend on a rule
        // nobody publishing one can see.
        state.setNotices(
            listOf(notice("first", WarrenNoticeLevel.INFO), notice("second", WarrenNoticeLevel.ERROR))
        )

        useCase().test { assertEquals("first", (awaitItem() as InAppNotification.OperatorNotice).notice.id) }
    }

    @Test
    fun `an erased notice clears the banner`() = runTest {
        state.setNotices(listOf(notice("a1")))

        useCase().test {
            assertEquals(InAppNotification.OperatorNotice(notice("a1")), awaitItem())
            state.setNotices(emptyList())
            assertNull(awaitItem())
        }
    }

    @Test
    fun `a notice the reader put away yields the slot to what it was hiding`() = runTest {
        val notice = notice("a1")
        state.setNotices(listOf(notice))

        useCase().test {
            assertEquals(InAppNotification.OperatorNotice(notice), awaitItem())
            dismissed.value = listOf(notice.dismissalKey)
            assertNull(awaitItem())
        }
    }

    @Test
    fun `putting the first notice away reveals the next one`() = runTest {
        val first = notice("first")
        state.setNotices(listOf(first, notice("second")))

        useCase().test {
            assertEquals("first", (awaitItem() as InAppNotification.OperatorNotice).notice.id)
            dismissed.value = listOf(first.dismissalKey)
            assertEquals("second", (awaitItem() as InAppNotification.OperatorNotice).notice.id)
        }
    }

    @Test
    fun `an operator who rewrites a notice in place raises it again`() = runTest {
        val read = notice("a1")
        dismissed.value = listOf(read.dismissalKey)
        state.setNotices(listOf(read.copy(message = "the beta runs one more week")))

        useCase().test {
            assertEquals("a1", (awaitItem() as InAppNotification.OperatorNotice).notice.id)
        }
    }

    @Test
    fun `a warning or an error keeps the slot whatever the reader put away`() = runTest {
        val alarm = notice("a1", WarrenNoticeLevel.ERROR)
        dismissed.value = listOf(alarm.dismissalKey)
        state.setNotices(listOf(alarm))

        useCase().test { assertEquals(InAppNotification.OperatorNotice(alarm), awaitItem()) }
    }

    @Test
    fun `severity drives the banner colour`() {
        assertEquals(
            StatusLevel.Error,
            InAppNotification.OperatorNotice(notice("a", WarrenNoticeLevel.ERROR)).statusLevel,
        )
        assertEquals(
            StatusLevel.Warning,
            InAppNotification.OperatorNotice(notice("a", WarrenNoticeLevel.WARNING)).statusLevel,
        )
        assertEquals(
            StatusLevel.Info,
            InAppNotification.OperatorNotice(notice("a", WarrenNoticeLevel.INFO)).statusLevel,
        )
    }

    @Test
    fun `a notice outranks every other banner, the stand-down included`() {
        // Desktop NotificationArea: the notice provider is first of all. When
        // the operator has something to say to everyone, that is the one thing
        // the user must see; the states it hides are still legible in the
        // connect card's own status.
        val notice = InAppNotification.OperatorNotice(notice("a1"))
        assertEquals(
            notice,
            listOf(
                    InAppNotification.EnvStandDown,
                    InAppNotification.HostOffline,
                    InAppNotification.TunnelStateBlocked,
                    notice,
                )
                .maxByOrNull { it.priority },
        )
    }

    @Test
    fun `an unknown severity reads as the calmest banner rather than dropping the message`() {
        assertEquals(WarrenNoticeLevel.INFO, WarrenNoticeLevel.of("catastrophe"))
        assertEquals(WarrenNoticeLevel.ERROR, WarrenNoticeLevel.of("error"))
        assertEquals(WarrenNoticeLevel.WARNING, WarrenNoticeLevel.of("warning"))
        assertEquals(WarrenNoticeLevel.INFO, WarrenNoticeLevel.of("info"))
    }
}
