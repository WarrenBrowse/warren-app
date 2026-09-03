package com.warrenbrowse.vpn.app.service

import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import java.io.IOException
import co.touchlab.kermit.Logger
import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.vpn.app.connectivity.canDialRelay
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

// Owns the Warren Quinn tunnel lifecycle for a given `WarrenVpnService`
// instance. Sits between the Android `VpnService` API and the Rust JNI:
//
//   WarrenVpnService -- onStartCommand --> WarrenQuinnAdapter.connect(config)
//                                                |
//                                                v
//                                       VpnService.Builder.establish()
//                                                |
//                                                v
//                              WarrenTunnelPlatform.connectTunnel(fd, configJson)
//                                                |
//                                                v
//                                  warren_tunnel Quinn pump
//
// State machine + reconnect-on-network-change live here, not in Rust.
//
// Every transition runs on [dispatcher], never on the caller's thread: the
// service reaches the adapter from its lifecycle scope, which is the main
// thread, and `establish()` (a Binder round trip into system_server), the
// native `connectTunnel` (config parse, PBKDF2 key derivation, TUN
// registration) and the teardown all take tens of milliseconds at the exact
// moment the connect animation starts. `WarrenQuinnAdapterTest` pins the
// thread affinity.
//
// TODO: the builder configuration (DNS, bypass CIDRs, multi-hop entry
// plumbing) is not wired yet.
class WarrenQuinnAdapter(
    vpnService: VpnService,
    connectivityManager: ConnectivityManager,
    private val settings: WarrenLocalSettingsRepository,
    private val connectivity: StateFlow<Connectivity>,
    // Every VpnService / native call goes through here so the handover
    // sequence's ORDER is observable off-device; see [WarrenTunnelPlatform].
    private val platform: WarrenTunnelPlatform =
        AndroidTunnelPlatform(vpnService, connectivityManager),
    // Where the platform and native calls run; injectable so a test can name
    // the thread it expects them on.
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)
    private val lock = Mutex()

    private val _state = MutableStateFlow<WarrenTunnelState>(WarrenTunnelState.Disconnected)
    val state: StateFlow<WarrenTunnelState> = _state.asStateFlow()

    // Live NAT-PMP port-forwarding status (raw JSON from the Rust side), read
    // on every status wake. `idle` when no mapping is active.
    private val _natPmpStatus = MutableStateFlow(NATPMP_IDLE)
    val natPmpStatus: StateFlow<String> = _natPmpStatus.asStateFlow()

    // Total automatic recoveries since process start: the Rust redial
    // engine's in-session redials (autoRecoveryCount) plus the
    // adapter's own retry-loop successes (AutoRecoveryTracker). Mirrors the
    // desktop reconnect_count row; user actions never count.
    private val autoRecovery = AutoRecoveryTracker()
    private val _autoRecoveryCount = MutableStateFlow(0)
    val autoRecoveryCount: StateFlow<Int> = _autoRecoveryCount.asStateFlow()

    // True while the engine's goodput prober reports a wedged datapath: the
    // session is up and nothing crosses it. The dead-path watches are blind to
    // this class, so without it the UI keeps claiming protection on a tunnel
    // that carries nothing.
    private val _pathWedged = MutableStateFlow(false)
    val pathWedged: StateFlow<Boolean> = _pathWedged.asStateFlow()

    private var activeConfig: WarrenTunnelConfig? = null
    // Held as a zeroizable [Mnemonic] (CharArray-backed), NOT a String: the
    // recovery phrase must not linger as a long-lived immutable String on the
    // JVM heap for the whole session (a heap dump would extract it verbatim).
    // The adapter owns this instance and wipes it via close() on teardown.
    // Do NOT switch this back to a String, it would defeat zeroization.
    private var activeMnemonic: Mnemonic? = null
    private var activeFd: ParcelFileDescriptor? = null
    private var statusWatchJob: Job? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    // The network believed to carry the datapath, `null` once it is lost and
    // before its replacement shows up. A change of THIS is what the migration
    // watchdog is told about, which is what makes the detection independent of
    // the order the system reports the two halves of a handover in.
    private var datapathNetwork: Network? = null
    // Whether the session has already adopted its baseline network. Without it,
    // a replacement arriving after the carrying network was lost would read as
    // a fresh baseline (`datapathNetwork` being null again) and no migration
    // would ever be asked for.
    private var datapathNetworkSeen = false
    private var pendingHandover: Job? = null
    // Set when a handover was handed to the native migration watchdog, cleared
    // once the session comes back or the fallback consumes it. It is what tells
    // a `Disconnected` apart: after a notification that status is the
    // watchdog's escalation (it could neither migrate nor redial the path), and
    // the answer is a fresh dial on the new network, not the generic drop
    // policy. A session death from another cause inside this window takes the
    // same fallback, which is strictly fail-closed (blackhole up, then redial).
    @Volatile
    private var handoverNotified = false

    // Kill-switch (lockdown) state. When the tunnel drops while lockdown is
    // enabled we keep `blockingFd` established as a blackhole interface so
    // traffic stays captured instead of leaking to the physical network.
    private var blockingFd: ParcelFileDescriptor? = null
    // Distinguishes a user-requested teardown (release traffic) from an
    // unexpected drop (engage the kill switch).
    private var userInitiatedDisconnect = false

    // Detects a flapping tunnel so the lockdown reconnect loop stops hammering
    // a network that is clearly down. Reset on a real connect + user teardown.
    private val flapDetector = FlapDetector()

    init {
        // Re-establish the TUN when the split-tunnelling selection changes while
        // connected, so newly excluded/included apps take effect without a
        // manual reconnect. The current-value emission is skipped (drop(1)); a
        // reconnect only fires from a settled Connected state.
        scope.launch {
            combine(settings.splitTunnelingEnabled, settings.excludedApps) { enabled, apps ->
                if (enabled) apps else emptySet()
            }
                .drop(1)
                .distinctUntilChanged()
                .collect {
                    if (activeConfig != null && _state.value is WarrenTunnelState.Connected) {
                        Logger.i("split-tunnelling selection changed; re-establishing tunnel")
                        reconnect()
                    }
                }
        }
    }

    /** Package names to route outside the tunnel, or empty when split off. */
    private fun currentExcludedApps(): Set<String> =
        if (settings.splitTunnelingEnabled.value) settings.excludedApps.value else emptySet()

    /**
     * Establish a Quinn session. Ownership of [mnemonic] transfers to the
     * adapter, which holds it (zeroizable) for the session so the reconnect /
     * handover paths can re-derive the SigningKey without re-prompting
     * BiometricPrompt, and wipes it via close() on teardown. Internal
     * reconnect paths pass the already-owned mnemonic back in (same instance),
     * which is reused rather than re-stored.
     */
    suspend fun connect(config: WarrenTunnelConfig, mnemonic: Mnemonic) =
        withContext(dispatcher) { connectLocked(config, mnemonic) }

    private suspend fun connectLocked(config: WarrenTunnelConfig, mnemonic: Mnemonic) = lock.withLock {
        if (_state.value !is WarrenTunnelState.Disconnected) {
            Logger.w("WarrenQuinnAdapter: connect() called while not disconnected")
            // Never zero the session's own mnemonic; only wipe a foreign one
            // that we are refusing to take ownership of.
            if (mnemonic !== activeMnemonic) mnemonic.close()
            return@withLock
        }
        _state.value = connectingFrom(config)
        userInitiatedDisconnect = false
        handoverNotified = false
        activeConfig = config
        if (mnemonic !== activeMnemonic) {
            activeMnemonic?.close()
            activeMnemonic = mnemonic
        }

        // Ensure the native side holds no stale tunnel before establishing a
        // new one. On a session drop/block the Quinn task dies but the native
        // ACTIVE_TUNNEL slot is cleared only by disconnectTunnel(), so a
        // reconnect would otherwise hit "Tunnel already running". Idempotent:
        // a no-op on the first connect (slot empty).
        platform.disconnectTunnel()

        val fd = buildTunInterface(config) ?: run {
            onSessionDown(config, "VpnService.Builder.establish() returned null")
            return@withLock
        }
        // Retain a dup of the TUN fd so the VPN interface stays established even
        // after the native side closes its copy on an unexpected session death:
        // traffic keeps entering the now-unread TUN (blocked, not leaked) until
        // the status watch installs the real blackhole. Without this the
        // interface tears down the instant Rust drops the fd, leaking clear
        // traffic on the physical link until the drop is observed. The
        // fail-closed logic below relies on activeFd being a LIVE handle to the
        // interface.
        activeFd = try {
            fd.dup()
        } catch (e: IOException) {
            Logger.e(throwable = e) { "failed to dup TUN fd; aborting connect" }
            fd.close()
            onSessionDown(config, "TUN fd dup failed")
            return@withLock
        }

        val rc = try {
            mnemonic.useAsString { phrase ->
                platform.connectTunnel(fd.detachFd(), phrase, config.toWireJson())
            }
        } catch (e: IllegalStateException) {
            // The cached mnemonic was wiped between scheduling and this
            // (re)connect, e.g. a user disconnect raced an automatic retry.
            // Fail closed instead of crashing the VPN service.
            Logger.w(throwable = e) { "connect: mnemonic unavailable, aborting" }
            onSessionDown(config, "mnemonic unavailable")
            return@withLock
        } catch (e: RuntimeException) {
            // Any other native-thrown error (e.g. a residual "Tunnel already
            // running" from a state desync). Fail closed into onSessionDown
            // rather than let an uncaught exception crash the VpnService.
            Logger.w(throwable = e) { "connect: native connectTunnel threw, aborting" }
            onSessionDown(config, e.message ?: "connectTunnel threw")
            return@withLock
        }
        if (rc != 0) {
            onSessionDown(config, "connectTunnel returned $rc")
            return@withLock
        }

        // The real tunnel is being (re)established: drop any blackhole
        // interface left over from a previous lockdown drop.
        exitBlockingMode()

        // Follow the Rust-side session status and translate transitions back
        // to `WarrenTunnelState`. The native side wakes this loop on every
        // change it publishes (a status edge, a datapath verdict, a NAT-PMP
        // transition, a landed redial), so a transition reaches the UI the
        // moment it happens and an idle session costs no wakeups; the bounded
        // wait is only the safety net for a wake that never came.
        val sessionConfig = config
        statusWatchJob = scope.launch(Dispatchers.IO) {
            var lastSeen = STATUS_CONNECTING
            var seenGeneration = 0L
            while (isActive) {
                // Parks the thread in the engine for up to the fallback period,
                // which is why this loop runs on IO and not on [dispatcher]: a
                // single-thread dispatcher would starve every other transition.
                seenGeneration = platform.awaitStatusChange(seenGeneration, STATUS_WAKE_FALLBACK_MS)
                if (!isActive) break
                val code = platform.tunnelStatus()
                // Read the datapath verdict BEFORE acting on the status. The
                // egress verdict ends the session it fires on, in the same
                // millisecond, so a reading taken after the drop branch would
                // never see it and the detection would be invisible above the
                // FFI. Taken first, it keeps the card saying "interrupted"
                // while the fail-closed policy blocks and redials.
                //
                // A dead datapath and a dead egress are both wedges; a
                // large-frame degradation is a last-mile shrink with its own
                // handling and must not read as one.
                val health = platform.pathHealth()
                val wedged =
                    health == PATH_HEALTH_DEGRADED_BOTH || health == PATH_HEALTH_EGRESS_DEAD
                if (wedged != _pathWedged.value) _pathWedged.value = wedged
                if (code != lastSeen) {
                    lastSeen = code
                    Logger.i("WarrenQuinnAdapter: native status $code")
                    if (code == STATUS_DISCONNECTED || code < 0) {
                        // Rust task ended (clean exit or error). Engage the
                        // kill switch on an unexpected drop under lockdown;
                        // otherwise release and stop polling.
                        val reason =
                            if (code < 0) "native status code $code" else "tunnel disconnected"
                        var escalatedHandover = false
                        lock.withLock {
                            if (userInitiatedDisconnect) {
                                _state.value = WarrenTunnelState.Disconnected
                            } else if (handoverNotified) {
                                // The migration watchdog escalated: the path is
                                // dead on the network we just moved onto, so a
                                // fresh dial is the answer, not the generic drop
                                // policy (which would release traffic outright
                                // when lockdown is off).
                                handoverNotified = false
                                escalatedHandover = true
                                _state.value = reconnectingFrom(sessionConfig)
                            } else {
                                onSessionDown(sessionConfig, reason)
                            }
                        }
                        // Off-lock: the fallback takes it in its own coroutine.
                        if (escalatedHandover) {
                            Logger.i(
                                "WarrenQuinnAdapter: migration watchdog escalated; " +
                                    "falling back to a full reconnect"
                            )
                            scheduleHandoverReconnect()
                        }
                        break
                    }
                    if (code == STATUS_UNAUTHORIZED) {
                        // Terminal: the exit refused the account (lapsed /
                        // revoked subscription). Retrying cannot recover it,
                        // so engage the kill switch per lockdown policy but
                        // stop the reconnect loop and surface "expired".
                        lock.withLock {
                            if (userInitiatedDisconnect) {
                                _state.value = WarrenTunnelState.Disconnected
                            } else {
                                onSessionExpired(sessionConfig)
                            }
                        }
                        break
                    }
                    // A real connection settles the network: forget prior
                    // drops so a later isolated drop is not mistaken for the
                    // tail of an earlier flap.
                    if (code == STATUS_CONNECTED) {
                        // The migration landed (or a redial did): a later
                        // disconnect is no longer this handover's escalation.
                        handoverNotified = false
                        flapDetector.reset()
                        if (autoRecovery.onConnected()) {
                            Logger.i("WarrenQuinnAdapter: automatic recovery landed")
                        }
                    }
                    _state.value = statusFromCode(code, sessionConfig)
                }
                // Mirror the live NAT-PMP status; its transitions ride the
                // same wake.
                val np = platform.natPmpStatus()
                if (np != _natPmpStatus.value) _natPmpStatus.value = np
                // Sum the native in-session redials with the adapter's own
                // retry-loop recoveries; a redial that lands wakes this loop
                // even though it has no status edge of its own.
                val recoveries = platform.autoRecoveryCount() + autoRecovery.count
                if (recoveries != _autoRecoveryCount.value) {
                    _autoRecoveryCount.value = recoveries
                }
            }
        }

        // Register a NetworkCallback for handover-triggered reconnect
        // (Wi-Fi <-> cellular <-> ethernet). The reconnect happens
        // off-lock via `triggerHandoverReconnect` so the callback returns
        // immediately to the ConnectivityManager.
        registerNetworkCallback()
    }

    /**
     * Tear down the active Quinn session and immediately reconnect using
     * the cached [activeConfig] + [activeMnemonic]. Used by the user-
     * facing Reconnect button (and by the OS handover flow); does NOT
     * re-prompt for biometric auth because we have the mnemonic still
     * in process memory from the prior connect.
     *
     * No-op when there is no active session; the user must use the
     * normal connect flow in that case.
     */

    /**
     * Waits for the session just torn down to actually finish winding down.
     *
     * The desktop daemon sequences a relay switch through a Disconnecting
     * state that awaits the tunnel close event before reconnecting. Android
     * used to signal the cancel and dial ~40 ms later, so the new session
     * registered on the exit while the old one still held the sticky inner IP;
     * the exit's takeover rule then decided which of the two kept the
     * downlink, and the fresh session could be left black-holed. Bounded so a
     * session that refuses to die can never wedge the reconnect: dialling late
     * is better than not dialling.
     */
    private suspend fun awaitPreviousSessionClosed() {
        // Blocking native wait; the callers already sit on [dispatcher].
        val closed = platform.awaitTunnelClosed(SESSION_CLOSE_TIMEOUT_MS)
        if (!closed) {
            Logger.w("WarrenQuinnAdapter: previous session still closing, dialling anyway")
        }
        // The local task finishing means the CONNECTION_CLOSE was SENT, not
        // that the exit has processed it, and the exit only releases the
        // sticky inner IP once it has. Desktop gets this settle for free: its
        // Disconnecting state also tears down routes and firewall rules, which
        // takes far longer than one RTT. Here the wind-down is ~200 ms, so
        // without an explicit settle a switch still raced the exit 2 times in 6.
        delay(PEER_CLOSE_SETTLE_MS)
    }

    suspend fun reconnect() = withContext(dispatcher) {
        val config = activeConfig
        val mnemonic = activeMnemonic
        if (config == null || mnemonic == null) {
            Logger.w("WarrenQuinnAdapter: reconnect() called without an active session")
            return@withContext
        }
        lock.withLock { teardownLocked(reconnecting = true) }
        awaitPreviousSessionClosed()
        // Reuse the same owned Mnemonic instance (connect() detects identity
        // and does not re-store or wipe it).
        connectLocked(config, mnemonic)
    }

    /**
     * Tear down the active session and reconnect with a freshly built
     * [newConfig], reusing the cached [activeMnemonic] (no biometric
     * re-prompt). Unlike [reconnect], this applies settings changed since the
     * last connect (exit relay, entry country, DAITA, NAT-PMP, DNS, ...) by
     * swapping in a config the caller rebuilt from the current settings.
     *
     * No-op when there is no active session (no cached mnemonic to reuse); the
     * user must use the normal connect flow in that case.
     */
    suspend fun reconnectWith(newConfig: WarrenTunnelConfig) = withContext(dispatcher) {
        val mnemonic = activeMnemonic
        if (mnemonic == null) {
            Logger.w("WarrenQuinnAdapter: reconnectWith() called without an active session")
            return@withContext
        }
        lock.withLock { teardownLocked(reconnecting = true) }
        awaitPreviousSessionClosed()
        // teardownLocked() clears activeConfig but keeps activeMnemonic, so the
        // instance we captured is still the session's own: connect() detects the
        // identity and does not re-store or wipe it.
        connectLocked(newConfig, mnemonic)
    }

    suspend fun disconnect() = withContext(dispatcher) {
        lock.withLock {
            teardownLocked()
            // User teardown is terminal: wipe the cached mnemonic.
            activeMnemonic?.close()
            activeMnemonic = null
        }
    }

    /**
     * [disconnect] without waiting for it, on the adapter's own scope. For the
     * service's `onDestroy`, whose lifecycle scope is already cancelled by the
     * time it runs and which must not block the main thread on the teardown.
     */
    fun disconnectInBackground(): Job = scope.launch { disconnect() }

    /**
     * Tear down the active session (cancel polling, drop the JNI tunnel and
     * TUN fd, clear the blackhole) and return to [WarrenTunnelState.Disconnected].
     * Must be called holding [lock]. Does NOT wipe [activeMnemonic] so the
     * reconnect path can reuse it; the public [disconnect] wipes it after.
     *
     * [reconnecting] marks a teardown a re-dial is already queued behind, so
     * the card reads it as a connection in progress rather than a disconnect.
     */
    private fun teardownLocked(reconnecting: Boolean = false) {
        // The native teardown and the fd juggling below take real time, and the
        // card must not keep offering a live Connect (or a green Connected)
        // while the tunnel is coming down.
        _state.value = WarrenTunnelState.Disconnecting(reconnecting)
        // Intentional teardown: release traffic instead of engaging the
        // kill switch.
        userInitiatedDisconnect = true
        handoverNotified = false
        flapDetector.reset()
        // A user action clears any pending recovery attribution: whatever
        // connects next is not an automatic recovery.
        autoRecovery.onUserAction()
        unregisterNetworkCallback()
        pendingHandover?.cancel()
        pendingHandover = null
        platform.disconnectTunnel()
        statusWatchJob?.cancel()
        statusWatchJob = null
        activeFd?.close()
        activeFd = null
        exitBlockingMode()
        activeConfig = null
        datapathNetwork = null
        datapathNetworkSeen = false
        _natPmpStatus.value = NATPMP_IDLE
        _state.value = WarrenTunnelState.Disconnected
    }

    private fun buildTunInterface(config: WarrenTunnelConfig): ParcelFileDescriptor? =
        platform.establish(planTunInterface(config, excludedApps = currentExcludedApps()))

    /**
     * Handle the active tunnel going down. Must be called holding [lock].
     *
     * When [WarrenTunnelConfig.lockdownMode] is on and the drop was not
     * user-initiated, establish a blackhole interface (kill switch) so
     * traffic stays blocked, then schedule a reconnect. Otherwise surface a
     * [WarrenTunnelState.Failed] and release traffic.
     */
    private fun onSessionDown(config: WarrenTunnelConfig, reason: String) {
        // CRITICAL (fail-closed): do NOT close activeFd here. Closing the only
        // TUN before a replacement is up opens a leak window (and a permanent
        // leak if the replacement fails to establish). The TUN is closed only
        // once a successor interface is confirmed up (enterBlockingMode) or the
        // user explicitly released traffic (RELEASE below). An active TUN whose
        // pump has died still drops everything, so keeping it is leak-safe.
        _natPmpStatus.value = NATPMP_IDLE
        // Only an unexpected drop counts as a flap; a user teardown must not
        // record one. The blackhole-up-FIRST behaviour (block, then retry) is
        // the Mullvad fail-closed model and lives in KillSwitchPolicy.
        val flapping =
            !userInitiatedDisconnect && flapDetector.recordDrop(SystemClock.elapsedRealtime())
        when (KillSwitchPolicy.decide(userInitiatedDisconnect, flapping, config.lockdownMode)) {
            KillSwitchAction.RELEASE -> {
                // The user asked to release traffic: now it is safe to drop the
                // TUN (this is the only path that returns traffic to the bare
                // network, and only on explicit intent).
                Logger.w("WarrenQuinnAdapter: releasing traffic ($reason)")
                activeFd?.close()
                activeFd = null
                exitBlockingMode()
                unregisterNetworkCallback()
                activeMnemonic?.close()
                activeMnemonic = null
                _state.value = WarrenTunnelState.Failed(reason)
            }
            KillSwitchAction.PARK -> {
                // Kill switch on and flapping: stay blocked until the user
                // acts. The network callback stays registered so a genuine
                // handover or a user reconnect resumes the normal flow.
                Logger.w("WarrenQuinnAdapter: tunnel flapping, parking ($reason)")
                enterBlockingMode(config, reason, flapping = true)
            }
            KillSwitchAction.BLOCK_AND_RETRY -> {
                enterBlockingMode(config, reason)
                scheduleDropReconnect(config)
            }
        }
    }

    /**
     * Establish (or keep) a kill-switch blackhole interface that captures
     * all traffic but pumps nothing, so it is dropped instead of leaking to
     * the physical network. Must be called holding [lock].
     *
     * Fail-closed ordering: the blackhole is established BEFORE the stale
     * active TUN is closed (on Android `establish()` atomically replaces the
     * current interface, so there is never a window with no TUN). If the
     * blackhole cannot be established, the existing active TUN is KEPT (its
     * pump is dead, so it already drops everything) rather than torn down,
     * so traffic still cannot leak.
     */
    private fun enterBlockingMode(
        config: WarrenTunnelConfig,
        reason: String,
        flapping: Boolean = false,
        expired: Boolean = false,
    ) {
        if (blockingFd == null) {
            val fd = platform.establish(planTunInterface(config, blocking = true))
            if (fd == null) {
                // Could not stand up the dedicated blackhole. Keep whatever
                // interface is currently up (the active TUN with a dead pump)
                // as the fail-closed blackhole - do NOT close it. Traffic stays
                // captured and dropped; we are still blocked, never leaking.
                Logger.e(
                    "WarrenQuinnAdapter: blackhole establish failed; keeping current " +
                        "interface as fail-closed blackhole ($reason)"
                )
                _state.value = WarrenTunnelState.Blocking(reason, flapping, expired)
                return
            }
            blockingFd = fd
            // The blackhole atomically replaced the active interface; the old
            // active fd is now stale, so close it (the interface itself stays
            // up as the blackhole).
            activeFd?.close()
            activeFd = null
            Logger.w("WarrenQuinnAdapter: lockdown engaged, traffic blocked ($reason)")
        }
        _state.value = WarrenTunnelState.Blocking(reason, flapping, expired)
    }

    /**
     * Handle the exit refusing the account (lapsed / revoked subscription).
     * Must be called holding [lock]. Unlike [onSessionDown] this NEVER
     * schedules a reconnect: retrying the same unauthorized account just
     * re-hits the rejection (a reconnect storm). Under lockdown the kill
     * switch stays engaged (fail-closed, no leak) with an "expired" cause;
     * without lockdown, traffic is released and a non-blocking expired error
     * is surfaced. The user recovers by renewing then reconnecting.
     */
    private fun onSessionExpired(config: WarrenTunnelConfig) {
        _natPmpStatus.value = NATPMP_IDLE
        flapDetector.reset()
        if (config.lockdownMode) {
            Logger.w("WarrenQuinnAdapter: account unauthorized; blocking (subscription expired)")
            enterBlockingMode(config, "subscription expired", flapping = false, expired = true)
        } else {
            Logger.w("WarrenQuinnAdapter: account unauthorized; releasing (subscription expired)")
            activeFd?.close()
            activeFd = null
            exitBlockingMode()
            unregisterNetworkCallback()
            activeMnemonic?.close()
            activeMnemonic = null
            _state.value = WarrenTunnelState.Failed("subscription expired", expired = true)
        }
    }

    /** Tear down the kill-switch blackhole interface, if any. */
    private fun exitBlockingMode() {
        blockingFd?.close()
        blockingFd = null
    }

    /**
     * After an unexpected drop, retry the real tunnel once the grace period
     * elapses. The blackhole interface stays up until [connect] confirms a
     * new tunnel (it calls [exitBlockingMode] on success), so there is no
     * leak window between attempts. Repeated failures re-enter blocking via
     * [onSessionDown], forming a bounded retry loop.
     */
    private fun scheduleDropReconnect(config: WarrenTunnelConfig) {
        val mnemonic = activeMnemonic ?: return
        pendingHandover?.cancel()
        pendingHandover = scope.launch {
            autoRecovery.armAutomation()
            delay(DROP_RETRY_GRACE_MS)
            awaitDialableNetwork()
            lock.withLock {
                if (userInitiatedDisconnect) return@withLock
                statusWatchJob?.cancel()
                statusWatchJob = null
                // Reset so connect()'s guard passes; the blackhole stays up.
                _state.value = WarrenTunnelState.Disconnected
            }
            if (!userInitiatedDisconnect) connectLocked(config, mnemonic)
        }
    }

    private fun registerNetworkCallback() {
        if (networkCallback != null) return
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if (network == datapathNetwork) return
                val baseline = !datapathNetworkSeen
                datapathNetworkSeen = true
                datapathNetwork = network
                // The network the session already dials on is not a change.
                if (!baseline) notifyMigrationWatchdog("underlying network changed")
            }

            override fun onLost(network: Network) {
                if (network != datapathNetwork) return
                datapathNetwork = null
                // The system is free to report the loss BEFORE the replacement's
                // onAvailable, and reading that replacement as a fresh session
                // baseline would skip the migration silently. Losing the network
                // that carries the datapath IS the change.
                notifyMigrationWatchdog("underlying network lost")
            }
        }
        if (platform.registerNetworkCallback(callback)) networkCallback = callback
    }

    /**
     * Hand a change of the network carrying the datapath to the native
     * migration watchdog, which rebinds the live QUIC endpoint onto a fresh
     * protected socket and revalidates the path in about one RTT. Nothing is
     * torn down here: the live TUN keeps holding the routes for the whole
     * migration, so there is no window to protect with a blackhole, and
     * [scheduleHandoverReconnect] runs only if the watchdog gives up.
     *
     * Notifying once too often costs one path probe the watchdog then declines
     * to act on; not notifying costs the whole migration, so a doubtful event
     * is notified.
     */
    private fun notifyMigrationWatchdog(reason: String) {
        Logger.i(
            "WarrenQuinnAdapter: $reason; handing the handover to the migration watchdog"
        )
        handoverNotified = true
        platform.notifyNetworkChanged()
    }

    private fun unregisterNetworkCallback() {
        val cb = networkCallback ?: return
        platform.unregisterNetworkCallback(cb)
        networkCallback = null
    }

    /**
     * Handover fallback: tear down the current Quinn session and re-issue
     * `connectTunnel` with the cached config + mnemonic. Reached only when the
     * native migration watchdog escalated, which means it already spent its
     * whole cascade (about 3 s of path probing, then up to 30 s of forced
     * redials) on the network we moved onto, so the grace here is short
     * ([HANDOVER_FALLBACK_GRACE_MS]): the settling time the old 15 s bought is
     * long gone by the time this runs.
     *
     * The call ORDER below is the leak-critical part and is pinned by
     * `WarrenQuinnAdapterTest.fallback poses blackhole before closing the tun`:
     * a TUN always holds the routes, and `activeFd` is closed only once the
     * blocking one is established.
     */
    private fun scheduleHandoverReconnect() {
        val config = activeConfig
        val mnemonic = activeMnemonic
        if (config == null || mnemonic == null) {
            Logger.w("scheduleHandoverReconnect: no active session to reconnect")
            return
        }
        pendingHandover?.cancel()
        pendingHandover = scope.launch {
            autoRecovery.armAutomation()
            // All shared-state mutation and the native teardown run under `lock`,
            // the same discipline as scheduleDropReconnect, so a concurrent
            // connect/disconnect/onSessionDown cannot interleave with this
            // handover (which previously mutated fds and _state off-lock).
            lock.withLock {
                if (userInitiatedDisconnect) return@withLock
                _state.value = reconnectingFrom(config)
                // Establish the blackhole BEFORE tearing the tunnel down so
                // traffic stays captured across the reconnect gap: establish()
                // atomically replaces the live interface, so there is never a
                // window without a TUN. The blackhole is torn down by
                // connect() -> exitBlockingMode() on success.
                if (blockingFd == null) {
                    val fd = platform.establish(planTunInterface(config, blocking = true))
                    if (fd != null) {
                        blockingFd = fd
                    } else {
                        Logger.w("scheduleHandoverReconnect: blackhole establish failed; brief leak possible")
                    }
                }
                // Stop the watch before the intentional teardown so the status
                // loop does not observe the DISCONNECTED transition and trip the
                // kill switch (this is an expected handover, not a drop).
                statusWatchJob?.cancel()
                statusWatchJob = null
                platform.disconnectTunnel()
                activeFd?.close()
                activeFd = null
                _state.value = WarrenTunnelState.Disconnected
            }
            delay(HANDOVER_FALLBACK_GRACE_MS)
            awaitDialableNetwork()
            // Re-check intent after the grace period: a user disconnect during
            // the wait cancels this job (teardownLocked cancels pendingHandover)
            // and flips the flag, so we must not reconnect over a user teardown.
            if (!userInitiatedDisconnect) connectLocked(config, mnemonic)
        }
    }

    /**
     * Park the retry until the device has a network a relay dial can
     * actually use (IPv4-bearing, see [canDialRelay]). Prevents the retry
     * loop from burning dial attempts (and flap-detector budget) while the
     * device is offline, and resumes promptly on the online edge. Mirrors
     * the desktop `Error(IsOffline)` family-gated auto-reconnect.
     */
    private suspend fun awaitDialableNetwork() {
        if (connectivity.value.canDialRelay()) return
        Logger.i(
            "WarrenQuinnAdapter: no dialable network (offline or IPv6-only); " +
                "waiting for the online edge before reconnecting"
        )
        connectivity.first { it.canDialRelay() }
        Logger.i("WarrenQuinnAdapter: dialable network is back; reconnecting")
    }

    // The dial in flight, described by the config it is dialling: the UI names
    // the exit and raises its chips from the first frame instead of guessing a
    // catalogue relay and correcting itself once the tunnel is up.
    private fun connectingFrom(config: WarrenTunnelConfig) =
        WarrenTunnelState.Connecting(
            exitEndpointHost = config.exitEndpoint,
            entryEndpointHost = config.entryHop?.relayEndpoint,
            multiHop = config.entryHop != null && config.multihopTwoHop,
            daita = config.daita != null,
        )

    private fun reconnectingFrom(config: WarrenTunnelConfig) =
        WarrenTunnelState.Reconnecting(
            exitEndpointHost = config.exitEndpoint,
            entryEndpointHost = config.entryHop?.relayEndpoint,
            multiHop = config.entryHop != null && config.multihopTwoHop,
            daita = config.daita != null,
        )

    private fun statusFromCode(code: Int, config: WarrenTunnelConfig): WarrenTunnelState =
        when (code) {
            STATUS_DISCONNECTED -> WarrenTunnelState.Disconnected
            STATUS_CONNECTING -> connectingFrom(config)
            STATUS_CONNECTED ->
                WarrenTunnelState.Connected(
                    exitId = config.exitPubkeyHex,
                    assignedNatPmpPort = null,
                    // Honest topology: entry_hop is always present (the tunnel
                    // rides the multi-hop wire), but a 1-hop circuit
                    // (multihopTwoHop == false) collapses onto one node, so it
                    // must NOT advertise the multi-hop indicator.
                    multiHop = config.entryHop != null && config.multihopTwoHop,
                    daita = config.daita != null,
                    exitEndpointHost = config.exitEndpoint,
                    entryEndpointHost = config.entryHop?.relayEndpoint,
                )
            STATUS_RECONNECTING -> reconnectingFrom(config)
            STATUS_UNAUTHORIZED -> WarrenTunnelState.Failed("subscription expired", expired = true)
            else -> WarrenTunnelState.Failed("native status code $code")
        }

    private companion object {
        const val STATUS_DISCONNECTED = 0
        const val STATUS_CONNECTING = 1
        const val STATUS_CONNECTED = 2
        const val STATUS_RECONNECTING = 3

        // The exit refused the setup (not authorized: lapsed / revoked
        // subscription). Mirrors `warren_jni::tunnel::SessionStatus::Unauthorized`.
        const val STATUS_UNAUTHORIZED = 4

        /**
         * Ceiling on one wait for a native status wake. The engine wakes the
         * watch on every change, so this only bounds the damage of a wake that
         * was never delivered: long enough that an idle session wakes about
         * once a second instead of four times, short enough that a lost
         * transition still reaches the card within a blink.
         */
        const val STATUS_WAKE_FALLBACK_MS = 1_000L

        /**
         * Ceiling on the wait for a torn-down session to finish. Generous
         * against a slow close, short enough that a stuck session delays a
         * reconnect by a noticeable blink rather than stranding it.
         */
        const val SESSION_CLOSE_TIMEOUT_MS = 3_000L

        /**
         * Grace for the exit to process the close and release the sticky inner
         * IP before the next session registers. Comfortably above the
         * client-to-exit RTT, and invisible next to the 21 s black-hole plus
         * redial it prevents.
         */
        const val PEER_CLOSE_SETTLE_MS = 400L
        const val NATPMP_IDLE = "{\"state\":\"idle\"}"

        /**
         * Retry delay after an unexpected drop, mirroring warren-core
         * `Backoff::HANDSHAKE` so the new handshake aligns with the exit's
         * expected re-handshake window. Unrelated to a handover: nothing has
         * probed the network before this wait.
         */
        const val DROP_RETRY_GRACE_MS = 15_000L

        /**
         * Retry delay on the handover fallback. The native migration watchdog
         * has already spent its 3 s probe window plus up to 30 s of forced
         * redials on the new network before it escalates here, so this only has
         * to cover the interface teardown itself; `awaitDialableNetwork()`
         * still parks the dial until the device has a v4-bearing network.
         */
        const val HANDOVER_FALLBACK_GRACE_MS = 2_000L
    }
}
