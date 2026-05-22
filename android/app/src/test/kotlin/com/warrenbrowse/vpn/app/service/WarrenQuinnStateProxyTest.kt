package com.warrenbrowse.vpn.app.service

import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenQuinnStateProxyTest {

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
    fun `Connected without feature flags renders plain Connected label`() = runTest {
        val proxy = WarrenQuinnStateProxy()
        proxy.update(
            WarrenTunnelState.Connected(
                exitId = "abc",
                assignedNatPmpPort = null,
                multiHop = false,
                daita = false,
                obfuscationM40 = false,
            )
        )
        assertEquals("Connected", proxy.state.value)
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
                obfuscationM40 = true,
            )
        )
        val label = proxy.state.value
        assertTrue(label.startsWith("Connected ("), "got: $label")
        assertTrue(label.contains("multi-hop"), "got: $label")
        assertTrue(label.contains("DAITA"), "got: $label")
        assertTrue(label.contains("M4.0"), "got: $label")
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
