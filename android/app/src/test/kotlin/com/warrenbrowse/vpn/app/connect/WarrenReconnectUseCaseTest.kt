package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkConstructor
import io.mockk.unmockkAll
import io.mockk.verify
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class WarrenReconnectUseCaseTest {

    private val mockContext: Context = mockk(relaxed = true)

    @BeforeEach
    fun setUp() {
        // See WarrenDisconnectUseCaseTest: mock the Intent constructor so the
        // `action` setter is observable in a plain JVM unit test (no android
        // framework / Robolectric).
        mockkConstructor(Intent::class)
        // `intent.action = x` compiles to `setAction(x)`, which returns Intent
        // (builder style); stub the method to return the mock itself so it does
        // not blow up with a Unit -> Intent ClassCastException.
        every { anyConstructed<Intent>().setAction(any()) } answers { self as Intent }
    }

    @AfterEach
    fun tearDown() = unmockkAll()

    @Test
    fun `reconnect dispatches a foreground-service intent with the reconnect action`() {
        every { mockContext.startForegroundService(any()) } returns null

        WarrenReconnectUseCase(mockContext).reconnect()

        verify { anyConstructed<Intent>().setAction(KEY_RECONNECT_ACTION) }
        verify { mockContext.startForegroundService(any()) }
    }

    @Test
    fun `reconnect swallows dispatch failures`() {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        WarrenReconnectUseCase(mockContext).reconnect()
    }
}
