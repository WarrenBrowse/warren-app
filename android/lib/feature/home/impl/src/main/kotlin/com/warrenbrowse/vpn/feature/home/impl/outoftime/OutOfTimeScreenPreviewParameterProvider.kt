package com.warrenbrowse.vpn.feature.home.impl.outoftime

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateConnectingState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateDisconnectedState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateErrorState

class OutOfTimeScreenPreviewParameterProvider : PreviewParameterProvider<OutOfTimeUiState> {
    override val values: Sequence<OutOfTimeUiState> =
        sequenceOf(
            OutOfTimeUiState(
                tunnelState = generateDisconnectedState(),
                "Heroic Frog",
                showSitePayment = true,
            ),
            OutOfTimeUiState(
                tunnelState =
                    generateConnectingState(featureIndicators = 0, quantumResistant = false),
                "Strong Rabbit",
                showSitePayment = true,
            ),
            OutOfTimeUiState(
                tunnelState = generateErrorState(isBlocking = true),
                deviceName = "Stable Horse",
                showSitePayment = true,
            ),
        )
}
