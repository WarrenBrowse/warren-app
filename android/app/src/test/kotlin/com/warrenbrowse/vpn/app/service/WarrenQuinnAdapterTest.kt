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
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotEquals
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
        const val STATUS_RECONNECTING = 3

        const val PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"

        const val NANOS_PER_MILLI = 1_000_000L
    }

    /**
     * Records every platform call in order and lets a test drive the native
     * status the adapter watches. The file descriptors are mocks that record
     * their own `close()`, which is how the ordering assertion sees the live
     * TUN being dropped.
     *
     * The status generation mirrors the engine's: every published change
     * advances it and wakes a parked `awaitStatusChange`, and the native
     * connect and teardown publish too. [wakeOnChange] off models a wake that
     * was never delivered, so only the caller's timeout can return.
     */
    private class RecordingPlatform : WarrenTunnelPlatform {
        val calls = mutableListOf<String>()

        /** The thread each platform call ran on, by call name (last one wins). */
        val threads = mutableMapOf<String, String>()

        /** How many times the adapter read the native status. */
        val statusReads = AtomicInteger()

        @Volatile
        var wakeOnChange = true

        private val lock = ReentrantLock()
        private val changed = lock.newCondition()
        private var generation = 0L

        @Volatile
        var status: Int = STATUS_CONNECTED
            set(value) {
                field = value
                publish()
            }

        var callback: ConnectivityManager.NetworkCallback? = null

        private fun publish() =
            lock.withLock {
                generation++
                if (wakeOnChange) changed.signalAll()
            }

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
            val call = if (plan.blocking) ESTABLISH_BLACKHOLE else ESTABLISH_LIVE
            calls += call
            threads[call] = Thread.currentThread().name
            return if (plan.blocking) blackholeTun else liveTun
        }

        /** The wire config of every dial, in order. */
        val configs = mutableListOf<String>()

        /** A status the native side reports as soon as a dial is issued, when set. */
        @Volatile
        var statusOnConnect: Int? = null

        override fun connectTunnel(tunFd: Int, mnemonic: String, configJson: String): Int {
            calls += CONNECT_TUNNEL
            configs += configJson
            threads[CONNECT_TUNNEL] = Thread.currentThread().name
            statusOnConnect?.let { status = it }
            publish()
            return 0
        }

        override fun disconnectTunnel() {
            calls += DISCONNECT_TUNNEL
            threads[DISCONNECT_TUNNEL] = Thread.currentThread().name
            publish()
        }

        override fun awaitTunnelClosed(timeoutMs: Long): Boolean {
            calls += AWAIT_CLOSED
            return true
        }

        override fun notifyNetworkChanged() {
            calls += NOTIFY_NETWORK_CHANGED
        }

        override fun awaitStatusChange(lastSeen: Long, timeoutMs: Long): Long =
            lock.withLock {
                var remainingNanos = timeoutMs * NANOS_PER_MILLI
                while (generation == lastSeen && remainingNanos > 0) {
                    remainingNanos = changed.awaitNanos(remainingNanos)
                }
                generation
            }

        // Read on every wake: recording them would drown the sequence.
        override fun tunnelStatus(): Int {
            statusReads.incrementAndGet()
            return status
        }

        override fun natPmpStatus(): String = "{\"state\":\"idle\"}"

        override fun autoRecoveryCount(): Int = 0

        @Volatile
        var health: Int = PATH_HEALTH_HEALTHY
            set(value) {
                field = value
                publish()
            }

        override fun pathHealth(): Int = health

        @Volatile
        var mtuVerdict: Int = 0
            set(value) {
                field = value
                publish()
            }

        override fun effectiveMtu(): Int = mtuVerdict

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

    private fun adapterWith(
        platform: RecordingPlatform,
        dispatcher: CoroutineDispatcher = Dispatchers.IO,
        failoverConfig: (WarrenTunnelConfig) -> WarrenTunnelConfig? = { null },
        dropRetryGraceMs: Long = 15_000L,
    ): WarrenQuinnAdapter {
        val settings = mockk<WarrenLocalSettingsRepository>(relaxed = true)
        every { settings.splitTunnelingEnabled } returns MutableStateFlow(false)
        every { settings.excludedApps } returns MutableStateFlow(emptySet())
        return WarrenQuinnAdapter(
            vpnService = mockk<VpnService>(relaxed = true),
            connectivityManager = mockk<ConnectivityManager>(relaxed = true),
            settings = settings,
            connectivity = MutableStateFlow<Connectivity>(Connectivity.PresumeOnline),
            platform = platform,
            dispatcher = dispatcher,
            failoverConfig = failoverConfig,
            dropRetryGraceMs = dropRetryGraceMs,
        )
    }

    /** A dispatcher whose only thread carries a name an assertion can read. */
    private fun namedDispatcher(name: String): CoroutineDispatcher =
        Executors.newSingleThreadExecutor { runnable -> Thread(runnable, name) }
            .asCoroutineDispatcher()

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
        // The status watch runs on its own IO thread; let it settle on
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
     * adapter watches the native status on `Dispatchers.IO`, which
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
        adapter.disconnect()
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
        adapter.disconnect()
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
            // The drop armed a 15 s retry that would redial, read the same
            // dead status and stamp the clock again once it is unmocked below,
            // in whichever test class happens to be running by then.
            adapter.disconnect()
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
        adapter.disconnect()
    }

    /**
     * The status used to be polled four times a second for the life of the
     * session, which is a permanent wake source on a phone that is otherwise
     * idle. The engine now wakes the watch on every change, so with nothing
     * changing the adapter must read nothing.
     */
    @Test
    fun `ensure an idle session reads the native status only on a wake`() = runTest {
        val platform = RecordingPlatform()
        val (adapter, _) = connectedAdapter(platform)
        platform.statusReads.set(0)

        withContext(Dispatchers.Default) { delay(700) }

        val reads = platform.statusReads.get()
        assertTrue(reads <= 1, "an idle session must not poll the status: $reads reads in 700 ms")
        adapter.disconnect()
    }

    /**
     * A transition the engine publishes must land on its wake, not on the
     * bounded fallback wait that only exists for a wake that was never
     * delivered.
     */
    @Test
    fun `ensure a native transition reaches the adapter on its wake`() = runTest {
        val platform = RecordingPlatform()
        val (adapter, _) = connectedAdapter(platform)

        val started = System.nanoTime()
        platform.status = STATUS_RECONNECTING
        awaitReal("the redial must reach the adapter") {
            adapter.state.value is WarrenTunnelState.Reconnecting
        }

        val elapsedMs = (System.nanoTime() - started) / NANOS_PER_MILLI
        assertTrue(
            elapsedMs < 400,
            "the transition must land on the wake, not after the fallback wait: $elapsedMs ms",
        )
        adapter.disconnect()
    }

    /**
     * The fallback is the whole safety net: a wake that never arrives must
     * still leave the card telling the truth within a bounded time.
     */
    @Test
    fun `ensure a lost wake is covered by the fallback read`() = runTest {
        val platform = RecordingPlatform()
        platform.wakeOnChange = false
        val (adapter, _) = connectedAdapter(platform)

        platform.status = STATUS_RECONNECTING

        awaitReal("the fallback read must pick the transition up") {
            adapter.state.value is WarrenTunnelState.Reconnecting
        }
        adapter.disconnect()
    }

    /**
     * `VpnService.Builder.establish()` is a Binder round trip into
     * `system_server` and `connectTunnel` parses the config, derives the
     * wallet key (PBKDF2) and registers the TUN with the engine. The service
     * calls the adapter from its lifecycle scope, which is the main thread,
     * so unless the adapter leaves that thread itself the connect animation
     * stalls for the whole sequence on every tap.
     */
    @Test
    fun `ensure connect establishes and dials on the adapter dispatcher`() = runTest {
        val platform = RecordingPlatform()
        val adapter = adapterWith(platform, dispatcher = namedDispatcher("adapter-io"))
        val caller = Thread.currentThread().name

        adapter.connect(config(), Mnemonic(PHRASE))

        assertNotEquals(caller, "adapter-io", "the test itself must not run on the dispatcher")
        assertEquals(
            "adapter-io",
            platform.threads[ESTABLISH_LIVE],
            "establish() must run on the adapter's dispatcher, got: ${platform.threads}",
        )
        assertEquals(
            "adapter-io",
            platform.threads[CONNECT_TUNNEL],
            "connectTunnel() must run on the adapter's dispatcher, got: ${platform.threads}",
        )
        adapter.disconnect()
    }

    /**
     * The teardown is the other stall: the native session abort plus the fd
     * juggling, reached from the same main-thread lifecycle scope on every
     * Disconnect tap and on every revoke.
     */
    @Test
    fun `ensure disconnect tears the session down on the adapter dispatcher`() = runTest {
        val platform = RecordingPlatform()
        val adapter = adapterWith(platform, dispatcher = namedDispatcher("adapter-io"))
        adapter.connect(config(), Mnemonic(PHRASE))
        awaitReal("the session must reach Connected") {
            adapter.state.value is WarrenTunnelState.Connected
        }
        platform.threads.clear()

        adapter.disconnect()

        assertEquals(
            "adapter-io",
            platform.threads[DISCONNECT_TUNNEL],
            "disconnectTunnel() must run on the adapter's dispatcher, got: ${platform.threads}",
        )
    }

    /**
     * Desktop parity (`assemble_failover_for_attempt`): the retry after an
     * unexpected drop dials the alternative the failover rule hands back, and
     * the switch is reported once that dial lands, never before, because the
     * banner promises a working alternative.
     */
    @Test
    fun `ensure a drop retry dials the failover exit and reports the switch once it lands`() =
        runTest {
            mockkStatic(SystemClock::class)
            every { SystemClock.elapsedRealtime() } returns 0L
            try {
                val platform = RecordingPlatform()
                val alternative =
                    config().copy(exitPubkeyHex = "ef".repeat(32), exitEndpoint = "exit2.example:443")
                val adapter =
                    adapterWith(platform, failoverConfig = { alternative }, dropRetryGraceMs = 0L)
                adapter.connect(config(), Mnemonic(PHRASE))
                awaitReal("the session must reach Connected") {
                    adapter.state.value is WarrenTunnelState.Connected
                }
                assertEquals(0, adapter.failoverCount.value)

                platform.statusOnConnect = STATUS_CONNECTED
                platform.status = STATUS_DISCONNECTED
                awaitReal("the retry must dial the alternative exit") {
                    platform.configs.lastOrNull()?.contains("ef".repeat(32)) == true
                }
                awaitReal("the landed retry must report one switch") {
                    adapter.failoverCount.value == 1
                }
                val landed = adapter.state.value as WarrenTunnelState.Connected
                assertEquals("exit2.example:443", landed.exitEndpointHost)
                adapter.disconnect()
            } finally {
                unmockkStatic(SystemClock::class)
            }
        }

    /** With no alternative the retry redials the same exit and reports no switch. */
    @Test
    fun `ensure a drop retry without an alternative redials the same exit silently`() = runTest {
        mockkStatic(SystemClock::class)
        every { SystemClock.elapsedRealtime() } returns 0L
        try {
            val platform = RecordingPlatform()
            val adapter = adapterWith(platform, failoverConfig = { null }, dropRetryGraceMs = 0L)
            adapter.connect(config(), Mnemonic(PHRASE))
            awaitReal("the session must reach Connected") {
                adapter.state.value is WarrenTunnelState.Connected
            }

            platform.statusOnConnect = STATUS_CONNECTED
            platform.status = STATUS_DISCONNECTED
            awaitReal("the retry must redial") { platform.configs.size == 2 }
            awaitReal("the redial must land") {
                adapter.state.value is WarrenTunnelState.Connected
            }
            assertTrue(platform.configs.last().contains("ab".repeat(32)))
            assertEquals(0, adapter.failoverCount.value)
            adapter.disconnect()
        } finally {
            unmockkStatic(SystemClock::class)
        }
    }

    /**
     * The engine's "Reduced MTU" verdict rides the status wake like the other
     * datapath facts, and a torn-down session takes its verdict with it: the
     * chip describes the live path, never the previous one.
     */
    @Test
    fun `ensure the reduced MTU verdict reaches the adapter and clears with the session`() =
        runTest {
            val platform = RecordingPlatform()
            val (adapter, _) = connectedAdapter(platform)
            assertEquals(null, adapter.effectiveMtu.value)

            platform.mtuVerdict = 1216
            awaitReal("the verdict must reach the adapter") { adapter.effectiveMtu.value == 1216 }

            adapter.disconnect()
            awaitReal("a torn-down session must clear the verdict") {
                adapter.effectiveMtu.value == null
            }
        }

    /**
     * Releasing traffic is the one drop path that hands the device back to the
     * bare network: a flapping tunnel with lockdown off. The native session
     * must be retired there like on every other teardown: its API connection
     * pool was opened through the interface that just went away, and a pool
     * left standing serves those dead sockets to the next request.
     */
    @Test
    fun `ensure a flapping tunnel without lockdown retires the native session before releasing`() =
        runTest {
            mockkStatic(SystemClock::class)
            every { SystemClock.elapsedRealtime() } returns 0L
            try {
                val platform = RecordingPlatform()
                val adapter = adapterWith(platform, dropRetryGraceMs = 0L)
                adapter.connect(config().copy(lockdownMode = false), Mnemonic(PHRASE))
                awaitReal("the session must reach Connected") {
                    adapter.state.value is WarrenTunnelState.Connected
                }

                // Every redial dies on arrival: the flap guard trips and, with
                // lockdown off, the policy releases traffic.
                platform.statusOnConnect = STATUS_DISCONNECTED
                platform.status = STATUS_DISCONNECTED
                awaitReal("the flapping tunnel must release traffic") {
                    adapter.state.value is WarrenTunnelState.Failed
                }

                val calls = platform.calls.toList()
                val lastDial = calls.lastIndexOf(CONNECT_TUNNEL)
                val released = calls.lastIndexOf(CLOSE_ACTIVE)
                assertTrue(released > lastDial, "the release closes the live TUN, got: $calls")
                assertTrue(
                    DISCONNECT_TUNNEL in calls.subList(lastDial, released),
                    "the release must retire the native session before dropping the TUN, got: " +
                        calls.subList(lastDial, calls.size),
                )
            } finally {
                unmockkStatic(SystemClock::class)
            }
        }

    /** The "Last" row needs the moment the count moved, not only the count. */
    @Test
    fun `ensure a landed recovery stamps the time of the last recovery`() = runTest {
        mockkStatic(SystemClock::class)
        every { SystemClock.elapsedRealtime() } returns 12_345L
        try {
            val platform = RecordingPlatform()
            val adapter = adapterWith(platform, failoverConfig = { null }, dropRetryGraceMs = 0L)
            adapter.connect(config(), Mnemonic(PHRASE))
            awaitReal("the session must reach Connected") {
                adapter.state.value is WarrenTunnelState.Connected
            }
            assertEquals(null, adapter.lastAutoRecoveryAtMs.value)

            platform.statusOnConnect = STATUS_CONNECTED
            platform.status = STATUS_DISCONNECTED
            awaitReal("the retry must be counted as a recovery") {
                adapter.autoRecoveryCount.value == 1
            }
            awaitReal("the recovery must be stamped") {
                adapter.lastAutoRecoveryAtMs.value == 12_345L
            }
            adapter.disconnect()
        } finally {
            unmockkStatic(SystemClock::class)
        }
    }
}
