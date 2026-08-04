package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

private const val DAY_SECS = 86_400L
private const val NOW = 1_800_000_000L

@ExtendWith(TestCoroutineRule::class)
class AccountExpiryNotificationUseCaseTest {

    private val expiry = MutableStateFlow(0L)
    private val localSettings: WarrenLocalSettingsRepository = mockk()

    private fun useCase(): AccountExpiryNotificationUseCase {
        every { localSettings.cachedSubscriptionExpiry } returns expiry
        return AccountExpiryNotificationUseCase(localSettings, now = { NOW })
    }

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    @Test
    fun `an unknown expiry raises nothing`() = runTest {
        expiry.value = 0L

        useCase()().test { assertNull(awaitItem()) }
    }

    @Test
    fun `an expiry beyond the warning window raises nothing`() = runTest {
        expiry.value = NOW + 10 * DAY_SECS

        useCase()().test { assertNull(awaitItem()) }
    }

    @Test
    fun `an expiry inside the warning window raises a yellow warning with the days left`() =
        runTest {
            expiry.value = NOW + 2 * DAY_SECS

            useCase()().test {
                val notification = awaitItem()
                assertEquals(InAppNotification.CloseToExpiry(daysLeft = 2), notification)
                assertEquals(StatusLevel.Warning, notification?.statusLevel)
            }
        }

    @Test
    fun `a partial day is rounded up so the last hours still read as one day`() = runTest {
        expiry.value = NOW + DAY_SECS / 2

        useCase()().test { assertEquals(InAppNotification.CloseToExpiry(daysLeft = 1), awaitItem()) }
    }

    @Test
    fun `an expiry already past raises the error tier with no days left`() = runTest {
        expiry.value = NOW - DAY_SECS

        useCase()().test {
            val notification = awaitItem()
            assertEquals(InAppNotification.CloseToExpiry(daysLeft = 0), notification)
            assertEquals(StatusLevel.Error, notification?.statusLevel)
        }
    }

    @Test
    fun `the banner ranks below the unsupported version banner`() {
        val versionInfo = VersionInfo(currentVersion = "1.0", isSupported = false)
        assertEquals(
            true,
            InAppNotification.CloseToExpiry(daysLeft = 1).priority <
                InAppNotification.UnsupportedVersion(versionInfo).priority,
        )
    }

    @Test
    fun `crossing into the window raises the banner without a restart`() = runTest {
        expiry.value = NOW + 10 * DAY_SECS

        useCase()().test {
            assertNull(awaitItem())
            expiry.value = NOW + DAY_SECS
            assertEquals(InAppNotification.CloseToExpiry(daysLeft = 1), awaitItem())
        }
    }
}
