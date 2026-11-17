package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class AlwaysOnVpnAdviceNotificationUseCaseTest {
    private val show = MutableStateFlow(false)
    private val useCase = AlwaysOnVpnAdviceNotificationUseCase { show }

    @Test
    fun `the advice shows while it is due and leaves once it is not`() = runTest {
        useCase().test {
            assertNull(awaitItem())
            show.value = true
            assertEquals(InAppNotification.AlwaysOnVpnAdvice, awaitItem())
            show.value = false
            assertNull(awaitItem())
        }
    }
}
