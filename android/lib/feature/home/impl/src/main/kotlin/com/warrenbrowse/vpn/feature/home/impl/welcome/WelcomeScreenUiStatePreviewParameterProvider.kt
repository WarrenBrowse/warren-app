package com.warrenbrowse.vpn.feature.home.impl.welcome

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.home.impl.TunnelStatePreviewData
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.AccountNumber

class WelcomeScreenUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Unit, WelcomeUiState>> {
    override val values =
        sequenceOf(
            Lc.Loading(Unit),
            WelcomeUiState(
                    tunnelState = TunnelStatePreviewData.generateDisconnectedState(),
                    accountNumber = AccountNumber("4444555566667777"),
                    deviceName = "Happy Mole",
                    showSitePayment = false,
                    verificationPending = true,
                )
                .toLc(),
            WelcomeUiState(
                    tunnelState =
                        TunnelStatePreviewData.generateConnectedState(featureIndicators = 1, false),
                    accountNumber = AccountNumber("4444555566667777"),
                    deviceName = "Happy Mole",
                    showSitePayment = true,
                    verificationPending = false,
                )
                .toLc(),
        )
}
