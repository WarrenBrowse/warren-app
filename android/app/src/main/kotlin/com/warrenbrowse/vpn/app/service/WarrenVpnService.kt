package com.warrenbrowse.vpn.app.service

import android.app.KeyguardManager
import android.content.Intent
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
                lifecycleScope.launch { connectionProxy.connectWithoutPermissionCheck() }
            }

            intent?.action == KEY_RECONNECT_ACTION -> {
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch { connectionProxy.reconnect() }
            }

            intent?.action == KEY_DISCONNECT_ACTION -> {
                // WarrenTileService might have launched this service with the expectancy of it
                // being foreground, thus it must go into foreground to please the android system
                // requirements.
                foregroundNotificationHandler.startForeground()
                lifecycleScope.launch {
                    connectionProxy.disconnect(DisconnectReason.USER_INITIATED_NOTIFICATION_TILE)
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
        runBlocking { connectionProxy.disconnect(DisconnectReason.SYSTEM_REVOKE) }
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
