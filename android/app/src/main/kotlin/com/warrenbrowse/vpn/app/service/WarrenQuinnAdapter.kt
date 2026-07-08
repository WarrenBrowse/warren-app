package com.warrenbrowse.vpn.app.service

import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.content.pm.PackageManager
import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import java.io.IOException
import co.touchlab.kermit.Logger
import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.vpn.app.connectivity.canDialRelay
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
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

// Owns the Warren Quinn tunnel lifecycle for a given `WarrenVpnService`
// instance. Sits between the Android `VpnService` API and the Rust JNI:
//
//   WarrenVpnService -- onStartCommand --> WarrenQuinnAdapter.connect(config)
//                                                |
//                                                v
//                                       VpnService.Builder.establish()
//                                                |
//                                                v
//                                  WarrenJni.connectTunnel(fd, configJson)
//                                                |
//                                                v
//                                  warren_tunnel Quinn pump
//
// State machine + reconnect-on-network-change live here, not in Rust.
//
// TODO: the builder configuration (DNS, bypass CIDRs, multi-hop entry
// plumbing) is not wired yet. See `.planning/session-d-d4-d7-design.md`.
class WarrenQuinnAdapter(
    private val vpnService: VpnService,
    private val connectivityManager: ConnectivityManager,
    private val settings: WarrenLocalSettingsRepository,
    private val connectivity: StateFlow<Connectivity>,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()

    private val _state = MutableStateFlow<WarrenTunnelState>(WarrenTunnelState.Disconnected)
    val state: StateFlow<WarrenTunnelState> = _state.asStateFlow()

    // Live NAT-PMP port-forwarding status (raw JSON from the Rust side),
    // polled alongside the tunnel status. `idle` when no mapping is active.
    private val _natPmpStatus = MutableStateFlow(NATPMP_IDLE)
    val natPmpStatus: StateFlow<String> = _natPmpStatus.asStateFlow()

    // Total automatic recoveries since process start: the Rust redial
    // engine's in-session redials (WarrenJni.getAutoRecoveryCount) plus the
    // adapter's own retry-loop successes (AutoRecoveryTracker). Mirrors the
    // desktop reconnect_count row; user actions never count.
    private val autoRecovery = AutoRecoveryTracker()
    private val _autoRecoveryCount = MutableStateFlow(0)
    val autoRecoveryCount: StateFlow<Int> = _autoRecoveryCount.asStateFlow()

    private var activeConfig: WarrenTunnelConfig? = null
    // Held as a zeroizable [Mnemonic] (CharArray-backed), NOT a String: the
    // recovery phrase must not linger as a long-lived immutable String on the
    // JVM heap for the whole session (a heap dump would extract it verbatim).
    // The adapter owns this instance and wipes it via close() on teardown.
    // Do NOT switch this back to a String, it would defeat zeroization.
    private var activeMnemonic: Mnemonic? = null
    private var activeFd: ParcelFileDescriptor? = null
    private var statusPollJob: Job? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var lastNetwork: Network? = null
    private var pendingHandover: Job? = null

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
    suspend fun connect(config: WarrenTunnelConfig, mnemonic: Mnemonic) = lock.withLock {
        if (_state.value !is WarrenTunnelState.Disconnected) {
            Logger.w("WarrenQuinnAdapter: connect() called while not disconnected")
            // Never zero the session's own mnemonic; only wipe a foreign one
            // that we are refusing to take ownership of.
            if (mnemonic !== activeMnemonic) mnemonic.close()
            return@withLock
        }
        _state.value = WarrenTunnelState.Connecting
        userInitiatedDisconnect = false
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
        WarrenJni.disconnectTunnel()

        val fd = buildTunInterface(config) ?: run {
            onSessionDown(config, "VpnService.Builder.establish() returned null")
            return@withLock
        }
        // Retain a dup of the TUN fd so the VPN interface stays established even
        // after the native side closes its copy on an unexpected session death:
        // traffic keeps entering the now-unread TUN (blocked, not leaked) until
        // the status poll installs the real blackhole. Without this the interface
        // tears down the instant Rust drops the fd, leaking clear traffic on the
        // physical link for up to one poll interval. The fail-closed logic below
        // relies on activeFd being a LIVE handle to the interface.
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
                WarrenJni.connectTunnel(
                    vpnService,
                    fd.detachFd(),
                    phrase,
                    config.toWireJson(),
                )
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

        // Poll the Rust-side session status atomic and translate transitions
        // back to `WarrenTunnelState`. A JNI callback channel would avoid the
        // busy-poll but requires JVM ref management gymnastics. The poll
        // cadence (250 ms) is fast enough to keep the UI responsive without
        // burning battery: the Rust side updates the atomic only once per
        // transition.
        val sessionConfig = config
        statusPollJob = scope.launch {
            var lastSeen = STATUS_CONNECTING
            while (isActive) {
                val code = WarrenJni.getTunnelStatus()
                if (code != lastSeen) {
                    lastSeen = code
                    if (code == STATUS_DISCONNECTED || code < 0) {
                        // Rust task ended (clean exit or error). Engage the
                        // kill switch on an unexpected drop under lockdown;
                        // otherwise release and stop polling.
                        val reason =
                            if (code < 0) "native status code $code" else "tunnel disconnected"
                        lock.withLock {
                            if (userInitiatedDisconnect) {
                                _state.value = WarrenTunnelState.Disconnected
                            } else {
                                onSessionDown(sessionConfig, reason)
                            }
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
                        flapDetector.reset()
                        if (autoRecovery.onConnected()) {
                            Logger.i("WarrenQuinnAdapter: automatic recovery landed")
                        }
                    }
                    _state.value = statusFromCode(code, sessionConfig)
                }
                // Mirror the live NAT-PMP status (cheap static read).
                val np = WarrenJni.getNatPmpStatus()
                if (np != _natPmpStatus.value) _natPmpStatus.value = np
                // Sum the native in-session redials with the adapter's own
                // retry-loop recoveries. Polled (not event-driven) because a
                // sub-poll-interval native redial has no observable status
                // transition on this side.
                val recoveries = WarrenJni.getAutoRecoveryCount() + autoRecovery.count
                if (recoveries != _autoRecoveryCount.value) {
                    _autoRecoveryCount.value = recoveries
                }
                delay(STATUS_POLL_INTERVAL_MS)
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
    suspend fun reconnect() {
        val config = activeConfig
        val mnemonic = activeMnemonic
        if (config == null || mnemonic == null) {
            Logger.w("WarrenQuinnAdapter: reconnect() called without an active session")
            return
        }
        lock.withLock { teardownLocked() }
        // Reuse the same owned Mnemonic instance (connect() detects identity
        // and does not re-store or wipe it).
        connect(config, mnemonic)
    }

    suspend fun disconnect() = lock.withLock {
        teardownLocked()
        // User teardown is terminal: wipe the cached mnemonic.
        activeMnemonic?.close()
        activeMnemonic = null
    }

    /**
     * Tear down the active session (cancel polling, drop the JNI tunnel and
     * TUN fd, clear the blackhole) and return to [WarrenTunnelState.Disconnected].
     * Must be called holding [lock]. Does NOT wipe [activeMnemonic] so the
     * reconnect path can reuse it; the public [disconnect] wipes it after.
     */
    private fun teardownLocked() {
        // Intentional teardown: release traffic instead of engaging the
        // kill switch.
        userInitiatedDisconnect = true
        flapDetector.reset()
        // A user action clears any pending recovery attribution: whatever
        // connects next is not an automatic recovery.
        autoRecovery.onUserAction()
        unregisterNetworkCallback()
        pendingHandover?.cancel()
        pendingHandover = null
        WarrenJni.disconnectTunnel()
        statusPollJob?.cancel()
        statusPollJob = null
        activeFd?.close()
        activeFd = null
        exitBlockingMode()
        activeConfig = null
        lastNetwork = null
        _natPmpStatus.value = NATPMP_IDLE
        _state.value = WarrenTunnelState.Disconnected
    }

    private fun buildTunInterface(config: WarrenTunnelConfig): ParcelFileDescriptor? =
        applyPlan(planTunInterface(config, excludedApps = currentExcludedApps()))

    /**
     * Apply a [WarrenTunInterfacePlan] to a fresh `VpnService.Builder` and
     * establish the interface. Invalid routes / DNS servers are skipped
     * rather than aborting the whole tunnel.
     */
    private fun applyPlan(plan: WarrenTunInterfacePlan): ParcelFileDescriptor? {
        val builder = vpnService.Builder()
            .setSession(plan.session)
            .setMtu(plan.mtu)
        plan.addresses.forEach {
            try {
                builder.addAddress(it.address, it.prefixLength)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid address ${it.address}/${it.prefixLength}" }
            }
        }
        plan.routes.forEach {
            try {
                builder.addRoute(it.address, it.prefixLength)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid route ${it.address}/${it.prefixLength}" }
            }
        }
        plan.dnsServers.forEach {
            try {
                builder.addDnsServer(it)
            } catch (e: IllegalArgumentException) {
                Logger.w(throwable = e) { "skipping invalid DNS server $it" }
            }
        }
        // Split tunnelling: route excluded apps outside the tunnel. A package
        // that is no longer installed throws NameNotFoundException; skip it
        // rather than abort the whole interface. Never on a blocking plan (its
        // excludedApps is empty), so the kill switch always captures everything.
        plan.excludedApps.forEach { pkg ->
            try {
                builder.addDisallowedApplication(pkg)
            } catch (e: PackageManager.NameNotFoundException) {
                Logger.w(throwable = e) { "skipping excluded app not installed: $pkg" }
            }
        }
        // establish() validates the full config in the system process and
        // throws IllegalArgumentException ("Cannot set address") on a rejected
        // combination (e.g. an IPv6 address with MTU < the 1280 v6 minimum).
        // Fail closed by returning null - the caller routes that into
        // onSessionDown - instead of letting it crash the VpnService.
        return try {
            builder.establish()
        } catch (e: IllegalArgumentException) {
            Logger.e(throwable = e) { "VpnService.Builder.establish() rejected the plan" }
            null
        }
    }

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
            val fd = applyPlan(planTunInterface(config, blocking = true))
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
            delay(HANDOVER_GRACE_MS)
            awaitDialableNetwork()
            lock.withLock {
                if (userInitiatedDisconnect) return@withLock
                statusPollJob?.cancel()
                statusPollJob = null
                // Reset so connect()'s guard passes; the blackhole stays up.
                _state.value = WarrenTunnelState.Disconnected
            }
            if (!userInitiatedDisconnect) connect(config, mnemonic)
        }
    }

    private fun registerNetworkCallback() {
        if (networkCallback != null) return
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if (lastNetwork == null) {
                    // First seen network during the session is the
                    // baseline; not a handover.
                    lastNetwork = network
                    return
                }
                if (network != lastNetwork) {
                    Logger.i(
                        "WarrenQuinnAdapter: underlying network changed " +
                            "($lastNetwork -> $network), scheduling reconnect"
                    )
                    lastNetwork = network
                    scheduleHandoverReconnect()
                }
            }

            override fun onLost(network: Network) {
                if (network == lastNetwork) {
                    Logger.i("WarrenQuinnAdapter: underlying network $network lost")
                    lastNetwork = null
                }
            }
        }
        try {
            connectivityManager.registerNetworkCallback(request, callback)
            networkCallback = callback
        } catch (e: SecurityException) {
            Logger.w(throwable = e) {
                "registerNetworkCallback denied (missing permission?); handover reconnect disabled"
            }
        }
    }

    private fun unregisterNetworkCallback() {
        val cb = networkCallback ?: return
        try {
            connectivityManager.unregisterNetworkCallback(cb)
        } catch (e: IllegalArgumentException) {
            Logger.w(throwable = e) {
                "unregisterNetworkCallback failed (callback was not registered)"
            }
        }
        networkCallback = null
    }

    /**
     * Tear down the current Quinn session and re-issue `connectTunnel`
     * with the cached config + mnemonic after a brief grace period. The
     * 15 s wait mirrors warren-core `Backoff::HANDSHAKE` so the new
     * handshake aligns with the exit's expected re-handshake window.
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
                _state.value = WarrenTunnelState.Reconnecting
                // Establish the blackhole BEFORE tearing the tunnel down so
                // traffic stays captured across the reconnect gap: establish()
                // atomically replaces the live interface, so there is never a
                // window without a TUN. The blackhole is torn down by
                // connect() -> exitBlockingMode() on success.
                if (blockingFd == null) {
                    val fd = applyPlan(planTunInterface(config, blocking = true))
                    if (fd != null) {
                        blockingFd = fd
                    } else {
                        Logger.w("scheduleHandoverReconnect: blackhole establish failed; brief leak possible")
                    }
                }
                // Stop polling before the intentional teardown so the status
                // loop does not observe the DISCONNECTED transition and trip the
                // kill switch (this is an expected handover, not a drop).
                statusPollJob?.cancel()
                statusPollJob = null
                WarrenJni.disconnectTunnel()
                activeFd?.close()
                activeFd = null
                _state.value = WarrenTunnelState.Disconnected
            }
            delay(HANDOVER_GRACE_MS)
            awaitDialableNetwork()
            // Re-check intent after the grace period: a user disconnect during
            // the wait cancels this job (teardownLocked cancels pendingHandover)
            // and flips the flag, so we must not reconnect over a user teardown.
            if (!userInitiatedDisconnect) connect(config, mnemonic)
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

    private fun statusFromCode(code: Int, config: WarrenTunnelConfig): WarrenTunnelState =
        when (code) {
            STATUS_DISCONNECTED -> WarrenTunnelState.Disconnected
            STATUS_CONNECTING -> WarrenTunnelState.Connecting
            STATUS_CONNECTED ->
                WarrenTunnelState.Connected(
                    exitId = config.exitPubkeyHex,
                    assignedNatPmpPort = null,
                    multiHop = config.entryHop != null,
                    daita = config.daita != null,
                    exitEndpointHost = config.exitEndpoint,
                    entryEndpointHost = config.entryHop?.relayEndpoint,
                )
            STATUS_RECONNECTING -> WarrenTunnelState.Reconnecting
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
        const val STATUS_POLL_INTERVAL_MS = 250L
        const val NATPMP_IDLE = "{\"state\":\"idle\"}"

        /** Backoff::HANDSHAKE = 15 s. */
        const val HANDOVER_GRACE_MS = 15_000L
    }
}
