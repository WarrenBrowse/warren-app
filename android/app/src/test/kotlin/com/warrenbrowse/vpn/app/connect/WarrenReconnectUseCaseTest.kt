package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class WarrenReconnectUseCaseTest {

    private val mockContext: Context = mockk(relaxed = true)

    @Test
    fun `reconnect dispatches a foreground-service intent with the reconnect action`() {
        every { mockContext.startForegroundService(any()) } returns null

        val useCase = WarrenReconnectUseCase(mockContext)
        useCase.reconnect()

        val captured = slot<Intent>()
        verify { mockContext.startForegroundService(capture(captured)) }
        assertEquals(KEY_RECONNECT_ACTION, captured.captured.action)
    }

    @Test
    fun `reconnect swallows dispatch failures`() {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        val useCase = WarrenReconnectUseCase(mockContext)
        useCase.reconnect()
    }
}
