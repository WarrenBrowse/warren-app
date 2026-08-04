package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkConstructor
import io.mockk.unmockkAll
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class WarrenDisconnectUseCaseTest {

    private val mockContext: Context = mockk(relaxed = true)
    private val engineState = MutableStateFlow<WarrenConnectedInfo>(
        WarrenConnectedInfo.Connected(
            exitEndpointHost = "185.65.135.10:443",
            entryEndpointHost = null,
            multiHop = false,
            daita = false,
            assignedNatPmpPort = null,
        ),
    )
    private val connectionProxy = ConnectionProxy(FakeTunnelStateProvider(engineState))

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

        WarrenDisconnectUseCase(mockContext, connectionProxy).disconnect()

        verify { anyConstructed<Intent>().setAction(KEY_DISCONNECT_ACTION) }
        verify { mockContext.startForegroundService(any()) }
    }

    @Test
    fun `the card shows the teardown before the service has answered`() = runTest {
        every { mockContext.startForegroundService(any()) } returns null

        WarrenDisconnectUseCase(mockContext, connectionProxy).disconnect()

        val state = connectionProxy.tunnelState.first()
        assertTrue(state is TunnelState.Disconnecting, "got: $state")
    }

    @Test
    fun `disconnect swallows dispatch failures (e g background-service restrictions)`() = runTest {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        // Should not throw - the use-case logs and continues so the caller
        // does not get a runtime crash on a transient OS restriction.
        WarrenDisconnectUseCase(mockContext, connectionProxy).disconnect()

        // ...and a teardown that never left must not be presented as one.
        val state = connectionProxy.tunnelState.first()
        assertTrue(state is TunnelState.Connected, "got: $state")
    }

    private class FakeTunnelStateProvider(
        private val infoFlow: MutableStateFlow<WarrenConnectedInfo>,
    ) : WarrenTunnelStateProvider {
        override val state: StateFlow<String> = MutableStateFlow("").asStateFlow()
        override val connectedInfo: StateFlow<WarrenConnectedInfo> = infoFlow.asStateFlow()
    }
}
