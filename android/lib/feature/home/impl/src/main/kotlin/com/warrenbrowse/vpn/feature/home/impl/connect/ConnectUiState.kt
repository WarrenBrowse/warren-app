package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState

data class ConnectUiState(
    val location: GeoIpLocation?,
    val selectedRelayItemTitle: String?,
    val tunnelState: TunnelState,
    val inAppNotification: InAppNotification?,
    val isPlayBuild: Boolean,
    // Debounced host-offline verdict; degrades the Connected presentation
    // ("Connection interrupted") because the tunnel can hold Connected
    // through its transparent redial window while nothing flows.
    val hostOffline: Boolean = false,
    // Automatic recoveries since process start (native redials + retry-loop
    // successes); shown as the "Reconnections" connection-details row.
    val autoRecoveryCount: Int = 0,
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
                isPlayBuild = false,
                hostOffline = false,
                autoRecoveryCount = 0,
            )
    }
}
