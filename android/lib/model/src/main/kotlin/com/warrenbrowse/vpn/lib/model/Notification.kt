package com.warrenbrowse.vpn.lib.model

sealed interface Notification {
    val actions: List<NotificationAction>
    val ongoing: Boolean
    val channelId: NotificationChannelId

    data class Tunnel(
        override val channelId: NotificationChannelId,
        val state: NotificationTunnelState,
        override val actions: List<NotificationAction.Tunnel>,
        override val ongoing: Boolean,
    ) : Notification

    // D.4 step 38: Notification.AccountExpiry dropped (Mullvad subscription
    // expiry notifications dead on Warren).
}
