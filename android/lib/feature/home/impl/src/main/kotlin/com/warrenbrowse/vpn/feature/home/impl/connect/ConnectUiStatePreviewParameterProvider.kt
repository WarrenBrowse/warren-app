package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import java.net.InetAddress
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification

class ConnectUiStatePreviewParameterProvider : PreviewParameterProvider<ConnectUiState> {
    override val values = sequenceOf(ConnectUiState.INITIAL) + otherStates
}

private val otherStates =
    sequenceOf(
            TunnelStatePreviewData.generateConnectedState(
                featureIndicators = 8,
                quantumResistant = true,
            ),
            TunnelStatePreviewData.generateDisconnectedState(),
            TunnelStatePreviewData.generateConnectingState(
                featureIndicators = 4,
                quantumResistant = false,
            ),
            TunnelStatePreviewData.generateDisconnectingState(
                actionAfterDisconnect = ActionAfterDisconnect.Reconnect
            ),
            TunnelStatePreviewData.generateDisconnectingState(
                actionAfterDisconnect = ActionAfterDisconnect.Block
            ),
            TunnelStatePreviewData.generateErrorState(isBlocking = true),
        )
        .mapIndexed { index, state ->
            ConnectUiState(
                location =
                    GeoIpLocation(
                        ipv4 = InetAddress.getLocalHost(),
                        ipv6 = null,
                        country = "Sweden",
                        city = "Göteborg",
                        latitude = 23.3,
                        longitude = 12.99,
                        hostname = "Hostname",
                        entryHostname = "EntryHostname",
                    ),
                selectedRelayItemTitle = "Relay Title",
                tunnelState = state,
                inAppNotification =
                    if (index == 0) InAppNotification.NewVersionChangelog else null,
                isPlayBuild = true,
            )
        }
