package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class HostOfflineNotificationUseCaseTest {

    private val hostOffline = MutableStateFlow(false)
    private val useCase =
        HostOfflineNotificationUseCase(
            object : WarrenHostOfflineProvider {
                override val hostOffline: StateFlow<Boolean>
                    get() = this@HostOfflineNotificationUseCaseTest.hostOffline
            }
        )

    @Test
    fun `online device produces no notification`() = runTest {
        useCase().test { assertNull(awaitItem()) }
    }

    @Test
    fun `offline edge raises the banner and the online edge drops it`() = runTest {
        useCase().test {
            assertNull(awaitItem())
            hostOffline.value = true
            assertEquals(InAppNotification.HostOffline, awaitItem())
            hostOffline.value = false
            assertNull(awaitItem())
        }
    }
}
