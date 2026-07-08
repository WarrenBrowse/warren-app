package com.warrenbrowse.vpn.app.service

import android.content.Intent
import android.net.ConnectivityManager
import android.os.Binder
import android.os.Build
import android.os.IBinder
import androidx.core.content.getSystemService
import androidx.lifecycle.lifecycleScope
import arrow.atomic.AtomicInt
import co.touchlab.kermit.Logger
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.connect.WarrenConnectUseCase
import com.warrenbrowse.vpn.app.connectivity.WarrenConnectivityMonitor
import com.warrenbrowse.vpn.app.service.migration.MigrateSplitTunneling
import com.warrenbrowse.vpn.app.service.notifications.ForegroundNotificationManager
import com.warrenbrowse.vpn.di.vpnServiceModule
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.common.constant.KEY_CONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_CONNECT_QUINN_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_TUNNEL_CONFIG_JSON
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import com.warrenbrowse.vpn.lib.repository.MnemonicCache
import com.warrenbrowse.talpid.LifecycleVpnService
import org.koin.android.ext.android.getKoin
import org.koin.core.context.loadKoinModules

// The tunnel lifecycle is managed by `quinnAdapter`: `WarrenQuinnAdapter`
// builds and establishes the VpnService.Builder itself and owns its own
// ConnectivityManager handover hook.
class WarrenVpnService : LifecycleVpnService() {

    private lateinit var warrenConnectUseCase: WarrenConnectUseCase

    private lateinit var migrateSplitTunneling: MigrateSplitTunneling
    private lateinit var apiEndpointFromIntentHolder: ApiEndpointFromIntentHolder

    private lateinit var foregroundNotificationHandler: ForegroundNotificationManager

    /**
     * Warren Quinn adapter owns the entire tunnel lifecycle. Constructed
     * in `onCreate` (it holds the [android.net.VpnService] reference and
     * the [android.net.ConnectivityManager] for handover reconnect, both
     * of which are unique to this service instance).
     *
     * This adapter is the only tunnel-state authority.
     */
    lateinit var quinnAdapter: WarrenQuinnAdapter
        private set

    private lateinit var quinnStateProxy: WarrenQuinnStateProxy

    private lateinit var connectivityMonitor: WarrenConnectivityMonitor

    // Count number of binds to know if the service is needed. If user actively using the VPN, a
    // bind from the system, should always be present.
    private val bindCount = AtomicInt()

    override fun onCreate() {
        super.onCreate()
        Logger.i("WarrenVpnService: onCreate")

        loadKoinModules(listOf(vpnServiceModule))
        with(getKoin()) {
            // Needed to create all the notification channels
            get<NotificationChannelFactory>()

            foregroundNotificationHandler =
                ForegroundNotificationManager(this@WarrenVpnService, get())
            get<NotificationManager>()

            migrateSplitTunneling = get()
            apiEndpointFromIntentHolder = get()
            quinnStateProxy = get()
            warrenConnectUseCase = get()
            connectivityMonitor = get()
        }

        // While this service is alive its VpnService.protect() backs the
        // monitor's underlying-network probe (the only truthful way to read
        // connectivity from under our own TUN). Cleared in onDestroy.
        connectivityMonitor.registerSocketProtector { socket -> protect(socket) }

        // Quinn adapter must outlive every connect/disconnect cycle, so
        // it is owned by the service and kept alive for the service's
        // lifetime. The adapter's internal `SupervisorJob` is cancelled
        // from `onDestroy` below.
        quinnAdapter = WarrenQuinnAdapter(
            vpnService = this,
            connectivityManager = getSystemService<ConnectivityManager>()!!,
            settings = getKoin().get(),
            connectivity = connectivityMonitor.connectivity,
        )

        // Observe Quinn tunnel transitions:
        //   1. mirror every state into the process-wide proxy so any
        //      Composable can read it without binding the service;
        //   2. stop the foreground notification on non-blocking failure
        //      (mirrors the legacy `managementService.tunnelState`
        //      collector below, which targets a daemon that does not
        //      exist on Warren mobile).
        lifecycleScope.launch {
            quinnAdapter.natPmpStatus.collect { status ->
                quinnStateProxy.updateNatPmpStatus(status)
            }
        }
        lifecycleScope.launch {
            quinnAdapter.autoRecoveryCount.collect { count ->
                quinnStateProxy.updateAutoRecoveryCount(count)
            }
        }
        lifecycleScope.launch {
            quinnAdapter.state.collect { state ->
                quinnStateProxy.update(state)
                when (state) {
                    is WarrenTunnelState.Failed -> {
                        Logger.w("Quinn tunnel failed: ${state.reason}")
                        foregroundNotificationHandler.stopForeground()
                    }
                    is WarrenTunnelState.Blocking -> {
                        // Kill switch engaged: keep the service in the
                        // foreground so the blackhole interface stays up
                        // and traffic cannot leak.
                        Logger.w("Quinn tunnel blocking (lockdown): ${state.reason}")
                    }
                    else -> Unit
                }
            }
        }

        // Warren fetches relay lists via warren-api-client at runtime, so the
        // upstream `relays.json` asset extraction (`prepareFiles()`) is gone.
        migrateSplitTunneling.migrate()

        // Log any API endpoint override seeded by mockapi tests so the
        // future warren-api-client can pick it up.
        val intentApiOverride = apiEndpointFromIntentHolder.apiEndpointOverride
        if (BuildConfig.DEBUG && intentApiOverride != null) {
            Logger.i("API endpoint override present: $intentApiOverride")
        }

        WarrenJni.initLogger(filesDir.absolutePath)
        Logger.i("warren-jni initialised")

        // The Quinn adapter owns the tunnel lifecycle end-to-end. The
        // components resolved from Koin here are the notification channel
        // factory, foreground notification manager, split-tunnelling migration
        // shim, and the API-endpoint intent holder.
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Logger.i(
            "onStartCommand (intent=$intent, action=${intent?.action}, flags=$flags, startId=$startId)"
        )

        val startResult = super.onStartCommand(intent, flags, startId)

        // Always promote to foreground if connect/disconnect actions are provided to mitigate cases
        // where the service would potentially otherwise be too slow running `startForeground`.
        when {
            intent.isFromSystem() || intent?.action == KEY_CONNECT_ACTION -> {
                // Boot auto-start (KEY_CONNECT_ACTION from the boot receiver or a
                // notification Connect button) and system always-on (isFromSystem)
                // both arrive without a config. Run the full connect orchestration,
                // which resolves the wallet + config, stages the mnemonic, and
                // dispatches KEY_WARREN_CONNECT_QUINN_ACTION back to this service.
                // The mnemonic read is silent (app-bound key), so this works
                // headless after boot even while the keyguard is locked.
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch {
                    val outcome = warrenConnectUseCase.invoke(applicationContext)
                    if (outcome !is WarrenConnectUseCase.Outcome.Success) {
                        Logger.w("auto-start/always-on connect did not dispatch: $outcome")
                    }
                }
            }

            intent?.action == KEY_RECONNECT_ACTION -> {
                // Reconnect routes through the Quinn adapter, which reuses the
                // cached config + mnemonic (no biometric re-prompt). Does
                // nothing if there is no active session.
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch { quinnAdapter.reconnect() }
            }

            intent?.action == KEY_WARREN_CONNECT_QUINN_ACTION -> {
                // Dedicated Quinn connect path. The caller has already
                // retrieved the mnemonic via the UI-side
                // BiometricPromptAuthorizer, stashed it in MnemonicCache, and
                // serialised a WarrenTunnelConfig in the extras.
                foregroundNotificationHandler.startForeground()
                val configJson = intent.getStringExtra(KEY_WARREN_TUNNEL_CONFIG_JSON)
                val mnemonic = MnemonicCache.consume()
                when {
                    configJson == null -> {
                        Logger.e("$KEY_WARREN_CONNECT_QUINN_ACTION missing config JSON extra")
                    }
                    mnemonic == null -> {
                        Logger.e(
                            "$KEY_WARREN_CONNECT_QUINN_ACTION fired without a staged mnemonic " +
                                "(UI must call MnemonicCache.put() before startService)"
                        )
                    }
                    else -> {
                        val config = try {
                            warrenTunnelConfigFromWireJson(configJson)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "Failed to deserialise WarrenTunnelConfig" }
                            null
                        }
                        if (config != null) {
                            lifecycleScope.launch {
                                // Ownership of the mnemonic transfers to the
                                // adapter, which holds it as a zeroizable
                                // Mnemonic for the session's reconnect path and
                                // wipes it via close() on teardown.
                                quinnAdapter.connect(config, mnemonic)
                            }
                        } else {
                            // No connect launched -> close eagerly so
                            // the CharArray isn't held past the dispatch.
                            mnemonic.close()
                        }
                    }
                }
            }

            intent?.action == KEY_DISCONNECT_ACTION -> {
                // WarrenTileService might have launched this service with the expectancy of it
                // being foreground, thus it must go into foreground to please the android system
                // requirements.
                foregroundNotificationHandler.startForeground()
                // Disconnect always routes through the Quinn adapter
                // now. If no session is running, this is a no-op
                // (Mutex + state-machine guard inside the adapter).
                lifecycleScope.launch {
                    quinnAdapter.disconnect()
                }

                // If disconnect intent is received and no one is using this service, simply stop
                // foreground and let system stop service when it deems it not to be necessary.
                if (bindCount.get() == 0) {
                    foregroundNotificationHandler.stopForeground()
                }
            }
        }

        return startResult
    }

    override fun onBind(intent: Intent?): IBinder {
        val count = bindCount.incrementAndGet()
        Logger.i("onBind: $intent, bindCount: $count")

        if (intent.isFromSystem()) {
            Logger.i("onBind was from system")
            foregroundNotificationHandler.startForeground()
        }

        // We always need to return a binder. If the system binds to our VPN service, VpnService
        // will return a binder that shall be user, otherwise we return an empty dummy binder to
        // keep connection service alive since the actual communication happens over gRPC.
        return super.onBind(intent) ?: emptyBinder()
    }

    override fun onRebind(intent: Intent?) {
        super.onRebind(intent)
        val count = bindCount.incrementAndGet()
        Logger.i("onRebind: $intent, bindCount: $count")

        if (intent.isFromSystem()) {
            Logger.i("onRebind from system")
            foregroundNotificationHandler.startForeground()
        }
    }

    private fun emptyBinder() =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            Binder(this.toString())
        } else {
            Binder()
        }

    override fun onRevoke() {
        Logger.d("onRevoke")
        // Bring the Quinn session down cleanly on system revoke.
        runBlocking { quinnAdapter.disconnect() }
    }

    override fun onUnbind(intent: Intent): Boolean {
        val count = bindCount.decrementAndGet()
        Logger.i("onUnbind: $intent, bindCount: $count")

        // Foreground?
        if (intent.isFromSystem()) {
            Logger.i("onUnbind from system")
            foregroundNotificationHandler.stopForeground()
        }

        return true
    }

    override fun onDestroy() {
        super.onDestroy()
        Logger.i("WarrenVpnService: onDestroy")

        connectivityMonitor.clearSocketProtector()

        // `disconnect()` is idempotent (state-machine guards a no-op if
        // already disconnected), so it is safe even if no session was
        // ever started.
        runBlocking { quinnAdapter.disconnect() }

        Logger.i("Shutdown complete")
    }

    // If an intent is from the system it is because of the OS starting/stopping the VPN.
    private fun Intent?.isFromSystem(): Boolean {
        return this?.action == SERVICE_INTERFACE
    }

    companion object {
        init {
            System.loadLibrary("warren_jni")
        }
    }
}
