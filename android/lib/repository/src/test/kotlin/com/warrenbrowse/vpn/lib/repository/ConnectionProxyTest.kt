package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.TunnelState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ConnectionProxyTest {

    private val info = MutableStateFlow<WarrenConnectedInfo>(WarrenConnectedInfo.Disconnected)
    private val provider = FakeTunnelStateProvider(info)
    private val proxy = ConnectionProxy(provider)

    private suspend fun mapped(next: WarrenConnectedInfo): TunnelState {
        info.value = next
        return proxy.tunnelState.first()
    }

    @Test
    fun `Disconnected maps to TunnelState Disconnected`() = runTest {
        assertTrue(mapped(WarrenConnectedInfo.Disconnected) is TunnelState.Disconnected)
    }

    @Test
    fun `Connecting and Reconnecting both map to Connecting`() = runTest {
        assertTrue(mapped(WarrenConnectedInfo.Connecting) is TunnelState.Connecting)
        assertTrue(mapped(WarrenConnectedInfo.Reconnecting) is TunnelState.Connecting)
    }

    @Test
    fun `Failed maps to a non-blocking Error (not Disconnected)`() = runTest {
        val state = mapped(WarrenConnectedInfo.Failed("boom"))
        assertTrue(state is TunnelState.Error, "got: $state")
        assertFalse((state as TunnelState.Error).errorState.isBlocking)
    }

    @Test
    fun `Blocking (kill switch) maps to a blocking Error`() = runTest {
        val state = mapped(WarrenConnectedInfo.Blocking("kill switch"))
        assertTrue(state is TunnelState.Error, "got: $state")
        assertTrue((state as TunnelState.Error).errorState.isBlocking)
    }

    @Test
    fun `Connected single-hop with DAITA exposes real endpoint and DAITA + QUIC chips`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "185.65.135.10:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = true,
                assignedNatPmpPort = null,
            ),
        ) as TunnelState.Connected

        assertEquals("185.65.135.10", state.endpoint.endpoint.address.address?.hostAddress)
        assertEquals(443, state.endpoint.endpoint.address.port)
        assertNull(state.endpoint.entryEndpoint)
        assertTrue(state.featureIndicators.contains(FeatureIndicator.DAITA))
        assertTrue(state.featureIndicators.contains(FeatureIndicator.QUIC))
        assertFalse(state.featureIndicators.contains(FeatureIndicator.MULTIHOP))
    }

    @Test
    fun `Connected multihop with DAITA collapses to DAITA_MULTIHOP and carries entry endpoint`() =
        runTest {
            val state = mapped(
                WarrenConnectedInfo.Connected(
                    exitEndpointHost = "185.65.135.10:443",
                    entryEndpointHost = "146.70.1.2:51820",
                    multiHop = true,
                    daita = true,
                    assignedNatPmpPort = 49160,
                ),
            ) as TunnelState.Connected

            assertEquals("146.70.1.2", state.endpoint.entryEndpoint?.address?.address?.hostAddress)
            assertTrue(state.featureIndicators.contains(FeatureIndicator.DAITA_MULTIHOP))
            assertTrue(state.featureIndicators.contains(FeatureIndicator.QUIC))
            assertFalse(state.featureIndicators.contains(FeatureIndicator.DAITA))
        }

    @Test
    fun `Connected with an unparseable host falls back to the sentinel endpoint`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "not-an-ip-host:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = false,
                assignedNatPmpPort = null,
            ),
        ) as TunnelState.Connected

        // Hostnames are rejected (no DNS on the collecting thread) → sentinel.
        assertEquals("0.0.0.0", state.endpoint.endpoint.address.address?.hostAddress)
        assertEquals(0, state.endpoint.endpoint.address.port)
        // QUIC is always present for Warren tunnels.
        assertTrue(state.featureIndicators.contains(FeatureIndicator.QUIC))
    }

    private class FakeTunnelStateProvider(
        private val infoFlow: MutableStateFlow<WarrenConnectedInfo>,
    ) : WarrenTunnelStateProvider {
        override val state: StateFlow<String> = MutableStateFlow("").asStateFlow()
        override val connectedInfo: StateFlow<WarrenConnectedInfo> = infoFlow.asStateFlow()
    }
}
