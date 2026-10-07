package com.warrenbrowse.vpn.app.service.notifications

import android.app.Service
import android.content.pm.ServiceInfo
import android.os.Build
import co.touchlab.kermit.Logger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.app.service.WarrenQuinnAdapter
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationChannel
import com.warrenbrowse.vpn.lib.model.NotificationTunnelState
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import com.warrenbrowse.vpn.lib.pushnotification.tunnelstate.TunnelStateNotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.withAppLocale
import com.warrenbrowse.vpn.lib.pushnotification.tunnelstate.toNotification

class ForegroundNotificationManager(
    private val vpnService: WarrenVpnService,
    private val tunnelStateNotificationProvider: TunnelStateNotificationProvider,
) {
    fun startForeground() {
        Logger.d("startForeground")
        notifyForeground(getTunnelStateNotificationOrDefault())
    }

    fun stopForeground() {
        Logger.d("stopForeground")
        vpnService.stopForeground(Service.STOP_FOREGROUND_DETACH)
    }

    /**
     * The blackhole holding the apps locked to the VPN lives as long as the
     * service does: in the foreground while [adapter] holds it, and out of it
     * with the blackhole when [idle] says nothing else needs it there.
     */
    fun followLockGuard(scope: CoroutineScope, adapter: WarrenQuinnAdapter, idle: () -> Boolean) {
        lockGuardActive = adapter.lockGuardActive
        scope.launch {
            adapter.lockGuardActive.collect { held ->
                if (held) startForeground() else if (idle()) stopForeground()
            }
        }
    }

    /** [stopForeground], unless the service holds the blackhole of locked apps. */
    fun stopForegroundUnlessHolding() {
        if (lockGuardActive?.value != true) stopForeground()
    }

    private var lockGuardActive: StateFlow<Boolean>? = null

    private fun getTunnelStateNotificationOrDefault(): Notification.Tunnel {
        val current = tunnelStateNotificationProvider.notifications.value

        return if (current is NotificationUpdate.Notify) {
            current.value
        } else {
            defaultNotification
        }
    }

    private fun notifyForeground(tunnelStateNotification: Notification.Tunnel) {

        // The service context resolves the system language below API 33,
        // whatever the picker says.
        val androidNotification = tunnelStateNotification.toNotification(vpnService.withAppLocale())
        if (vpnService.prepareVpnSafe().isLeft()) {
            // Got connect/disconnect intent, but we  don't have permission to go in foreground.
            // tunnel state will return permission and we will eventually get stopped by system.
            Logger.i("Did not start foreground: VPN permission not granted")
            return
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            Logger.i("Starting foreground UPSIDE_DOWN_CAKE")
            vpnService.startForeground(
                tunnelStateNotificationProvider.notificationId.value,
                androidNotification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
            )
        } else {
            vpnService.startForeground(
                tunnelStateNotificationProvider.notificationId.value,
                androidNotification,
            )
        }
    }

    private val defaultNotification =
        Notification.Tunnel(
            NotificationChannel.TunnelUpdates.id,
            NotificationTunnelState.Disconnected(null),
            emptyList(),
            false,
        )
}
