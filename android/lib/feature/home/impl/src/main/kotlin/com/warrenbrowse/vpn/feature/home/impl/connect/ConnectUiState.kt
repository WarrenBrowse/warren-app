package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState

data class ConnectUiState(
    val location: GeoIpLocation?,
    val selectedRelayItemTitle: String?,
    val tunnelState: TunnelState,
    val inAppNotification: InAppNotification?,
    val deviceName: String?,
    // D.4 step 42: daysLeftUntilExpiry dropped (Mullvad subscription dead).
    val isPlayBuild: Boolean,
) {

    val showLoading =
        tunnelState is TunnelState.Connecting || tunnelState is TunnelState.Disconnecting

    companion object {
        val INITIAL =
            ConnectUiState(
                location = null,
                selectedRelayItemTitle = null,
                tunnelState = TunnelState.Disconnected(),
                inAppNotification = null,
                deviceName = null,
                isPlayBuild = false,
            )
    }
}
