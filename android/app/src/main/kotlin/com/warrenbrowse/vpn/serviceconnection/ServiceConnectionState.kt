package com.warrenbrowse.vpn.serviceconnection

sealed class ServiceConnectionState {
    data object Bound : ServiceConnectionState()

    data object Unbound : ServiceConnectionState()
}
