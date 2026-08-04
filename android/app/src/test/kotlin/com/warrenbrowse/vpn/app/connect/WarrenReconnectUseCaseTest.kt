package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
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

class WarrenReconnectUseCaseTest {

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

        WarrenReconnectUseCase(mockContext, connectionProxy).reconnect()

        verify { anyConstructed<Intent>().setAction(KEY_RECONNECT_ACTION) }
        verify { mockContext.startForegroundService(any()) }
    }

    @Test
    fun `reconnect swallows dispatch failures`() = runTest {
        every { mockContext.startForegroundService(any()) } throws
            IllegalStateException("not allowed to start service from background")

        WarrenReconnectUseCase(mockContext, connectionProxy).reconnect()

        // A re-dial that never left must not put the card into a transition.
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
