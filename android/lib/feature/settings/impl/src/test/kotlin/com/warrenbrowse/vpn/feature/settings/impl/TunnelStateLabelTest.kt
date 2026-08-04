package com.warrenbrowse.vpn.feature.settings.impl

import android.content.Context
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.MockKAnswerScope
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

// getString(int, vararg Any) delivers its format args as a single Array in
// invocation.args[1] (mockk does not flatten varargs); fall back to flattened
// args defensively.
private fun MockKAnswerScope<String, *>.fmtArgs(): List<Any?> =
    (invocation.args.getOrNull(1) as? Array<*>)?.toList() ?: invocation.args.drop(1)

/**
 * The settings banner is built from the typed tunnel state, so every state and
 * every feature chip resolves to a string resource. The engine failure reason
 * is developer text: it must never reach the banner.
 */
class TunnelStateLabelTest {

    private val context: Context = mockk {
        every { getString(R.string.tunnel_state_disconnected) } returns "Disconnected"
        every { getString(R.string.tunnel_state_connecting) } returns "Connecting…"
        every { getString(R.string.tunnel_state_reconnecting) } returns "Reconnecting…"
        every { getString(R.string.tunnel_state_disconnecting) } returns "Disconnecting…"
        every { getString(R.string.tunnel_state_connected) } returns "Connected"
        every { getString(R.string.tunnel_state_blocking) } returns "Blocking internet (kill switch)"
        every { getString(R.string.tunnel_state_failed) } returns "Connection failed"
        every { getString(R.string.multihop) } returns "Multihop"
        every { getString(R.string.daita) } returns "DAITA"
        every { getString(R.string.tunnel_feature_mimicry) } returns "Mimicry"
        every { getString(eq(R.string.tunnel_feature_port), any()) } answers
            { "Port ${fmtArgs()[0]}" }
        every { getString(eq(R.string.tunnel_state_with_features), any(), any()) } answers
            { "${fmtArgs()[0]} (${fmtArgs()[1]})" }
    }

    @Test
    fun `every non-connected state resolves to its own resource`() {
        assertEquals("Disconnected", tunnelStateLabel(context, WarrenConnectedInfo.Disconnected))
        assertEquals("Connecting…", tunnelStateLabel(context, WarrenConnectedInfo.Connecting()))
        assertEquals("Reconnecting…", tunnelStateLabel(context, WarrenConnectedInfo.Reconnecting()))
        assertEquals(
            "Disconnecting…",
            tunnelStateLabel(context, WarrenConnectedInfo.Disconnecting()),
        )
    }

    @Test
    fun `a reconnecting teardown reads as reconnecting, not as a disconnect`() {
        assertEquals(
            "Reconnecting…",
            tunnelStateLabel(context, WarrenConnectedInfo.Disconnecting(reconnecting = true)),
        )
    }

    @Test
    fun `a connected tunnel lists its live features`() {
        val info = WarrenConnectedInfo.Connected(
            exitEndpointHost = "1.2.3.4:443",
            entryEndpointHost = "5.6.7.8:443",
            multiHop = true,
            daita = true,
            assignedNatPmpPort = 51820,
        )
        assertEquals(
            "Connected (Multihop, DAITA, Mimicry, Port 51820)",
            tunnelStateLabel(context, info),
        )
    }

    @Test
    fun `mimicry is always listed because it is not togglable`() {
        val info = WarrenConnectedInfo.Connected(
            exitEndpointHost = "1.2.3.4:443",
            entryEndpointHost = null,
            multiHop = false,
            daita = false,
            assignedNatPmpPort = null,
        )
        assertEquals("Connected (Mimicry)", tunnelStateLabel(context, info))
    }

    @Test
    fun `a failure renders a generic line and never the engine reason`() {
        val label = tunnelStateLabel(
            context,
            WarrenConnectedInfo.Failed("VpnService.Builder.establish() returned null"),
        )
        assertEquals("Connection failed", label)
        assertFalse(label.contains("establish"))
    }

    @Test
    fun `a blocking state names the kill switch and never the engine reason`() {
        val label = tunnelStateLabel(context, WarrenConnectedInfo.Blocking("connectTunnel returned -3"))
        assertEquals("Blocking internet (kill switch)", label)
        assertFalse(label.contains("connectTunnel"))
    }

    @Test
    fun `only a live tunnel offers the reconnect affordance`() {
        val connected = WarrenConnectedInfo.Connected("1.2.3.4:443", null, false, false, null)
        assertTrue(tunnelStateIsConnected(connected))
        assertFalse(tunnelStateIsConnected(WarrenConnectedInfo.Connecting()))
        assertFalse(tunnelStateIsConnected(WarrenConnectedInfo.Disconnected))
        assertFalse(tunnelStateIsConnected(WarrenConnectedInfo.Blocking("x")))
    }
}
