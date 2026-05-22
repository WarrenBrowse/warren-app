package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class WarrenDisconnectUseCaseTest {

    private val mockContext: Context = mockk(relaxed = true)

    @Test
    fun `disconnect dispatches a foreground-service intent with the disconnect action`() {
        every { mockContext.startForegroundService(any()) } returns null

        val useCase = WarrenDisconnectUseCase(mockContext)
        useCase.disconnect()

        val captured = slot<Intent>()
        verify { mockContext.startForegroundService(capture(captured)) }
        assertEquals(KEY_DISCONNECT_ACTION, captured.captured.action)
    }

    @Test
    fun `disconnect swallows dispatch failures (e g background-service restrictions)`() {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        val useCase = WarrenDisconnectUseCase(mockContext)
        // Should not throw - the use-case logs and continues so the caller
        // does not get a runtime crash on a transient OS restriction.
        useCase.disconnect()
    }
}
