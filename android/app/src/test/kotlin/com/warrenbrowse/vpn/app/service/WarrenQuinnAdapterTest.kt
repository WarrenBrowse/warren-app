package com.warrenbrowse.vpn.app.service

import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The network-handover path of [WarrenQuinnAdapter], driven through the
 * [WarrenTunnelPlatform] seam.
 *
 * Two properties are pinned here, and only the second one is about the user's
 * connection: the first is about a leak. A handover must reach the native
 * migration watchdog without tearing anything down, and the fallback that runs
 * when the watchdog gives up must establish the blocking TUN BEFORE it drops
 * the live one. Inverting that order hands traffic back to the physical
 * network with no interface capturing it.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class WarrenQuinnAdapterTest {

    private companion object {
        const val ESTABLISH_LIVE = "establish(live)"
        const val ESTABLISH_BLACKHOLE = "establish(blackhole)"
        const val CONNECT_TUNNEL = "connectTunnel"
        const val DISCONNECT_TUNNEL = "disconnectTunnel"
        const val AWAIT_CLOSED = "awaitTunnelClosed"
        const val NOTIFY_NETWORK_CHANGED = "notifyNetworkChanged"
        const val CLOSE_ACTIVE = "close(activeTun)"

        const val STATUS_CONNECTED = 2
        const val STATUS_DISCONNECTED = 0

        const val PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"
    }

    /**
     * Records every platform call in order and lets a test drive the native
     * status the adapter polls. The file descriptors are mocks that record
     * their own `close()`, which is how the ordering assertion sees the live
     * TUN being dropped.
     */
    private class RecordingPlatform : WarrenTunnelPlatform {
        val calls = mutableListOf<String>()

        @Volatile
        var status: Int = STATUS_CONNECTED
        var callback: ConnectivityManager.NetworkCallback? = null

        /** The fd the adapter keeps as `activeFd` (a dup of the live one). */
        private val activeTun: ParcelFileDescriptor = fd("activeTun")
        private val liveTun: ParcelFileDescriptor = fd("liveTun", dupTo = activeTun)
        private val blackholeTun: ParcelFileDescriptor = fd("blackhole")

        private fun fd(tag: String, dupTo: ParcelFileDescriptor? = null): ParcelFileDescriptor {
            val descriptor = mockk<ParcelFileDescriptor>(relaxed = true)
            every { descriptor.close() } answers { calls += "close($tag)" }
            every { descriptor.detachFd() } returns 7
            if (dupTo != null) every { descriptor.dup() } returns dupTo
            return descriptor
        }

        override fun establish(plan: WarrenTunInterfacePlan): ParcelFileDescriptor {
            calls += if (plan.blocking) ESTABLISH_BLACKHOLE else ESTABLISH_LIVE
            return if (plan.blocking) blackholeTun else liveTun
        }

        override fun connectTunnel(tunFd: Int, mnemonic: String, configJson: String): Int {
            calls += CONNECT_TUNNEL
            return 0
        }

        override fun disconnectTunnel() {
            calls += DISCONNECT_TUNNEL
        }

        override fun awaitTunnelClosed(timeoutMs: Long): Boolean {
            calls += AWAIT_CLOSED
            return true
        }

        override fun notifyNetworkChanged() {
            calls += NOTIFY_NETWORK_CHANGED
        }

        // Polled every 250 ms: recording them would drown the sequence.
        override fun tunnelStatus(): Int = status

        override fun natPmpStatus(): String = "{\"state\":\"idle\"}"

        override fun autoRecoveryCount(): Int = 0

        @Volatile
        var health: Int = PATH_HEALTH_HEALTHY

        override fun pathHealth(): Int = health

        override fun registerNetworkCallback(
            callback: ConnectivityManager.NetworkCallback
        ): Boolean {
            this.callback = callback
            return true
        }

        override fun unregisterNetworkCallback(callback: ConnectivityManager.NetworkCallback) {
            this.callback = null
        }
    }

    private fun config() = WarrenTunnelConfig(
        exitPubkeyHex = "ab".repeat(32),
        exitEndpoint = "exit.example:443",
        walletPubkeyHex = "cd".repeat(32),
        lockdownMode = true,
    )

    private fun adapterWith(platform: RecordingPlatform): WarrenQuinnAdapter {
        val settings = mockk<WarrenLocalSettingsRepository>(relaxed = true)
        every { settings.splitTunnelingEnabled } returns MutableStateFlow(false)
        every { settings.excludedApps } returns MutableStateFlow(emptySet())
        return WarrenQuinnAdapter(
            vpnService = mockk<VpnService>(relaxed = true),
            connectivityManager = mockk<ConnectivityManager>(relaxed = true),
            settings = settings,
            connectivity = MutableStateFlow<Connectivity>(Connectivity.PresumeOnline),
            platform = platform,
        )
    }

    /**
     * Connect, then hand back the callback the adapter registered and a
     * recorder cleared of the connect sequence, so an assertion only sees what
     * the handover itself did.
     */
    private suspend fun connectedAdapter(
        platform: RecordingPlatform,
        baseline: Network = mockk<Network>(),
    ): Pair<WarrenQuinnAdapter, ConnectivityManager.NetworkCallback> {
        val adapter = adapterWith(platform)
        adapter.connect(config(), Mnemonic(PHRASE))
        val callback = platform.callback
        checkNotNull(callback) { "connect() must register a network callback" }
        // The status poll runs on the adapter's own IO scope; let it settle on
        // Connected first, so a handover is never raced by the very transition
        // that clears its pending flag.
        awaitReal("the session must reach Connected") {
            adapter.state.value is WarrenTunnelState.Connected
        }
        // First sighting of a network is the session baseline, never a handover.
        callback.onAvailable(baseline)
        platform.calls.clear()
        return adapter to callback
    }

    /**
     * Await [predicate] in REAL time, or fail with what was recorded. The
     * adapter polls the native status on its own `Dispatchers.IO` scope, which
     * `runTest`'s virtual clock does not drive.
     */
    private suspend fun awaitReal(what: String, predicate: () -> Boolean) {
        withContext(Dispatchers.Default) {
            try {
                withTimeout(5_000) {
                    while (!predicate()) delay(20)
                }
            } catch (e: kotlinx.coroutines.TimeoutCancellationException) {
                throw AssertionError(what, e)
            }
        }
    }

    /**
     * The nominal handover: the adapter tells the native migration watchdog and
     * stops there. The live TUN keeps holding the routes and the session keeps
     * running, so a teardown here would be a 15 s outage the watchdog exists to
     * remove (and a blackhole nobody needs).
     */
    @Test
    fun `handover notifies native and does not tear down`() = runTest {
        val platform = RecordingPlatform()
        val (adapter, callback) = connectedAdapter(platform)

        callback.onAvailable(mockk<Network>())

        val calls = platform.calls.toList()
        assertTrue(
            NOTIFY_NETWORK_CHANGED in calls,
            "a handover must reach the native migration watchdog, got: $calls"
        )
        assertFalse(
            DISCONNECT_TUNNEL in calls,
            "the nominal handover must not tear the session down, got: $calls"
        )
        assertFalse(
            ESTABLISH_BLACKHOLE in calls,
            "the live TUN still holds the routes, so no blackhole is needed, got: $calls"
        )
        assertFalse(
            CLOSE_ACTIVE in calls,
            "the live TUN must survive the migration, got: $calls"
        )
        adapter.disconnect()
    }

    /**
     * A handover reaches the app as two independent system events, and
     * `ConnectivityManager` does not order them. This is the gentle order, the
     * replacement is up before the old network goes away.
     */
    @Test
    fun `handover notifies native when the new network arrives before the old one is lost`() =
        runTest {
            val platform = RecordingPlatform()
            val old = mockk<Network>()
            val new = mockk<Network>()
            val (adapter, callback) = connectedAdapter(platform, baseline = old)

            callback.onAvailable(new)
            callback.onLost(old)

            val calls = platform.calls.toList()
            assertTrue(
                NOTIFY_NETWORK_CHANGED in calls,
                "a handover must reach the native migration watchdog, got: $calls"
            )
            assertFalse(
                DISCONNECT_TUNNEL in calls,
                "losing the network the datapath already left must not tear the session down, " +
                    "got: $calls"
            )
            adapter.disconnect()
        }

    /**
     * The abrupt order, which is what walking out of Wi-Fi range looks like:
     * the network carrying the datapath dies before its replacement is up.
     * Reading that replacement as a fresh session baseline skips the migration
     * silently, leaving the user on the slow teardown-and-redial fallback, so
     * losing the carrying network is itself a change event.
     */
    @Test
    fun `handover notifies native when the old network is lost before the new one arrives`() =
        runTest {
            val platform = RecordingPlatform()
            val old = mockk<Network>()
            val new = mockk<Network>()
            val (adapter, callback) = connectedAdapter(platform, baseline = old)

            callback.onLost(old)
            callback.onAvailable(new)

            val calls = platform.calls.toList()
            assertTrue(
                NOTIFY_NETWORK_CHANGED in calls,
                "the watchdog must be told whichever way the system orders the two events, " +
                    "got: $calls"
            )
            assertFalse(
                DISCONNECT_TUNNEL in calls,
                "the migration must still be tried before any teardown, got: $calls"
            )
            adapter.disconnect()
        }

    /**
     * The fallback, taken when the watchdog escalated and the native status
     * went disconnected: the order is the whole safety property. A blocking TUN
     * must hold the routes BEFORE the session is torn down and BEFORE the live
     * fd is closed, because a closed TUN with no successor hands every packet
     * straight back to the physical network. Swapping these calls must fail
     * this test.
     */
    @Test
    fun `fallback poses blackhole before closing the tun`() = runTest {
        val platform = RecordingPlatform()
        val (adapter, callback) = connectedAdapter(platform)

        callback.onAvailable(mockk<Network>())
        // The watchdog could neither migrate nor redial the path, so it ended
        // the session: that is what Kotlin sees as a disconnect.
        platform.status = STATUS_DISCONNECTED

        awaitReal("the escalation must trigger the handover fallback") {
            CLOSE_ACTIVE in platform.calls.toList()
        }
        val calls = platform.calls.toList()

        val blackhole = calls.indexOf(ESTABLISH_BLACKHOLE)
        val disconnect = calls.indexOf(DISCONNECT_TUNNEL)
        val closed = calls.indexOf(CLOSE_ACTIVE)
        assertTrue(blackhole >= 0, "the fallback must establish a blocking TUN, got: $calls")
        assertTrue(
            blackhole < disconnect,
            "the blocking TUN must hold the routes before the session is torn down, got: $calls"
        )
        assertTrue(
            blackhole < closed,
            "closing the live TUN before its successor is up is the leak, got: $calls"
        )
        assertTrue(
            disconnect < closed,
            "the native side must release its fd copy before the adapter drops its own, got: $calls"
        )
        adapter.disconnect()
    }

    @Test
    fun `ensure a wedged datapath is exposed while the session stays connected`() = runTest {
        // The engine's dead-path watches cannot see this class: the transport
        // stays up and only the goodput prober notices nothing crosses. If the
        // adapter does not surface it, the UI keeps claiming protection on a
        // tunnel that carries nothing.
        val platform = RecordingPlatform()
        val (adapter, _) = connectedAdapter(platform)
        assertFalse(adapter.pathWedged.value, "a healthy path must not read as wedged")

        platform.health = PATH_HEALTH_DEGRADED_BOTH
        awaitReal("the wedge must reach the adapter") { adapter.pathWedged.value }
        assertTrue(adapter.state.value is WarrenTunnelState.Connected, "still Connected")

        // A last-mile shrink is NOT a wedge: it has its own MSS/PMTU handling
        // and must not be reported as a dead datapath.
        platform.health = PATH_HEALTH_DEGRADED_LARGE
        awaitReal("a large-frame degradation must not read as wedged") { !adapter.pathWedged.value }

        platform.health = PATH_HEALTH_HEALTHY
        awaitReal("recovery must clear the wedge") { !adapter.pathWedged.value }
    }

    @Test
    fun `ensure a dead egress reads as wedged`() = runTest {
        // The in-tunnel egress probe's own verdict: the exit answers the client
        // and forwards nothing to the internet. The goodput prober stays green
        // on it (the exit answers the tunnel gateway), so this value is the only
        // thing that can stop the card claiming protection.
        val platform = RecordingPlatform()
        val (adapter, _) = connectedAdapter(platform)
        assertFalse(adapter.pathWedged.value, "a healthy path must not read as wedged")

        platform.health = PATH_HEALTH_EGRESS_DEAD
        awaitReal("a dead egress must reach the adapter") { adapter.pathWedged.value }
        assertTrue(adapter.state.value is WarrenTunnelState.Connected, "still Connected")

        platform.health = PATH_HEALTH_HEALTHY
        awaitReal("recovery must clear the wedge") { !adapter.pathWedged.value }
    }

    @Test
    fun `ensure the verdict that ended the session survives into the drop`() = runTest {
        // The egress verdict ENDS the session it fires on, in the same
        // millisecond, so a poll that read the health only after deciding the
        // session was gone would never see it: the whole detection would be
        // invisible above the FFI. The reading has to be taken before the drop
        // is acted on, so the card keeps saying "interrupted" while the
        // fail-closed policy blocks and redials.
        // The drop path stamps the flap detector off the framework clock.
        mockkStatic(SystemClock::class)
        every { SystemClock.elapsedRealtime() } returns 0L
        try {
            val platform = RecordingPlatform()
            val (adapter, _) = connectedAdapter(platform)
            assertFalse(adapter.pathWedged.value, "a healthy path must not read as wedged")

            platform.health = PATH_HEALTH_EGRESS_DEAD
            platform.status = STATUS_DISCONNECTED
            awaitReal("the verdict must survive the teardown it caused") {
                adapter.pathWedged.value
            }
        } finally {
            unmockkStatic(SystemClock::class)
        }
    }

    @Test
    fun `ensure an exit switch waits for the old session to close before dialling`() = runTest {
        // Desktop sequences this through a Disconnecting state that awaits the
        // tunnel close event; Android used to signal the cancel and dial 40 ms
        // later, so the new session registered on the exit while the old one
        // still held the sticky inner IP. The exit then handed the downlink to
        // whichever side its takeover rule picked, and the fresh session was
        // black-holed until a probe killed it.
        val platform = RecordingPlatform()
        val (adapter, _) = connectedAdapter(platform)
        platform.calls.clear()

        adapter.reconnectWith(config())

        val closed = platform.calls.indexOf(AWAIT_CLOSED)
        val dialled = platform.calls.indexOf(CONNECT_TUNNEL)
        assertTrue(closed >= 0, "the reconnect must await the old session's close: ${platform.calls}")
        assertTrue(
            closed < dialled,
            "must await the close BEFORE dialling, got ${platform.calls}",
        )
    }
}
