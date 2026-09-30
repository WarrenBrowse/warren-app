package com.warrenbrowse.vpn.lib.model

sealed interface NotificationTunnelState {
    data class Disconnected(val prepareError: PrepareError?) : NotificationTunnelState

    data class Connecting(val location: GeoIpLocation?) : NotificationTunnelState

    data class Connected(val location: GeoIpLocation?) : NotificationTunnelState

    data object Blocking : NotificationTunnelState

    data object Disconnecting : NotificationTunnelState

    sealed interface Error : NotificationTunnelState {
        data object DeviceOffline : Error

        data object Blocked : Error

        /**
         * Blocked because this network reaches no entry server the circuit may
         * use. Its own title: under the generic one a user's quote could not
         * tell this network apart from an outage (topic 210).
         */
        data object NoDialableNetwork : Error

        data object VpnPermissionDenied : Error

        data class AlwaysOnVpn(val appName: String) : Error

        data object LegacyLockdown : Error

        data object Critical : Error

        /** The tunnel gave up after repeated drops and the traffic left the VPN. */
        data object TrafficReleased : Error
    }
}
