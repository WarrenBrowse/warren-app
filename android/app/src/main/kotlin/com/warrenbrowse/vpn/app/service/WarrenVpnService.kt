package com.warrenbrowse.vpn.app.service

import android.app.KeyguardManager
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
import kotlinx.serialization.json.Json
import com.warrenbrowse.vpn.BuildConfig
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

// D.4 step 56: WarrenVpnService now extends LifecycleVpnService directly.
// The old TalpidVpnService superclass was Mullvad daemon JNI glue (openTun /
// closeTun / bypass / connectivityListener marshalling) — entirely dead on
// Warren since the tunnel lifecycle is managed by `quinnAdapter` :
// `WarrenQuinnAdapter` builds + establishes the VpnService.Builder itself,
// owns its own ConnectivityManager handover hook, and never calls back into
// the Mullvad daemon JNI.
class WarrenVpnService : LifecycleVpnService() {

    private lateinit var keyguardManager: KeyguardManager

    private lateinit var migrateSplitTunneling: MigrateSplitTunneling
    private lateinit var apiEndpointFromIntentHolder: ApiEndpointFromIntentHolder

    private lateinit var foregroundNotificationHandler: ForegroundNotificationManager

    /**
     * Warren Quinn adapter owns the entire tunnel lifecycle. Constructed
     * in `onCreate` (it holds the [android.net.VpnService] reference and
     * the [android.net.ConnectivityManager] for handover reconnect, both
     * of which are unique to this service instance).
     *
     * D.4 step 10: the legacy gRPC management service / connection
     * proxy layer has been fully removed; this adapter is now the only
     * tunnel-state authority.
     */
    lateinit var quinnAdapter: WarrenQuinnAdapter
        private set

    private lateinit var quinnStateProxy: WarrenQuinnStateProxy

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
        }

        keyguardManager = getSystemService<KeyguardManager>()!!

        // Quinn adapter must outlive every connect/disconnect cycle, so
        // it is owned by the service and kept alive for the service's
        // lifetime. The adapter's internal `SupervisorJob` is cancelled
        // from `onDestroy` below.
        quinnAdapter = WarrenQuinnAdapter(
            vpnService = this,
            connectivityManager = getSystemService<ConnectivityManager>()!!,
        )

        // Observe Quinn tunnel transitions:
        //   1. mirror every state into the process-wide proxy so any
        //      Composable can read it without binding the service;
        //   2. stop the foreground notification on non-blocking failure
        //      (mirrors the legacy `managementService.tunnelState`
        //      collector below, which targets a daemon that does not
        //      exist on Warren mobile).
        lifecycleScope.launch {
            quinnAdapter.state.collect { state ->
                quinnStateProxy.update(state)
                if (state is WarrenTunnelState.Failed) {
                    Logger.w("Quinn tunnel failed: ${state.reason}")
                    foregroundNotificationHandler.stopForeground()
                }
            }
        }

        // Warren fetches relay lists via warren-api-client at runtime, so the
        // upstream `relays.json` asset extraction (`prepareFiles()`) is gone.
        migrateSplitTunneling.migrate()

        // Log any API endpoint override seeded by mockapi tests so the
        // future warren-api-client can pick it up (D.6 wiring).
        val intentApiOverride = apiEndpointFromIntentHolder.apiEndpointOverride
        if (BuildConfig.DEBUG && intentApiOverride != null) {
            Logger.i("API endpoint override present: $intentApiOverride")
        }

        WarrenJni.initLogger(filesDir.absolutePath)
        Logger.i("warren-jni initialised")

        // D.4 step 10: the gRPC management service / connection proxy
        // pathway has been entirely removed. The Quinn adapter now owns
        // the tunnel lifecycle end-to-end; the only legacy components
        // still resolved from Koin are the notification channel
        // factory, foreground notification manager, split-tunnelling
        // migration shim, and the API-endpoint intent holder (all of
        // which are warren-native or trivial).
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Logger.i(
            "onStartCommand (intent=$intent, action=${intent?.action}, flags=$flags, startId=$startId)"
        )

        val startResult = super.onStartCommand(intent, flags, startId)

        // Always promote to foreground if connect/disconnect actions are provided to mitigate cases
        // where the service would potentially otherwise be too slow running `startForeground`.
        when {
            keyguardManager.isKeyguardLocked -> {
                Logger.i("Keyguard is locked, ignoring command")
            }

            intent.isFromSystem() || intent?.action == KEY_CONNECT_ACTION -> {
                // D.4 step 10: legacy KEY_CONNECT_ACTION no longer
                // dispatches to a daemon. Callers should migrate to
                // KEY_WARREN_CONNECT_QUINN_ACTION (with a serialised
                // WarrenTunnelConfig + a MnemonicCache.put preceding
                // it). For now, a system-initiated connect with no
                // config gets a no-op + log line; the legacy
                // ConnectionProxy is gone.
                foregroundNotificationHandler.startForeground()
                Logger.w(
                    "Received legacy KEY_CONNECT_ACTION (system or callsite); " +
                        "migrate to KEY_WARREN_CONNECT_QUINN_ACTION"
                )
            }

            intent?.action == KEY_RECONNECT_ACTION -> {
                // D.4 step 13: reconnect routes through the Quinn adapter,
                // which reuses the cached config + mnemonic (no biometric
                // re-prompt). No-op if there is no active session.
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch { quinnAdapter.reconnect() }
            }

            intent?.action == KEY_WARREN_CONNECT_QUINN_ACTION -> {
                // D.4 step 7: dedicated Quinn connect path. Caller has
                // already retrieved the mnemonic via the UI-side
                // BiometricPromptAuthorizer, stashed it in MnemonicCache,
                // and serialised a WarrenTunnelConfig in the extras.
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
                            Json.decodeFromString<WarrenTunnelConfig>(configJson)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "Failed to deserialise WarrenTunnelConfig" }
                            null
                        }
                        if (config != null) {
                            lifecycleScope.launch {
                                // `use { }` zeros the underlying CharArray
                                // after `connect` returns (whether it
                                // succeeded or threw). The adapter is
                                // responsible for any longer-lived
                                // mnemonic storage on its own side.
                                mnemonic.use { m ->
                                    quinnAdapter.connect(config, m.phrase)
                                }
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
