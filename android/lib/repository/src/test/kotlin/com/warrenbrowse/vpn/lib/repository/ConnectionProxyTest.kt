package com.warrenbrowse.vpn.lib.repository

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
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
        assertTrue(mapped(WarrenConnectedInfo.Connecting()) is TunnelState.Connecting)
        assertTrue(mapped(WarrenConnectedInfo.Reconnecting()) is TunnelState.Connecting)
    }

    @Test
    fun `Disconnecting maps to a plain teardown`() = runTest {
        val state = mapped(WarrenConnectedInfo.Disconnecting()) as TunnelState.Disconnecting
        assertEquals(ActionAfterDisconnect.Nothing, state.actionAfterDisconnect)
    }

    @Test
    fun `a teardown that is on its way back up maps to the reconnect variant`() = runTest {
        val state =
            mapped(WarrenConnectedInfo.Disconnecting(reconnecting = true))
                as TunnelState.Disconnecting
        assertEquals(ActionAfterDisconnect.Reconnect, state.actionAfterDisconnect)
    }

    @Test
    fun `Connecting carries the dialled exit endpoint so the In row is populated`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connecting(
                exitEndpointHost = "185.65.135.10:443",
                entryEndpointHost = "146.70.1.2:51820",
            ),
        ) as TunnelState.Connecting

        assertEquals("185.65.135.10", state.endpoint?.endpoint?.address?.address?.hostAddress)
        assertEquals("146.70.1.2", state.endpoint?.entryEndpoint?.address?.address?.hostAddress)
    }

    @Test
    fun `Connecting with no dialled host yet leaves the endpoint unknown`() = runTest {
        val state = mapped(WarrenConnectedInfo.Connecting()) as TunnelState.Connecting
        assertNull(state.endpoint)
    }

    @Test
    fun `Connecting raises the feature chips before the tunnel is up`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connecting(multiHop = true, daita = true),
        ) as TunnelState.Connecting

        assertTrue(state.featureIndicators.contains(FeatureIndicator.DAITA_MULTIHOP))
        assertTrue(state.featureIndicators.contains(FeatureIndicator.QUIC))
        // Port forwarding advertises a granted mapping, which no dial has yet.
        assertFalse(state.featureIndicators.contains(FeatureIndicator.PORT_FORWARDING))
    }

    @Test
    fun `a dispatched connect shows Connecting before the engine has moved`() = runTest {
        info.value = WarrenConnectedInfo.Disconnected
        proxy.onConnectRequested(multiHop = true, daita = false)

        val state = proxy.tunnelState.first() as TunnelState.Connecting
        assertTrue(state.featureIndicators.contains(FeatureIndicator.MULTIHOP))
    }

    @Test
    fun `the optimistic frame gives way to the engine state`() = runTest {
        info.value = WarrenConnectedInfo.Disconnected
        proxy.onConnectRequested()
        assertTrue(proxy.tunnelState.first() is TunnelState.Connecting)

        val state = mapped(WarrenConnectedInfo.Failed("boom"))
        assertTrue(state is TunnelState.Error, "got: $state")
    }

    @Test
    fun `an answered optimistic frame is not revived by an identical engine state`() = runTest {
        info.value = WarrenConnectedInfo.Disconnected
        proxy.onConnectRequested()
        assertTrue(proxy.tunnelState.first() is TunnelState.Connecting)

        // The engine answers, then comes all the way back to the state the
        // request was dispatched against.
        assertTrue(mapped(WarrenConnectedInfo.Connecting()) is TunnelState.Connecting)
        assertTrue(mapped(WarrenConnectedInfo.Disconnected) is TunnelState.Disconnected)
    }

    @Test
    fun `a request that never reached the engine drops its optimistic frame`() = runTest {
        info.value = WarrenConnectedInfo.Disconnected
        proxy.onConnectRequested()
        proxy.onRequestAborted()

        assertTrue(proxy.tunnelState.first() is TunnelState.Disconnected)
    }

    @Test
    fun `a dispatched disconnect tears the card down before the engine confirms`() = runTest {
        info.value = connectedInfo()
        proxy.onDisconnectRequested()

        val state = proxy.tunnelState.first() as TunnelState.Disconnecting
        assertEquals(ActionAfterDisconnect.Nothing, state.actionAfterDisconnect)
    }

    @Test
    fun `a disconnect dispatched with nothing to tear down shows no teardown`() = runTest {
        info.value = WarrenConnectedInfo.Disconnected
        proxy.onDisconnectRequested()

        assertTrue(proxy.tunnelState.first() is TunnelState.Disconnected)
    }

    private fun connectedInfo() = WarrenConnectedInfo.Connected(
        exitEndpointHost = "185.65.135.10:443",
        entryEndpointHost = null,
        multiHop = false,
        daita = false,
        assignedNatPmpPort = null,
    )

    @Test
    fun `a reconnect bridges the teardown gap so Disconnected never reaches the card`() = runTest {
        info.value = connectedInfo()
        proxy.onReconnectRequested()

        proxy.tunnelState.test {
            assertTrue(awaitItem() is TunnelState.Connected)

            info.value = WarrenConnectedInfo.Disconnecting(reconnecting = true)
            val teardown = awaitItem() as TunnelState.Disconnecting
            assertEquals(ActionAfterDisconnect.Reconnect, teardown.actionAfterDisconnect)

            // The engine passes through Disconnected between dropping the old
            // session and dialling the next one. That frame would flip the
            // scenery to the exposed art and collapse the card for ~200ms.
            info.value = WarrenConnectedInfo.Disconnected
            testScheduler.runCurrent()
            expectNoEvents()

            info.value = WarrenConnectedInfo.Connecting(exitEndpointHost = "185.65.135.11:443")
            assertTrue(awaitItem() is TunnelState.Connecting)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `a genuine disconnect still shows Disconnected`() = runTest {
        info.value = connectedInfo()
        proxy.onDisconnectRequested()

        proxy.tunnelState.test {
            val teardown = awaitItem() as TunnelState.Disconnecting
            assertEquals(ActionAfterDisconnect.Nothing, teardown.actionAfterDisconnect)

            info.value = WarrenConnectedInfo.Disconnected
            assertTrue(awaitItem() is TunnelState.Disconnected)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `the reconnect bridge is spent once the new tunnel is up`() = runTest {
        info.value = connectedInfo()
        proxy.onReconnectRequested()

        proxy.tunnelState.test {
            assertTrue(awaitItem() is TunnelState.Connected)

            info.value = WarrenConnectedInfo.Disconnected
            assertTrue(awaitItem() is TunnelState.Disconnecting)

            info.value = WarrenConnectedInfo.Connecting()
            assertTrue(awaitItem() is TunnelState.Connecting)

            // Re-dialling the SAME exit lands on an identical engine state, so
            // the bridge cannot be retired by comparing against its baseline.
            info.value = connectedInfo()
            assertTrue(awaitItem() is TunnelState.Connected)

            // Spent: the next teardown is the user's own disconnect.
            info.value = WarrenConnectedInfo.Disconnected
            assertTrue(awaitItem() is TunnelState.Disconnected)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    @Test
    fun `a reconnect the engine never answers stops bridging`() = runTest {
        info.value = connectedInfo()
        proxy.onReconnectRequested()

        proxy.tunnelState.test {
            assertTrue(awaitItem() is TunnelState.Connected)

            info.value = WarrenConnectedInfo.Disconnected
            assertTrue(awaitItem() is TunnelState.Disconnecting)

            testScheduler.advanceTimeBy(ConnectionProxy.OPTIMISTIC_FRAME_TIMEOUT_MILLIS + 1)
            assertTrue(awaitItem() is TunnelState.Disconnected)
            cancelAndIgnoreRemainingEvents()
        }
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
    fun `a non-flapping Blocking carries the kill-switch-active cause (not a firewall error)`() =
        runTest {
            val state = mapped(WarrenConnectedInfo.Blocking("kill switch")) as TunnelState.Error
            // The kill switch succeeded in blocking; it must NOT be reported as
            // a FirewallPolicyError (which means the firewall could not be
            // applied and tells the user to send a problem report).
            assertTrue(state.errorState.cause is ErrorStateCause.WarrenKillSwitchActive)
            assertTrue(state.errorState.isBlocking)
        }

    @Test
    fun `a flapping Blocking stays blocking but carries the flap cause`() = runTest {
        val state =
            mapped(WarrenConnectedInfo.Blocking("flapping", flapping = true)) as TunnelState.Error
        // Kill switch still engaged (no leak)...
        assertTrue(state.errorState.isBlocking)
        // ...but the user sees "network unstable" rather than a firewall error.
        assertTrue(state.errorState.cause is ErrorStateCause.WarrenTunnelFlapping)
    }

    @Test
    fun `an expired Blocking stays blocking but carries the expired-account cause`() = runTest {
        val state =
            mapped(WarrenConnectedInfo.Blocking("expired", expired = true)) as TunnelState.Error
        // Kill switch still engaged (no leak)...
        assertTrue(state.errorState.isBlocking)
        // ...and the user sees an actionable "subscription expired" auth error.
        val cause = state.errorState.cause
        assertTrue(cause is ErrorStateCause.AuthFailed)
        assertTrue((cause as ErrorStateCause.AuthFailed).isCausedByExpiredAccount())
    }

    @Test
    fun `an expired Failed is a non-blocking expired-account error`() = runTest {
        val state =
            mapped(WarrenConnectedInfo.Failed("expired", expired = true)) as TunnelState.Error
        assertFalse(state.errorState.isBlocking)
        val cause = state.errorState.cause
        assertTrue(cause is ErrorStateCause.AuthFailed)
        assertTrue((cause as ErrorStateCause.AuthFailed).isCausedByExpiredAccount())
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
    fun `Connected with a granted NAT-PMP port exposes the PORT_FORWARDING chip`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "185.65.135.10:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = false,
                assignedNatPmpPort = 49160,
            ),
        ) as TunnelState.Connected

        assertTrue(state.featureIndicators.contains(FeatureIndicator.PORT_FORWARDING))
    }

    @Test
    fun `Connected without a NAT-PMP mapping omits the PORT_FORWARDING chip`() = runTest {
        val state = mapped(
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "185.65.135.10:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = false,
                assignedNatPmpPort = null,
            ),
        ) as TunnelState.Connected

        assertFalse(state.featureIndicators.contains(FeatureIndicator.PORT_FORWARDING))
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
