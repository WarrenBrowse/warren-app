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
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.service.migration.MigrateSplitTunneling
import com.warrenbrowse.vpn.app.service.notifications.ForegroundNotificationManager
import com.warrenbrowse.vpn.di.vpnServiceModule
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.common.constant.KEY_CONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.talpid.TalpidVpnService
import org.koin.android.ext.android.getKoin
import org.koin.core.context.loadKoinModules

class WarrenVpnService : TalpidVpnService() {

    private lateinit var keyguardManager: KeyguardManager

    private lateinit var managementService: ManagementService
    private lateinit var migrateSplitTunneling: MigrateSplitTunneling
    private lateinit var apiEndpointFromIntentHolder: ApiEndpointFromIntentHolder
    private lateinit var connectionProxy: ConnectionProxy

    private lateinit var foregroundNotificationHandler: ForegroundNotificationManager

    /**
     * D.4 step 6 partial: instantiate the Warren Quinn adapter once
     * during `onCreate` so the service owns the tunnel lifecycle. The
     * adapter holds the VpnService reference and the ConnectivityManager
     * needed for handover reconnect; both come from the service so we
     * cannot Koin-inject this one - we construct it in-place.
     *
     * The legacy [connectionProxy] / [managementService] state machine
     * is still resolved from Koin and `start()`-ed in a try/catch so the
     * service boots on emulator. D.4 step 7+ will remove that layer
     * entirely once the connect intent carries a [WarrenTunnelConfig].
     */
    lateinit var quinnAdapter: WarrenQuinnAdapter
        private set

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

            managementService = get()

            foregroundNotificationHandler =
                ForegroundNotificationManager(this@WarrenVpnService, get())
            get<NotificationManager>()

            migrateSplitTunneling = get()
            apiEndpointFromIntentHolder = get()
            connectionProxy = get()
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

        // Observe Quinn tunnel transitions and stop the foreground
        // notification on a non-blocking failure (mirrors the legacy
        // `managementService.tunnelState` collector below, which targets
        // a daemon that does not exist on Warren mobile).
        lifecycleScope.launch {
            quinnAdapter.state.collect { state ->
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

        // The gRPC management service targets a daemon that does not exist
        // on Warren mobile (the JNI library exposes `WarrenJni.connectTunnel`
        // direct). Calling `managementService.start()` here would block on
        // a non-existent UDS socket. We guard the start + tunnelState
        // collect behind a try/catch so the service still boots on
        // emulator while D.4 step 6 rewrites the lifecycle to use
        // `WarrenQuinnAdapter` direct.
        try {
            Logger.i("Start management service (legacy mullvad gRPC, dead at runtime)")
            managementService.start()

            lifecycleScope.launch {
                managementService.tunnelState
                    .filterIsInstance<TunnelState.Error>()
                    .filter { !it.errorState.isBlocking }
                    .collect { foregroundNotificationHandler.stopForeground() }
            }
        } catch (e: Exception) {
            Logger.w(throwable = e) {
                "managementService.start() failed (expected on Warren mobile — no daemon backend); " +
                    "D.4 step 6 surgical removal pending"
            }
        }
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
                foregroundNotificationHandler.startForeground()
                // Legacy `connectionProxy.connectWithoutPermissionCheck()` proxies to a
                // gRPC-driven daemon that does not exist on Warren mobile. The new
                // connect path is `quinnAdapter.connect(config, mnemonic)` driven by
                // D.4 step 7's Intent extras carrying a serialised WarrenTunnelConfig.
                // We keep the legacy dispatch behind try/catch so the service still
                // boots on emulator while the new wiring is built.
                lifecycleScope.launch {
                    try {
                        connectionProxy.connectWithoutPermissionCheck()
                    } catch (e: Exception) {
                        Logger.w(throwable = e) {
                            "connectionProxy.connect dead at runtime (pending D.4 step 7)"
                        }
                    }
                }
            }

            intent?.action == KEY_RECONNECT_ACTION -> {
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch {
                    try {
                        connectionProxy.reconnect()
                    } catch (e: Exception) {
                        Logger.w(throwable = e) {
                            "connectionProxy.reconnect dead at runtime (pending D.4 step 7)"
                        }
                    }
                }
            }

            intent?.action == KEY_DISCONNECT_ACTION -> {
                // WarrenTileService might have launched this service with the expectancy of it
                // being foreground, thus it must go into foreground to please the android system
                // requirements.
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch {
                    try {
                        connectionProxy.disconnect(DisconnectReason.USER_INITIATED_NOTIFICATION_TILE)
                    } catch (e: Exception) {
                        Logger.w(throwable = e) {
                            "connectionProxy.disconnect dead at runtime (pending D.4 step 7)"
                        }
                    }
                    // Always issue a Quinn-side disconnect: if a Quinn session is
                    // running, this brings it down cleanly; if not, the call is a
                    // no-op (Mutex + state-machine guard inside the adapter).
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
        runBlocking {
            try {
                connectionProxy.disconnect(DisconnectReason.SYSTEM_REVOKE)
            } catch (e: Exception) {
                Logger.w(throwable = e) {
                    "connectionProxy.disconnect dead at runtime on revoke (pending D.4 step 7)"
                }
            }
            // Bring the Quinn session down cleanly on system revoke too.
            quinnAdapter.disconnect()
        }
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

        // Bring the Quinn session down before shutting the legacy layer.
        // `disconnect()` is idempotent (state-machine guards a no-op if
        // already disconnected), so it is safe even if no session was
        // ever started.
        runBlocking { quinnAdapter.disconnect() }

        // Shut down the legacy managementService gRPC layer. These calls
        // are no-ops at runtime on Warren mobile (no daemon backend);
        // wrapped in try/catch so a missing socket doesn't crash the
        // service teardown. D.4 step 6 will excise this entire layer.
        try {
            managementService.stop()
            Logger.i("Enter Idle")
            managementService.enterIdle()
        } catch (e: Exception) {
            Logger.w(throwable = e) {
                "managementService teardown failed (dead at runtime — expected)"
            }
        }

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
