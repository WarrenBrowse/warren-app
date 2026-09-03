package com.warrenbrowse.vpn.lib.model

import java.net.InetSocketAddress
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The Android arm of the desktop `isMultihopTunnelState`: a circuit is
 * multi-hop when the tunnel is up or coming up through an entry relay that is
 * a different host from the exit. Only the host is compared: the entry and
 * exit legs of a one-hop circuit can differ by port alone.
 */
class TunnelStateMultihopTest {

    private fun endpoint(host: String, port: Int = 443) =
        Endpoint(InetSocketAddress.createUnresolved(host, port), TransportProtocol.Udp)

    private fun tunnelEndpoint(entry: Endpoint?, exit: Endpoint) =
        TunnelEndpoint(
            entryEndpoint = entry,
            endpoint = exit,
            quantumResistant = false,
            obfuscation = null,
            daita = false,
        )

    @Test
    fun `connected through a distinct entry relay is a two-hop circuit`() {
        val state =
            TunnelState.Connected(
                endpoint = tunnelEndpoint(endpoint("relay.example"), endpoint("exit.example")),
                location = null,
                featureIndicators = emptyList(),
            )

        assertTrue(state.isMultihopCircuit())
    }

    @Test
    fun `a dial through a distinct entry relay already reads as two hops`() {
        val state =
            TunnelState.Connecting(
                endpoint = tunnelEndpoint(endpoint("relay.example"), endpoint("exit.example")),
                location = null,
                featureIndicators = emptyList(),
            )

        assertTrue(state.isMultihopCircuit())
    }

    @Test
    fun `no entry relay is one hop`() {
        val state =
            TunnelState.Connected(
                endpoint = tunnelEndpoint(null, endpoint("exit.example")),
                location = null,
                featureIndicators = emptyList(),
            )

        assertFalse(state.isMultihopCircuit())
    }

    @Test
    fun `the same host on both legs is one hop whatever the ports and case`() {
        val state =
            TunnelState.Connected(
                endpoint = tunnelEndpoint(endpoint("Exit.Example", 4433), endpoint("exit.example")),
                location = null,
                featureIndicators = emptyList(),
            )

        assertFalse(state.isMultihopCircuit())
    }

    @Test
    fun `a dial with no endpoint yet, and every torn-down state, is not a circuit`() {
        assertFalse(
            TunnelState.Connecting(endpoint = null, location = null, featureIndicators = emptyList())
                .isMultihopCircuit()
        )
        assertFalse(TunnelState.Disconnected().isMultihopCircuit())
        assertFalse(TunnelState.Disconnecting(ActionAfterDisconnect.Nothing).isMultihopCircuit())
    }
}
