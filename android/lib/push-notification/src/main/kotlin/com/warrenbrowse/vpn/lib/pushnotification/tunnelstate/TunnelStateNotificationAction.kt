package com.warrenbrowse.vpn.lib.pushnotification.tunnelstate

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.warrenbrowse.vpn.lib.common.constant.KEY_CONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_REQUEST_VPN_PROFILE
import com.warrenbrowse.vpn.lib.common.constant.MAIN_ACTIVITY_CLASS
import com.warrenbrowse.vpn.lib.common.util.getSupportedPendingIntentFlags
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationAction
import com.warrenbrowse.vpn.lib.model.NotificationTunnelState
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.ui.resource.R

fun Notification.Tunnel.toNotification(context: Context) =
    NotificationCompat.Builder(context, channelId.value)
        .setContentIntent(contentIntent(context))
        .setContentTitle(state.notificationTitle(context))
        .setContentText(state.notificationText(context))
        .setSmallIcon(R.drawable.small_logo_white)
        .apply { actions.forEach { addAction(it.toCompatAction(context)) } }
        .setOngoing(ongoing)
        .setVisibility(NotificationCompat.VISIBILITY_SECRET)
        .build()

private fun Notification.Tunnel.contentIntent(context: Context): PendingIntent {
    val intent =
        Intent().apply {
            setClassName(context.packageName, MAIN_ACTIVITY_CLASS)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
            action = Intent.ACTION_MAIN
        }

    return PendingIntent.getActivity(context, 1, intent, getSupportedPendingIntentFlags())
}

internal fun NotificationTunnelState.notificationTitle(context: Context): String =
    when (this) {
        is NotificationTunnelState.Connected ->
            location?.shortName()?.let { context.getString(R.string.notification_connected_to, it) }
                ?: context.getString(R.string.connected)
        is NotificationTunnelState.Connecting ->
            location?.shortName()?.let {
                context.getString(R.string.notification_connecting_to, it)
            } ?: context.getString(R.string.connecting)
        is NotificationTunnelState.Disconnected -> {
            when (prepareError) {
                is PrepareError.NotPrepared ->
                    context.getString(R.string.disconnected_vpn_permission_error)
                // Desktop spells out that a plain disconnect leaves the device
                // exposed; "Disconnected" alone reads as a neutral idle state.
                else -> context.getString(R.string.notification_disconnected_unsecure)
            }
        }
        NotificationTunnelState.Disconnecting -> context.getString(R.string.disconnecting)
        NotificationTunnelState.Blocking -> context.getString(R.string.blocking)
        NotificationTunnelState.Error.Blocked -> context.getString(R.string.blocking_internet)
        is NotificationTunnelState.Error.Critical -> context.getString(R.string.critical_error)
        NotificationTunnelState.Error.DeviceOffline ->
            context.getString(R.string.blocking_internet_device_offline)
        NotificationTunnelState.Error.VpnPermissionDenied ->
            context.getString(R.string.vpn_permission_error_notification_title)
        is NotificationTunnelState.Error.AlwaysOnVpn ->
            context.getString(R.string.always_on_vpn_error_notification_title, appName)
        NotificationTunnelState.Error.LegacyLockdown ->
            context.getString(R.string.legacy_always_on_vpn_error_notification_title)
    }

/** The city, or the country when the exit reports no city. */
private fun GeoIpLocation.shortName(): String = city?.takeIf { it.isNotBlank() } ?: country

internal fun NotificationTunnelState.notificationText(context: Context): CharSequence? {
    val location =
        when (this) {
            is NotificationTunnelState.Connected -> location
            is NotificationTunnelState.Connecting -> location
            else -> null
        } ?: return null
    val city = location.city?.takeIf { it.isNotBlank() } ?: return location.country
    // Never the relay hostname: the location picker deliberately hides raw
    // endpoints, so the notification must not leak what the UI withholds.
    return context.getString(R.string.country_comma_city, location.country, city)
}

internal fun NotificationAction.Tunnel.toCompatAction(context: Context): NotificationCompat.Action {

    val pendingIntent =
        if (this is NotificationAction.Tunnel.RequestVpnProfile) {
            val intent =
                Intent().apply {
                    setClassName(context.packageName, MAIN_ACTIVITY_CLASS)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP)
                    setAction(KEY_REQUEST_VPN_PROFILE)
                }

            PendingIntent.getActivity(context, 1, intent, getSupportedPendingIntentFlags())
        } else {
            val intent = Intent(toKey()).setPackage(context.packageName)
            PendingIntent.getService(context, 1, intent, getSupportedPendingIntentFlags())
        }

    return NotificationCompat.Action(
        toIconResource(),
        context.getString(titleResource()),
        pendingIntent,
    )
}

fun NotificationAction.Tunnel.titleResource() =
    when (this) {
        NotificationAction.Tunnel.Cancel -> R.string.cancel
        NotificationAction.Tunnel.Connect,
        is NotificationAction.Tunnel.RequestVpnProfile -> R.string.connect
        NotificationAction.Tunnel.Reconnect -> R.string.reconnect
        NotificationAction.Tunnel.Disconnect -> R.string.disconnect
        NotificationAction.Tunnel.Dismiss -> R.string.dismiss
    }

fun NotificationAction.Tunnel.toKey() =
    when (this) {
        NotificationAction.Tunnel.Connect -> KEY_CONNECT_ACTION
        NotificationAction.Tunnel.Reconnect -> KEY_RECONNECT_ACTION
        is NotificationAction.Tunnel.RequestVpnProfile -> KEY_REQUEST_VPN_PROFILE
        NotificationAction.Tunnel.Cancel,
        NotificationAction.Tunnel.Disconnect,
        NotificationAction.Tunnel.Dismiss -> KEY_DISCONNECT_ACTION
    }

fun NotificationAction.Tunnel.toIconResource() =
    when (this) {
        NotificationAction.Tunnel.Connect -> R.drawable.icon_notification_connect
        else -> R.drawable.icon_notification_disconnect
    }
