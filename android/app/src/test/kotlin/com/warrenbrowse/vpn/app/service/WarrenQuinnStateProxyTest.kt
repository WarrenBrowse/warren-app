package com.warrenbrowse.vpn.app.service

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenQuinnStateProxyTest {

    // The proxy binds its state projection to Dispatchers.Main.immediate at
    // construction, so the Main dispatcher must be installed before any test
    // builds a proxy. UnconfinedTestDispatcher runs the eager stateIn
    // collection synchronously, so `.value` reflects updates immediately.
    @BeforeEach
    fun setUp() {
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @AfterEach
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `default state is Disconnected`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        assertEquals(WarrenTunnelState.Disconnected, proxy.tunnelState.value)
        assertEquals("Disconnected", proxy.state.value)
    }

    @Test
    fun `update flows through to both state surfaces`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(WarrenTunnelState.Connecting)
        assertEquals(WarrenTunnelState.Connecting, proxy.tunnelState.value)
        assertEquals("Connecting...", proxy.state.value)
    }

    @Test
    fun `Connected always advertises always-on HTTP3 mimicry`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(
            WarrenTunnelState.Connected(
                exitId = "abc",
                assignedNatPmpPort = null,
                multiHop = false,
                daita = false,
            )
        )
        // HTTP/3 mimicry is always-on for Warren tunnels, so the
        // connection summary surfaces it even with every other flag off.
        assertEquals("Connected (mimicry)", proxy.state.value)
    }

    @Test
    fun `Connected with feature flags renders inline feature list`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(
            WarrenTunnelState.Connected(
                exitId = "abc",
                assignedNatPmpPort = 51234,
                multiHop = true,
                daita = true,
            )
        )
        val label = proxy.state.value
        assertTrue(label.startsWith("Connected ("), "got: $label")
        assertTrue(label.contains("multi-hop"), "got: $label")
        assertTrue(label.contains("DAITA"), "got: $label")
        assertTrue(label.contains("mimicry"), "got: $label")
        assertTrue(label.contains("port 51234"), "got: $label")
    }

    @Test
    fun `Reconnecting renders Reconnecting label`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(WarrenTunnelState.Reconnecting)
        assertEquals("Reconnecting...", proxy.state.value)
    }

    @Test
    fun `Failed renders Failed with reason`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(WarrenTunnelState.Failed("handshake timeout"))
        assertEquals("Failed: handshake timeout", proxy.state.value)
    }
}
