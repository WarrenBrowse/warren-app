package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.repository.WarrenFailoverProvider
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ExitSwitchedNotificationUseCaseTest {

    private val failoverCount = MutableStateFlow(0)

    private val failoverProvider =
        object : WarrenFailoverProvider {
            override val failoverCount: StateFlow<Int>
                get() = this@ExitSwitchedNotificationUseCaseTest.failoverCount
        }

    private fun useCase() = ExitSwitchedNotificationUseCase(failoverProvider)

    @Test
    fun `no failover raises nothing`() = runTest {
        useCase()().test { assertNull(awaitItem()) }
    }

    @Test
    fun `a landed failover raises the banner until it is acknowledged`() = runTest {
        val useCase = useCase()
        useCase().test {
            assertNull(awaitItem())
            failoverCount.value = 1
            assertEquals(InAppNotification.ExitSwitched, awaitItem())
            useCase.acknowledge()
            assertNull(awaitItem())
        }
    }

    @Test
    fun `a further failover raises the banner again after an acknowledgement`() = runTest {
        val useCase = useCase()
        failoverCount.value = 1
        useCase.acknowledge()
        useCase().test {
            assertNull(awaitItem())
            failoverCount.value = 2
            assertEquals(InAppNotification.ExitSwitched, awaitItem())
        }
    }

    @Test
    fun `the switch ranks below an unsupported version and above the expiry alarm`() {
        // Desktop provider order: UnsupportedVersion, WarrenFailover, then the
        // account expiry family.
        val versionInfo = VersionInfo(currentVersion = "1.0", isSupported = false)
        assertTrue(
            InAppNotification.ExitSwitched.priority <
                InAppNotification.UnsupportedVersion(versionInfo).priority
        )
        assertTrue(
            InAppNotification.ExitSwitched.priority >
                InAppNotification.CloseToExpiry(daysLeft = 1).priority
        )
    }
}
