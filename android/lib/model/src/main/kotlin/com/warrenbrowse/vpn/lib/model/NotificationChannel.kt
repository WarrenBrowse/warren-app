package com.warrenbrowse.vpn.lib.model

sealed interface NotificationChannel {
    val id: NotificationChannelId

    data object TunnelUpdates : NotificationChannel {
        private const val CHANNEL_ID = "vpn_tunnel_status"
        override val id: NotificationChannelId = NotificationChannelId(CHANNEL_ID)
    }

    // D.4 step 38: AccountUpdates channel dropped (Mullvad subscription
    // expiry notifications dead on Warren).
}
