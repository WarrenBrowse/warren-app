package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import io.mockk.MockKAnnotations
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class UpdateAvailableNotificationUseCaseTest {

    private val mockAppVersionInfoRepository: AppVersionInfoRepository = mockk()
    private val mockUserPreferencesRepository: UserPreferencesRepository = mockk()

    private val versionInfo = MutableStateFlow(VersionInfo(currentVersion = "1.0", isSupported = true))
    private val dismissedUpgradeVersion = MutableStateFlow("")

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
        every { mockAppVersionInfoRepository.versionInfo } returns versionInfo
        every { mockUserPreferencesRepository.dismissedUpgradeVersion() } returns
            dismissedUpgradeVersion
    }

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    private fun useCase(isInstalledFromStore: Boolean) =
        UpdateAvailableNotificationUseCase(
            appVersionInfoRepository = mockAppVersionInfoRepository,
            userPreferencesRepository = mockUserPreferencesRepository,
            isVersionInfoNotificationEnabled = true,
            isInstalledFromStore = { isInstalledFromStore },
        )

    @Test
    fun `sideload with available upgrade should emit UpdateAvailable`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")

        useCase(isInstalledFromStore = false)().test {
            assertEquals(InAppNotification.UpdateAvailable("1.3.0"), awaitItem())
        }
    }

    @Test
    fun `store install with available upgrade should emit nothing`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")

        useCase(isInstalledFromStore = true)().test { assertNull(awaitItem()) }
    }

    @Test
    fun `no available upgrade should emit nothing even on sideload`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = null)

        useCase(isInstalledFromStore = false)().test { assertNull(awaitItem()) }
    }

    @Test
    fun `disabled flag should emit nothing`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")

        UpdateAvailableNotificationUseCase(
                appVersionInfoRepository = mockAppVersionInfoRepository,
                userPreferencesRepository = mockUserPreferencesRepository,
                isVersionInfoNotificationEnabled = false,
                isInstalledFromStore = { false },
            )()
            .test { assertNull(awaitItem()) }
    }

    @Test
    fun `an upgrade the user dismissed stays dismissed`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")
        dismissedUpgradeVersion.value = "1.3.0"

        useCase(isInstalledFromStore = false)().test { assertNull(awaitItem()) }
    }

    @Test
    fun `dismissing one upgrade does not suppress the next one`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")
        dismissedUpgradeVersion.value = "1.3.0"

        useCase(isInstalledFromStore = false)().test {
            assertNull(awaitItem())
            versionInfo.value =
                VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.4.0")
            assertEquals(InAppNotification.UpdateAvailable("1.4.0"), awaitItem())
        }
    }

    @Test
    fun `dismissing the live upgrade drops the banner`() = runTest {
        versionInfo.value =
            VersionInfo(currentVersion = "1.0", isSupported = true, availableUpgrade = "1.3.0")

        useCase(isInstalledFromStore = false)().test {
            assertEquals(InAppNotification.UpdateAvailable("1.3.0"), awaitItem())
            dismissedUpgradeVersion.value = "1.3.0"
            assertNull(awaitItem())
        }
    }

    @Test
    fun `the changelog banner outranks the update prompt`() {
        assertEquals(
            true,
            InAppNotification.NewVersionChangelog.priority >
                InAppNotification.UpdateAvailable("1.3.0").priority,
        )
    }

    @Test
    fun `the update prompt is a warning, not an info banner`() {
        assertEquals(StatusLevel.Warning, InAppNotification.UpdateAvailable("1.3.0").statusLevel)
        assertEquals(StatusLevel.Info, InAppNotification.NewVersionChangelog.statusLevel)
    }
}
