package com.warrenbrowse.vpn.lib.model

sealed interface NotificationAction {

    // D.4 step 38: AccountExpiry actions dropped (subscription expiry dead).

    sealed interface Tunnel : NotificationAction {
        data object Connect : Tunnel

        data object Reconnect : Tunnel

        data object Disconnect : Tunnel

        data object Cancel : Tunnel

        data object Dismiss : Tunnel

        data object RequestVpnProfile : Tunnel
    }
}
