package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkConstructor
import io.mockk.unmockkAll
import io.mockk.verify
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class WarrenDisconnectUseCaseTest {

    private val mockContext: Context = mockk(relaxed = true)

    @BeforeEach
    fun setUp() {
        // The use-case builds a real android.content.Intent, whose methods are
        // not available in a plain JVM unit test (the android.jar stub throws
        // "not mocked"). Mock the constructor so the Intent's `action` setter
        // is observable without Robolectric.
        mockkConstructor(Intent::class)
        // `intent.action = x` compiles to `setAction(x)`, which returns Intent
        // (builder style). Stub the method to return the mock itself; a
        // property-setter stub (`just Runs`) would return Unit and blow up with
        // a ClassCastException inside setAction.
        every { anyConstructed<Intent>().setAction(any()) } answers { self as Intent }
    }

    @AfterEach
    fun tearDown() = unmockkAll()

    @Test
    fun `disconnect dispatches a foreground-service intent with the disconnect action`() {
        every { mockContext.startForegroundService(any()) } returns null

        WarrenDisconnectUseCase(mockContext).disconnect()

        verify { anyConstructed<Intent>().setAction(KEY_DISCONNECT_ACTION) }
        verify { mockContext.startForegroundService(any()) }
    }

    @Test
    fun `disconnect swallows dispatch failures (e g background-service restrictions)`() {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        // Should not throw - the use-case logs and continues so the caller
        // does not get a runtime crash on a transient OS restriction.
        WarrenDisconnectUseCase(mockContext).disconnect()
    }
}
