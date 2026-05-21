package com.warrenbrowse.vpn.lib.pushnotification.tunnelstate

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationAction
import com.warrenbrowse.vpn.lib.model.NotificationChannelId
import com.warrenbrowse.vpn.lib.model.NotificationId
import com.warrenbrowse.vpn.lib.model.NotificationTunnelState
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

class TunnelStateNotificationProvider(
    context: Context,
    connectionProxy: ConnectionProxy,
    deviceRepository: DeviceRepository,
    preferences: UserPreferencesRepository,
    channelId: NotificationChannelId,
    scope: CoroutineScope,
) : NotificationProvider<Notification.Tunnel> {
    val notificationId = NotificationId(2)

    override val notifications: StateFlow<NotificationUpdate<Notification.Tunnel>> =
        combine(
                connectionProxy.tunnelState,
                deviceRepository.deviceState,
                preferences.preferencesFlow(),
            ) { tunnelState, deviceState, prefs ->
                if (
                    deviceState is DeviceState.LoggedOut && tunnelState is TunnelState.Disconnected
                ) {
                    return@combine NotificationUpdate.Cancel(notificationId)
                }
                val notificationTunnelState =
                    tunnelState.toNotificationTunnelState(
                        prepareError = context.prepareVpnSafe().leftOrNull(),
                        showLocation = prefs.showLocationInSystemNotification,
                    )

                return@combine NotificationUpdate.Notify(
                    notificationId,
                    Notification.Tunnel(
                        channelId = channelId,
                        state = notificationTunnelState,
                        actions = notificationTunnelState.toActions(),
                        ongoing = notificationTunnelState is NotificationTunnelState.Connected,
                    ),
                )
            }
            .stateIn(scope, SharingStarted.Eagerly, NotificationUpdate.Cancel(notificationId))

    private fun TunnelState.toNotificationTunnelState(
        prepareError: PrepareError?,
        showLocation: Boolean,
    ) =
        when (this) {
            is TunnelState.Disconnected -> NotificationTunnelState.Disconnected(prepareError)
            is TunnelState.Connecting ->
                NotificationTunnelState.Connecting(if (showLocation) location else null)
            is TunnelState.Disconnecting ->
                when (actionAfterDisconnect) {
                    ActionAfterDisconnect.Reconnect -> NotificationTunnelState.Connecting(null)
                    ActionAfterDisconnect.Block -> NotificationTunnelState.Blocking
                    ActionAfterDisconnect.Nothing -> NotificationTunnelState.Disconnecting
                }
            is TunnelState.Connected ->
                NotificationTunnelState.Connected(if (showLocation) location else null)
            is TunnelState.Error -> toNotificationTunnelState()
        }

    private fun TunnelState.Error.toNotificationTunnelState(): NotificationTunnelState.Error {
        val cause = errorState.cause
        return when {
            cause is ErrorStateCause.IsOffline && errorState.isBlocking ->
                NotificationTunnelState.Error.DeviceOffline
            cause is ErrorStateCause.InvalidDnsServers -> NotificationTunnelState.Error.Blocked
            cause is ErrorStateCause.OtherLegacyAlwaysOnApp ->
                NotificationTunnelState.Error.LegacyLockdown
            cause is ErrorStateCause.NotPrepared ->
                NotificationTunnelState.Error.VpnPermissionDenied
            cause is ErrorStateCause.OtherAlwaysOnApp ->
                NotificationTunnelState.Error.AlwaysOnVpn(cause.appName)
            errorState.isBlocking -> NotificationTunnelState.Error.Blocked
            else -> NotificationTunnelState.Error.Critical
        }
    }

    private fun NotificationTunnelState.toActions(): List<NotificationAction.Tunnel> =
        when (this) {
            is NotificationTunnelState.Disconnected -> {
                when (prepareError) {
                    is PrepareError.OtherAlwaysOnApp,
                    is PrepareError.OtherLegacyAlwaysOnVpn,
                    null -> listOf(NotificationAction.Tunnel.Connect)
                    is PrepareError.NotPrepared ->
                        listOf(NotificationAction.Tunnel.RequestVpnProfile)
                }
            }
            NotificationTunnelState.Disconnecting -> listOf(NotificationAction.Tunnel.Connect)
            NotificationTunnelState.Error.Blocked,
            NotificationTunnelState.Blocking,
            NotificationTunnelState.Error.DeviceOffline,
            is NotificationTunnelState.Connected ->
                listOf(NotificationAction.Tunnel.Reconnect, NotificationAction.Tunnel.Disconnect)
            is NotificationTunnelState.Connecting -> listOf(NotificationAction.Tunnel.Cancel)
            is NotificationTunnelState.Error.Critical,
            NotificationTunnelState.Error.VpnPermissionDenied,
            is NotificationTunnelState.Error.AlwaysOnVpn,
            NotificationTunnelState.Error.LegacyLockdown ->
                listOf(NotificationAction.Tunnel.Dismiss)
        }
}
