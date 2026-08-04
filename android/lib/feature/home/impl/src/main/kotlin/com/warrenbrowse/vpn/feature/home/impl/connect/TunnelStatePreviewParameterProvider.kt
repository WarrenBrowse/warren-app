package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateConnectedState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateConnectingState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateDisconnectedState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateDisconnectingState
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData.generateErrorState
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.TunnelState

class TunnelStatePreviewParameterProvider : PreviewParameterProvider<TunnelState> {
    override val values: Sequence<TunnelState> =
        sequenceOf(
            generateDisconnectedState(),
            generateConnectingState(featureIndicators = 0, quantumResistant = false),
            generateConnectingState(featureIndicators = 0, quantumResistant = true),
            generateConnectedState(featureIndicators = 0, quantumResistant = false),
            generateConnectedState(featureIndicators = 0, quantumResistant = true),
            generateDisconnectingState(actionAfterDisconnect = ActionAfterDisconnect.Block),
            generateDisconnectingState(actionAfterDisconnect = ActionAfterDisconnect.Nothing),
            generateDisconnectingState(actionAfterDisconnect = ActionAfterDisconnect.Reconnect),
            generateErrorState(isBlocking = true),
            generateErrorState(isBlocking = false),
        )
}
